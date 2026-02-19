#![cfg(mobile)]

use tauri::{
    plugin::{Builder, PluginHandle, TauriPlugin},
    Manager, Runtime,
};

#[cfg(target_os = "ios")]
tauri::ios_plugin_binding!(init_plugin_share_file);

pub struct ShareFile<R: Runtime>(PluginHandle<R>);

impl<R: Runtime> ShareFile<R> {
    pub fn share_file(&self, path: &str, mime_type: &str) -> Result<(), String> {
        self.0
            .run_mobile_plugin::<serde_json::Value>(
                "shareFile",
                serde_json::json!({
                    "path": path,
                    "mimeType": mime_type
                }),
            )
            .map_err(|e| e.to_string())?;
        Ok(())
    }
}

pub trait ShareFileExt<R: Runtime> {
    fn share_file_plugin(&self) -> &ShareFile<R>;
}

impl<R: Runtime, T: Manager<R>> ShareFileExt<R> for T {
    fn share_file_plugin(&self) -> &ShareFile<R> {
        self.state::<ShareFile<R>>().inner()
    }
}

pub fn init<R: Runtime>() -> TauriPlugin<R> {
    Builder::new("share-file")
        .setup(|app, api| {
            #[cfg(target_os = "ios")]
            let handle = api.register_ios_plugin(init_plugin_share_file)?;
            #[cfg(target_os = "android")]
            let handle = api.register_android_plugin("com.kestrel.ghostauth", "ShareFilePlugin")?;
            app.manage(ShareFile(handle));
            Ok(())
        })
        .build()
}
