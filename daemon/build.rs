fn main() {
    println!("cargo:rerun-if-changed=assets/app.ico");
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("windows") {
        winresource::WindowsResource::new()
            .set_icon("assets/app.ico")
            .set("ProductName", "ESP32 Media Daemon")
            .set("FileDescription", "ESP32 Media Daemon")
            .compile_for("esp32-media-daemon")
            .expect("compile Windows application icon resource");
    }
}
