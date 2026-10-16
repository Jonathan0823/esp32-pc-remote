#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

#[cfg(windows)]
fn main() -> Result<(), Box<dyn std::error::Error>> {
    media_daemon::adapters::windows_app::run()
}

#[cfg(not(windows))]
fn main() {
    eprintln!("esp32-media-daemon is a Windows 11 x64 application");
}
