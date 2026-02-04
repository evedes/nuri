use palette::Lab;

use crate::cli::ThemeMode;

/// Detect whether the image is predominantly dark or light.
pub fn detect_mode(_pixels: &[Lab]) -> ThemeMode {
    todo!("Ticket 5: dark/light mode detection")
}
