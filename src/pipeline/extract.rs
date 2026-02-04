use std::path::Path;

use anyhow::Result;
use palette::Lab;

/// A color extracted from the image with its cluster weight.
#[derive(Debug, Clone)]
pub struct ExtractedColor {
    pub lab: Lab,
    pub weight: f32,
}

/// Load an image, resize to 256x256, and convert pixels to LAB space.
pub fn load_and_prepare(_path: &Path) -> Result<Vec<Lab>> {
    todo!("Ticket 3: image loading and pre-processing")
}

/// Run K-means on LAB pixels to extract dominant colors.
pub fn extract_colors(_pixels: &[Lab], _k: usize) -> Vec<ExtractedColor> {
    todo!("Ticket 4: K-means color extraction")
}
