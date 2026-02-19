const COMMANDS: &[&str] = &["share_file"];

fn main() {
    tauri_plugin::Builder::new(COMMANDS)
        .ios_path("ios")
        .try_build()
        .unwrap();
}
