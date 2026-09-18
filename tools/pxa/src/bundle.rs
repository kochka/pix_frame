//! `pxa bundle`: builds the raw image of the animations partition.
//!
//! The bundle is the concatenation of complete, validated PXA files followed
//! by a four-byte end marker. The firmware derives each file's size from its
//! header, so no directory is needed. See `doc/PXA_spec.md` section 4.

use std::path::Path;

use anyhow::{Context, Result, bail};
use pxa_core::PxaFile;

/// Written after the last file so the reader stops even when the partition
/// still holds bytes from a previous, larger bundle.
pub const END_MARKER: [u8; 4] = [0; 4];

/// One file placed in the bundle.
pub struct Placement {
    pub offset: usize,
    pub len: usize,
    pub width: u16,
    pub height: u16,
    pub frame_count: u32,
}

/// Validates every input and concatenates them; any invalid file aborts.
pub fn bundle(inputs: &[impl AsRef<Path>]) -> Result<(Vec<u8>, Vec<Placement>)> {
    let mut image = Vec::new();
    let mut placements = Vec::with_capacity(inputs.len());

    for input in inputs {
        let input = input.as_ref();
        let bytes =
            std::fs::read(input).with_context(|| format!("cannot read {}", input.display()))?;
        let file = PxaFile::parse(&bytes)
            .map_err(|e| anyhow::anyhow!("{}: invalid PXA file: {e}", input.display()))?;
        let header = file.header();
        placements.push(Placement {
            offset: image.len(),
            len: bytes.len(),
            width: header.width(),
            height: header.height(),
            frame_count: header.frame_count(),
        });
        image.extend_from_slice(&bytes);
    }

    if placements.is_empty() {
        bail!("no input files");
    }
    image.extend_from_slice(&END_MARKER);
    Ok((image, placements))
}
