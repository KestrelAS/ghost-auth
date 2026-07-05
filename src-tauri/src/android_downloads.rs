//! Android: save files into the public Downloads collection via MediaStore.
//!
//! On Android 10+ (API 29) scoped storage forbids raw filesystem writes to
//! shared external storage. Tauri's `path().download_dir()` resolves to the
//! app-scoped external directory (`.../Android/data/<pkg>/files/Download`),
//! which is invisible in the user's Downloads app and wiped on uninstall.
//!
//! The supported, permission-free way to place a file in the user-visible
//! Downloads folder is the MediaStore API: insert a row into
//! `MediaStore.Downloads`, stream the bytes to its content URI, then clear the
//! `IS_PENDING` flag to publish it.
//!
//! This mirrors the JNI approach used in `keystore.rs` for the Android KeyStore.

use jni::JNIEnv;
use jni::JavaVM;
use jni::objects::{JObject, JValue};

// MediaStore column names. These are the stable, documented string values of
// the `MediaStore.MediaColumns` constants (`_display_name`, `mime_type`,
// `relative_path`, `is_pending`); using the literals avoids fragile reflective
// static-field lookups and never changes as part of the public contract.
const COL_DISPLAY_NAME: &str = "_display_name";
const COL_MIME_TYPE: &str = "mime_type";
const COL_RELATIVE_PATH: &str = "relative_path";
const COL_IS_PENDING: &str = "is_pending";

// Value of Environment.DIRECTORY_DOWNLOADS — the Downloads sub-path.
const DIRECTORY_DOWNLOADS: &str = "Download";

// MediaStore.Downloads and IS_PENDING were both introduced in API 29.
const MIN_MEDIASTORE_API: i32 = 29;

fn jni_err(e: impl std::fmt::Display) -> String {
    format!("JNI error: {e}")
}

/// Run a closure with a JNI environment and Android context, clearing any
/// pending Java exception on error so it cannot poison later JNI calls on this
/// thread.
fn with_jni<F, T>(f: F) -> Result<T, String>
where
    F: FnOnce(&mut JNIEnv, &JObject) -> Result<T, String>,
{
    let ctx = ndk_context::android_context();
    let vm = unsafe { JavaVM::from_raw(ctx.vm().cast()) }
        .map_err(|e| format!("Failed to get JavaVM: {e}"))?;
    let mut env = vm
        .attach_current_thread_as_daemon()
        .map_err(|e| format!("Failed to attach JNI thread: {e}"))?;
    let context = unsafe { JObject::from_raw(ctx.context().cast()) };

    let result = f(&mut env, &context);
    if result.is_err() {
        let _ = env.exception_clear();
    }
    result
}

fn jstr<'a>(env: &mut JNIEnv<'a>, s: &str) -> Result<JObject<'a>, String> {
    Ok(env.new_string(s).map_err(jni_err)?.into())
}

/// The running device's API level (`Build.VERSION.SDK_INT`).
fn sdk_int(env: &mut JNIEnv) -> Result<i32, String> {
    env.get_static_field("android/os/Build$VERSION", "SDK_INT", "I")
        .map_err(jni_err)?
        .i()
        .map_err(jni_err)
}

/// `values.put(key, stringValue)`
fn put_string(env: &mut JNIEnv, values: &JObject, key: &str, val: &str) -> Result<(), String> {
    let k = jstr(env, key)?;
    let v = jstr(env, val)?;
    env.call_method(
        values,
        "put",
        "(Ljava/lang/String;Ljava/lang/String;)V",
        &[JValue::Object(&k), JValue::Object(&v)],
    )
    .map_err(jni_err)?;
    Ok(())
}

/// `values.put(key, Integer.valueOf(intValue))`
fn put_int(env: &mut JNIEnv, values: &JObject, key: &str, val: i32) -> Result<(), String> {
    let k = jstr(env, key)?;
    let boxed = env
        .call_static_method(
            "java/lang/Integer",
            "valueOf",
            "(I)Ljava/lang/Integer;",
            &[JValue::Int(val)],
        )
        .map_err(jni_err)?
        .l()
        .map_err(jni_err)?;
    env.call_method(
        values,
        "put",
        "(Ljava/lang/String;Ljava/lang/Integer;)V",
        &[JValue::Object(&k), JValue::Object(&boxed)],
    )
    .map_err(jni_err)?;
    Ok(())
}

/// Save `data` to the public Downloads collection via MediaStore (API 29+).
///
/// Returns a user-facing location label ("Download/<filename>") on success.
/// Callers should treat an `Err` (including pre-29 devices) as a signal to fall
/// back to app-scoped storage rather than failing the operation outright.
pub fn save_to_downloads(filename: &str, mime: &str, data: &[u8]) -> Result<String, String> {
    with_jni(|env, context| {
        if sdk_int(env)? < MIN_MEDIASTORE_API {
            return Err("MediaStore Downloads requires Android 10 (API 29)".to_string());
        }

        // ContentResolver resolver = context.getContentResolver();
        let resolver = env
            .call_method(
                context,
                "getContentResolver",
                "()Landroid/content/ContentResolver;",
                &[],
            )
            .map_err(jni_err)?
            .l()
            .map_err(jni_err)?;

        // ContentValues values = new ContentValues();
        let values = env
            .new_object("android/content/ContentValues", "()V", &[])
            .map_err(jni_err)?;
        put_string(env, &values, COL_DISPLAY_NAME, filename)?;
        put_string(env, &values, COL_MIME_TYPE, mime)?;
        put_string(env, &values, COL_RELATIVE_PATH, DIRECTORY_DOWNLOADS)?;
        // Mark pending so no other app sees a partially written file.
        put_int(env, &values, COL_IS_PENDING, 1)?;

        // Uri collection = MediaStore.Downloads.EXTERNAL_CONTENT_URI;
        let collection = env
            .get_static_field(
                "android/provider/MediaStore$Downloads",
                "EXTERNAL_CONTENT_URI",
                "Landroid/net/Uri;",
            )
            .map_err(jni_err)?
            .l()
            .map_err(jni_err)?;

        // Uri item = resolver.insert(collection, values);
        let item = env
            .call_method(
                &resolver,
                "insert",
                "(Landroid/net/Uri;Landroid/content/ContentValues;)Landroid/net/Uri;",
                &[JValue::Object(&collection), JValue::Object(&values)],
            )
            .map_err(jni_err)?
            .l()
            .map_err(jni_err)?;
        if item.is_null() {
            return Err("MediaStore insert returned null".to_string());
        }

        // Stream the bytes to the content URI.
        let write_result = write_to_uri(env, &resolver, &item, data);

        match write_result {
            Ok(()) => {
                // values2.put(IS_PENDING, 0); resolver.update(item, values2, null, null);
                let publish = env
                    .new_object("android/content/ContentValues", "()V", &[])
                    .map_err(jni_err)?;
                put_int(env, &publish, COL_IS_PENDING, 0)?;
                env.call_method(
                    &resolver,
                    "update",
                    "(Landroid/net/Uri;Landroid/content/ContentValues;Ljava/lang/String;[Ljava/lang/String;)I",
                    &[
                        JValue::Object(&item),
                        JValue::Object(&publish),
                        JValue::Object(&JObject::null()),
                        JValue::Object(&JObject::null()),
                    ],
                )
                .map_err(jni_err)?;
                Ok(format!("{DIRECTORY_DOWNLOADS}/{filename}"))
            }
            Err(e) => {
                // Clear the pending Java exception before issuing more JNI calls,
                // then remove the empty MediaStore row we inserted.
                let _ = env.exception_clear();
                let _ = env.call_method(
                    &resolver,
                    "delete",
                    "(Landroid/net/Uri;Ljava/lang/String;[Ljava/lang/String;)I",
                    &[
                        JValue::Object(&item),
                        JValue::Object(&JObject::null()),
                        JValue::Object(&JObject::null()),
                    ],
                );
                Err(e)
            }
        }
    })
}

/// `OutputStream os = resolver.openOutputStream(uri); os.write(data); os.close();`
fn write_to_uri(
    env: &mut JNIEnv,
    resolver: &JObject,
    uri: &JObject,
    data: &[u8],
) -> Result<(), String> {
    let os = env
        .call_method(
            resolver,
            "openOutputStream",
            "(Landroid/net/Uri;)Ljava/io/OutputStream;",
            &[JValue::Object(uri)],
        )
        .map_err(jni_err)?
        .l()
        .map_err(jni_err)?;
    if os.is_null() {
        return Err("openOutputStream returned null".to_string());
    }

    let arr = env.byte_array_from_slice(data).map_err(jni_err)?;
    let write = env.call_method(&os, "write", "([B)V", &[JValue::Object(&arr.into())]);
    // Always attempt to close the stream, even if the write failed.
    let close = env.call_method(&os, "close", "()V", &[]);

    write.map_err(jni_err)?;
    close.map_err(jni_err)?;
    Ok(())
}
