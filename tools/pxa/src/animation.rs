//! Common in-memory representation shared by every importer.
//!
//! Importers decode a source file into complete RGBA frames; the PXA
//! encoder never sees the source format.

use image::RgbaImage;

/// One complete source frame.
pub struct SourceFrame {
    /// Fully composited image, source composition and disposal already applied.
    pub image: RgbaImage,
    /// Source delay in milliseconds, or `None` for a static image with no timing.
    pub duration_ms: Option<u32>,
}

/// A decoded source animation.
pub struct Animation {
    pub width: u32,
    pub height: u32,
    pub frames: Vec<SourceFrame>,
}
