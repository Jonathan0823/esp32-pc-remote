use eframe::{egui, icon_data};

pub fn settings_icon() -> egui::IconData {
    icon_data::from_png_bytes(include_bytes!("../../assets/app.png"))
        .expect("embedded application icon must be valid PNG")
}

pub fn tray_icon() -> Result<tray_icon::Icon, String> {
    let icon = icon_data::from_png_bytes(include_bytes!("../../assets/tray.png"))
        .map_err(|error| error.to_string())?;
    tray_icon::Icon::from_rgba(icon.rgba, icon.width, icon.height)
        .map_err(|error| error.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn embedded_icons_have_transparency_and_expected_dimensions() {
        for (bytes, size) in [
            (include_bytes!("../../assets/app.png").as_slice(), 256),
            (include_bytes!("../../assets/tray.png").as_slice(), 32),
        ] {
            let icon = icon_data::from_png_bytes(bytes).unwrap();
            assert_eq!((icon.width, icon.height), (size, size));
            assert_eq!(icon.rgba.len(), (size * size * 4) as usize);
            assert!(icon.rgba.chunks_exact(4).any(|pixel| pixel[3] == 0));
            assert!(icon.rgba.chunks_exact(4).any(|pixel| pixel[3] == 255));
        }
    }
}
