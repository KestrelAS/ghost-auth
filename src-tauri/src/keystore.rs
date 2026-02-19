//! Platform-abstracted secure key storage.
//!
//! On desktop (Windows/macOS/Linux): uses the OS keychain via the `keyring` crate.
//! On iOS: uses the system Keychain via the `security-framework` crate.
//! On Android: uses Android KeyStore via JNI for hardware-backed key wrapping.

const SERVICE: &str = "ghost-auth";
const ACCOUNT: &str = "encryption-key";

// ── Desktop: OS keychain via keyring crate ──────────────────────────

#[cfg(not(any(target_os = "android", target_os = "ios")))]
pub fn load_key() -> Option<[u8; 32]> {
    let entry = keyring::Entry::new(SERVICE, ACCOUNT).ok()?;
    let secret = entry.get_secret().ok()?;
    if secret.len() != 32 {
        return None;
    }
    let mut key = [0u8; 32];
    key.copy_from_slice(&secret);
    Some(key)
}

#[cfg(not(any(target_os = "android", target_os = "ios")))]
pub fn store_key(key: &[u8; 32]) -> bool {
    let Ok(entry) = keyring::Entry::new(SERVICE, ACCOUNT) else {
        return false;
    };
    entry.set_secret(key).is_ok()
}

#[cfg(not(any(target_os = "android", target_os = "ios")))]
#[allow(dead_code)]
pub fn delete_key() -> bool {
    let Ok(entry) = keyring::Entry::new(SERVICE, ACCOUNT) else {
        return false;
    };
    entry.delete_credential().is_ok()
}

// ── iOS: Keychain via security-framework ────────────────────────────

#[cfg(target_os = "ios")]
pub fn load_key() -> Option<[u8; 32]> {
    use security_framework::passwords::get_generic_password;
    match get_generic_password(SERVICE, ACCOUNT) {
        Ok(secret) if secret.len() == 32 => {
            let mut key = [0u8; 32];
            key.copy_from_slice(&secret);
            Some(key)
        }
        _ => None,
    }
}

#[cfg(target_os = "ios")]
pub fn store_key(key: &[u8; 32]) -> bool {
    use security_framework::passwords::set_generic_password;
    set_generic_password(SERVICE, ACCOUNT, key).is_ok()
}

#[cfg(target_os = "ios")]
pub fn delete_key() -> bool {
    use security_framework::passwords::delete_generic_password;
    delete_generic_password(SERVICE, ACCOUNT).is_ok()
}

// ── Android: hardware-backed key storage via JNI ────────────────────
//
// Uses Android KeyStore to generate a hardware-backed AES-256/GCM master
// key, which wraps (encrypts) the app's 32-byte encryption key. The
// wrapped key is stored in SharedPreferences as Base64.
//
// The master key never leaves the secure hardware (TEE/StrongBox), so
// even on rooted devices the raw key material cannot be extracted.

#[cfg(target_os = "android")]
pub fn load_key() -> Option<[u8; 32]> {
    android_keystore::load().ok()
}

#[cfg(target_os = "android")]
pub fn store_key(key: &[u8; 32]) -> bool {
    android_keystore::store(key).is_ok()
}

#[cfg(target_os = "android")]
pub fn delete_key() -> bool {
    android_keystore::delete().is_ok()
}

#[cfg(target_os = "android")]
mod android_keystore {
    use jni::objects::{JByteArray, JObject, JValue};
    use jni::JNIEnv;
    use jni::JavaVM;

    const KEYSTORE_ALIAS: &str = "ghost_auth_master";
    const PREFS_NAME: &str = "ghost_auth_keys";
    const PREFS_KEY: &str = "wrapped_key";

    /// Run a closure with a JNI environment and Android context.
    fn with_jni<F, T>(f: F) -> Result<T, String>
    where
        F: FnOnce(&mut JNIEnv, &JObject) -> Result<T, String>,
    {
        // ndk-context is initialized by tao/android-activity before
        // Tauri's setup() runs. The returned VM pointer is valid for the
        // app's lifetime and context is a global reference held by the ART.
        let ctx = ndk_context::android_context();
        let vm = unsafe { JavaVM::from_raw(ctx.vm().cast()) }
            .map_err(|e| format!("Failed to get JavaVM: {e}"))?;
        let mut env = vm
            .attach_current_thread_as_daemon()
            .map_err(|e| format!("Failed to attach JNI thread: {e}"))?;
        let context = unsafe { JObject::from_raw(ctx.context().cast()) };
        f(&mut env, &context)
    }

    /// Create a JString-as-JObject from a Rust &str.
    fn jstr<'a>(env: &mut JNIEnv<'a>, s: &str) -> Result<JObject<'a>, String> {
        Ok(env.new_string(s).map_err(jni_err)?.into())
    }

    fn jni_err(e: impl std::fmt::Display) -> String {
        format!("JNI error: {e}")
    }

    /// Ensure the AES-256/GCM master key exists in Android KeyStore.
    fn ensure_master_key(env: &mut JNIEnv) -> Result<(), String> {
        // KeyStore ks = KeyStore.getInstance("AndroidKeyStore"); ks.load(null);
        let ks_type = jstr(env, "AndroidKeyStore")?;
        let ks = env
            .call_static_method(
                "java/security/KeyStore",
                "getInstance",
                "(Ljava/lang/String;)Ljava/security/KeyStore;",
                &[JValue::Object(&ks_type)],
            )
            .map_err(jni_err)?
            .l()
            .map_err(jni_err)?;
        env.call_method(
            &ks,
            "load",
            "(Ljava/security/KeyStore$LoadStoreParameter;)V",
            &[JValue::Object(&JObject::null())],
        )
        .map_err(jni_err)?;

        // if (ks.containsAlias(ALIAS)) return;
        let alias = jstr(env, KEYSTORE_ALIAS)?;
        let exists = env
            .call_method(
                &ks,
                "containsAlias",
                "(Ljava/lang/String;)Z",
                &[JValue::Object(&alias)],
            )
            .map_err(jni_err)?
            .z()
            .map_err(jni_err)?;
        if exists {
            return Ok(());
        }

        // KeyGenerator kg = KeyGenerator.getInstance("AES", "AndroidKeyStore");
        let aes = jstr(env, "AES")?;
        let aks = jstr(env, "AndroidKeyStore")?;
        let kg = env
            .call_static_method(
                "javax/crypto/KeyGenerator",
                "getInstance",
                "(Ljava/lang/String;Ljava/lang/String;)Ljavax/crypto/KeyGenerator;",
                &[JValue::Object(&aes), JValue::Object(&aks)],
            )
            .map_err(jni_err)?
            .l()
            .map_err(jni_err)?;

        // KeyGenParameterSpec.Builder(ALIAS, PURPOSE_ENCRYPT | PURPOSE_DECRYPT)
        let alias2 = jstr(env, KEYSTORE_ALIAS)?;
        let builder = env
            .new_object(
                "android/security/keystore/KeyGenParameterSpec$Builder",
                "(Ljava/lang/String;I)V",
                &[JValue::Object(&alias2), JValue::Int(1 | 2)],
            )
            .map_err(jni_err)?;

        // .setBlockModes(new String[]{"GCM"})
        let gcm = jstr(env, "GCM")?;
        let modes = env
            .new_object_array(1, "java/lang/String", &gcm)
            .map_err(jni_err)?;
        env.call_method(
            &builder,
            "setBlockModes",
            "([Ljava/lang/String;)Landroid/security/keystore/KeyGenParameterSpec$Builder;",
            &[JValue::Object(&modes.into())],
        )
        .map_err(jni_err)?;

        // .setEncryptionPaddings(new String[]{"NoPadding"})
        let nopad = jstr(env, "NoPadding")?;
        let pads = env
            .new_object_array(1, "java/lang/String", &nopad)
            .map_err(jni_err)?;
        env.call_method(
            &builder,
            "setEncryptionPaddings",
            "([Ljava/lang/String;)Landroid/security/keystore/KeyGenParameterSpec$Builder;",
            &[JValue::Object(&pads.into())],
        )
        .map_err(jni_err)?;

        // .setKeySize(256)
        env.call_method(
            &builder,
            "setKeySize",
            "(I)Landroid/security/keystore/KeyGenParameterSpec$Builder;",
            &[JValue::Int(256)],
        )
        .map_err(jni_err)?;

        // .build()
        let spec = env
            .call_method(
                &builder,
                "build",
                "()Landroid/security/keystore/KeyGenParameterSpec;",
                &[],
            )
            .map_err(jni_err)?
            .l()
            .map_err(jni_err)?;

        // kg.init(spec); kg.generateKey();
        env.call_method(
            &kg,
            "init",
            "(Ljava/security/spec/AlgorithmParameterSpec;)V",
            &[JValue::Object(&spec)],
        )
        .map_err(jni_err)?;
        env.call_method(&kg, "generateKey", "()Ljavax/crypto/SecretKey;", &[])
            .map_err(jni_err)?;

        Ok(())
    }

    /// Load the master key reference from Android KeyStore.
    fn get_master_key<'a>(env: &mut JNIEnv<'a>) -> Result<JObject<'a>, String> {
        let ks_type = jstr(env, "AndroidKeyStore")?;
        let ks = env
            .call_static_method(
                "java/security/KeyStore",
                "getInstance",
                "(Ljava/lang/String;)Ljava/security/KeyStore;",
                &[JValue::Object(&ks_type)],
            )
            .map_err(jni_err)?
            .l()
            .map_err(jni_err)?;
        env.call_method(
            &ks,
            "load",
            "(Ljava/security/KeyStore$LoadStoreParameter;)V",
            &[JValue::Object(&JObject::null())],
        )
        .map_err(jni_err)?;

        let alias = jstr(env, KEYSTORE_ALIAS)?;
        env.call_method(
            &ks,
            "getKey",
            "(Ljava/lang/String;[C)Ljava/security/Key;",
            &[JValue::Object(&alias), JValue::Object(&JObject::null())],
        )
        .map_err(jni_err)?
        .l()
        .map_err(jni_err)
    }

    /// Encrypt data with the KeyStore master key. Returns IV (12) + ciphertext.
    fn encrypt(env: &mut JNIEnv, data: &[u8]) -> Result<Vec<u8>, String> {
        let key = get_master_key(env)?;
        let transform = jstr(env, "AES/GCM/NoPadding")?;
        let cipher = env
            .call_static_method(
                "javax/crypto/Cipher",
                "getInstance",
                "(Ljava/lang/String;)Ljavax/crypto/Cipher;",
                &[JValue::Object(&transform)],
            )
            .map_err(jni_err)?
            .l()
            .map_err(jni_err)?;

        // cipher.init(Cipher.ENCRYPT_MODE, key)
        env.call_method(
            &cipher,
            "init",
            "(ILjava/security/Key;)V",
            &[JValue::Int(1), JValue::Object(&key)],
        )
        .map_err(jni_err)?;

        // byte[] iv = cipher.getIV()
        let iv_obj = env
            .call_method(&cipher, "getIV", "()[B", &[])
            .map_err(jni_err)?
            .l()
            .map_err(jni_err)?;
        let iv_arr: JByteArray = iv_obj.into();
        let iv = env.convert_byte_array(iv_arr).map_err(jni_err)?;

        // byte[] encrypted = cipher.doFinal(data)
        let data_arr = env.byte_array_from_slice(data).map_err(jni_err)?;
        let ct_obj = env
            .call_method(
                &cipher,
                "doFinal",
                "([B)[B",
                &[JValue::Object(&data_arr.into())],
            )
            .map_err(jni_err)?
            .l()
            .map_err(jni_err)?;
        let ct_arr: JByteArray = ct_obj.into();
        let ct = env.convert_byte_array(ct_arr).map_err(jni_err)?;

        let mut out = Vec::with_capacity(iv.len() + ct.len());
        out.extend_from_slice(&iv);
        out.extend_from_slice(&ct);
        Ok(out)
    }

    /// Decrypt data with the KeyStore master key. Input: IV (12) + ciphertext.
    fn decrypt(env: &mut JNIEnv, data: &[u8]) -> Result<Vec<u8>, String> {
        if data.len() < 28 {
            // 12-byte IV + 16-byte GCM tag minimum
            return Err("Wrapped key data too short".to_string());
        }
        let (iv, ciphertext) = data.split_at(12);

        let key = get_master_key(env)?;
        let transform = jstr(env, "AES/GCM/NoPadding")?;
        let cipher = env
            .call_static_method(
                "javax/crypto/Cipher",
                "getInstance",
                "(Ljava/lang/String;)Ljavax/crypto/Cipher;",
                &[JValue::Object(&transform)],
            )
            .map_err(jni_err)?
            .l()
            .map_err(jni_err)?;

        // GCMParameterSpec(128, iv)
        let iv_arr = env.byte_array_from_slice(iv).map_err(jni_err)?;
        let gcm_spec = env
            .new_object(
                "javax/crypto/spec/GCMParameterSpec",
                "(I[B)V",
                &[JValue::Int(128), JValue::Object(&iv_arr.into())],
            )
            .map_err(jni_err)?;

        // cipher.init(Cipher.DECRYPT_MODE, key, gcmSpec)
        env.call_method(
            &cipher,
            "init",
            "(ILjava/security/Key;Ljava/security/spec/AlgorithmParameterSpec;)V",
            &[
                JValue::Int(2),
                JValue::Object(&key),
                JValue::Object(&gcm_spec),
            ],
        )
        .map_err(jni_err)?;

        // byte[] decrypted = cipher.doFinal(ciphertext)
        let ct_arr = env.byte_array_from_slice(ciphertext).map_err(jni_err)?;
        let pt_obj = env
            .call_method(
                &cipher,
                "doFinal",
                "([B)[B",
                &[JValue::Object(&ct_arr.into())],
            )
            .map_err(jni_err)?
            .l()
            .map_err(jni_err)?;
        let pt_arr: JByteArray = pt_obj.into();
        env.convert_byte_array(pt_arr).map_err(jni_err)
    }

    /// Store a Base64-encoded value in SharedPreferences.
    fn prefs_put(
        env: &mut JNIEnv,
        context: &JObject,
        key: &str,
        data: &[u8],
    ) -> Result<(), String> {
        let encoded = data_encoding::BASE64.encode(data);

        let prefs_name = jstr(env, PREFS_NAME)?;
        let prefs = env
            .call_method(
                context,
                "getSharedPreferences",
                "(Ljava/lang/String;I)Landroid/content/SharedPreferences;",
                &[JValue::Object(&prefs_name), JValue::Int(0)],
            )
            .map_err(jni_err)?
            .l()
            .map_err(jni_err)?;

        let editor = env
            .call_method(
                &prefs,
                "edit",
                "()Landroid/content/SharedPreferences$Editor;",
                &[],
            )
            .map_err(jni_err)?
            .l()
            .map_err(jni_err)?;

        let k = jstr(env, key)?;
        let v = jstr(env, &encoded)?;
        env.call_method(
            &editor,
            "putString",
            "(Ljava/lang/String;Ljava/lang/String;)Landroid/content/SharedPreferences$Editor;",
            &[JValue::Object(&k), JValue::Object(&v)],
        )
        .map_err(jni_err)?;

        env.call_method(&editor, "apply", "()V", &[])
            .map_err(jni_err)?;
        Ok(())
    }

    /// Load a Base64-encoded value from SharedPreferences.
    fn prefs_get(
        env: &mut JNIEnv,
        context: &JObject,
        key: &str,
    ) -> Result<Option<Vec<u8>>, String> {
        let prefs_name = jstr(env, PREFS_NAME)?;
        let prefs = env
            .call_method(
                context,
                "getSharedPreferences",
                "(Ljava/lang/String;I)Landroid/content/SharedPreferences;",
                &[JValue::Object(&prefs_name), JValue::Int(0)],
            )
            .map_err(jni_err)?
            .l()
            .map_err(jni_err)?;

        let k = jstr(env, key)?;
        let result = env
            .call_method(
                &prefs,
                "getString",
                "(Ljava/lang/String;Ljava/lang/String;)Ljava/lang/String;",
                &[JValue::Object(&k), JValue::Object(&JObject::null())],
            )
            .map_err(jni_err)?
            .l()
            .map_err(jni_err)?;

        if result.is_null() {
            return Ok(None);
        }

        let jstr_ref: jni::objects::JString = result.into();
        let encoded: String = env.get_string(&jstr_ref).map_err(jni_err)?.into();
        let bytes = data_encoding::BASE64
            .decode(encoded.as_bytes())
            .map_err(|e| format!("Base64 decode error: {e}"))?;
        Ok(Some(bytes))
    }

    /// Remove a key from SharedPreferences.
    fn prefs_remove(env: &mut JNIEnv, context: &JObject, key: &str) -> Result<(), String> {
        let prefs_name = jstr(env, PREFS_NAME)?;
        let prefs = env
            .call_method(
                context,
                "getSharedPreferences",
                "(Ljava/lang/String;I)Landroid/content/SharedPreferences;",
                &[JValue::Object(&prefs_name), JValue::Int(0)],
            )
            .map_err(jni_err)?
            .l()
            .map_err(jni_err)?;

        let editor = env
            .call_method(
                &prefs,
                "edit",
                "()Landroid/content/SharedPreferences$Editor;",
                &[],
            )
            .map_err(jni_err)?
            .l()
            .map_err(jni_err)?;

        let k = jstr(env, key)?;
        env.call_method(
            &editor,
            "remove",
            "(Ljava/lang/String;)Landroid/content/SharedPreferences$Editor;",
            &[JValue::Object(&k)],
        )
        .map_err(jni_err)?;

        env.call_method(&editor, "apply", "()V", &[])
            .map_err(jni_err)?;
        Ok(())
    }

    // ── Public API ──────────────────────────────────────────────────

    pub fn load() -> Result<[u8; 32], String> {
        with_jni(|env, context| {
            ensure_master_key(env)?;
            let wrapped = prefs_get(env, context, PREFS_KEY)?
                .ok_or_else(|| "No wrapped key stored".to_string())?;
            let raw = decrypt(env, &wrapped)?;
            if raw.len() != 32 {
                return Err("Decrypted key has wrong length".to_string());
            }
            let mut key = [0u8; 32];
            key.copy_from_slice(&raw);
            Ok(key)
        })
    }

    pub fn store(key: &[u8; 32]) -> Result<(), String> {
        with_jni(|env, context| {
            ensure_master_key(env)?;
            let wrapped = encrypt(env, key)?;
            prefs_put(env, context, PREFS_KEY, &wrapped)
        })
    }

    pub fn delete() -> Result<(), String> {
        with_jni(|env, context| {
            prefs_remove(env, context, PREFS_KEY)?;

            // Also delete the master key from Android KeyStore
            let ks_type = jstr(env, "AndroidKeyStore")?;
            let ks = env
                .call_static_method(
                    "java/security/KeyStore",
                    "getInstance",
                    "(Ljava/lang/String;)Ljava/security/KeyStore;",
                    &[JValue::Object(&ks_type)],
                )
                .map_err(jni_err)?
                .l()
                .map_err(jni_err)?;
            env.call_method(
                &ks,
                "load",
                "(Ljava/security/KeyStore$LoadStoreParameter;)V",
                &[JValue::Object(&JObject::null())],
            )
            .map_err(jni_err)?;

            let alias = jstr(env, KEYSTORE_ALIAS)?;
            env.call_method(
                &ks,
                "deleteEntry",
                "(Ljava/lang/String;)V",
                &[JValue::Object(&alias)],
            )
            .map_err(jni_err)?;
            Ok(())
        })
    }
}
