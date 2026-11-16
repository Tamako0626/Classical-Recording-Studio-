//! 古典录音工作台.

#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

fn main() {
    std::panic::set_hook(Box::new(|info| {
        let dir = classical_recording_studio::store::app_dir();
        let _ = std::fs::create_dir_all(&dir);
        let _ = std::fs::write(dir.join("panic.log"), format!("{info}\n"));
        eprintln!("{info}");
    }));
    classical_recording_studio::gui::run();
}
