//! `pxa inspect`: human-readable dump of a PXA file.

use std::path::Path;

use anyhow::{Result, anyhow};
use pxa_core::{Header, PxaFile};

pub fn inspect(path: &Path, bytes: &[u8]) -> Result<()> {
    println!("file:        {} ({} bytes)", path.display(), bytes.len());

    // Show what the header claims even when the rest of the file is invalid.
    if let Ok(header) = Header::parse(bytes) {
        print_header(&header);
    }

    let file = PxaFile::parse(bytes).map_err(|e| anyhow!("invalid PXA file: {e}"))?;

    println!("palette:");
    for (index, [r, g, b]) in file.palette().iter().enumerate() {
        println!("  {index:3}: #{r:02X}{g:02X}{b:02X}");
    }

    println!("frames:");
    let mut total_ms = 0u64;
    for frame in file.frames() {
        println!(
            "  {:4}: {:5} ms, {} payload bytes",
            frame.index(),
            frame.duration_ms(),
            frame.payload().len()
        );
        total_ms += u64::from(frame.duration_ms());
    }
    println!("total:       {total_ms} ms per loop");
    Ok(())
}

fn print_header(header: &Header) {
    println!("magic:       PXA1");
    println!("size:        {}x{}", header.width(), header.height());
    println!("frames:      {}", header.frame_count());
    println!("palette:     {} colors", header.palette_size());
    println!("depth:       {} bpp", header.bits_per_pixel().bits());
    match header.payload_size() {
        Ok(size) => println!("payload:     {size} bytes per frame"),
        Err(e) => println!("payload:     {e}"),
    }
    match header.file_size() {
        Ok(size) => println!("expected:    {size} bytes"),
        Err(e) => println!("expected:    {e}"),
    }
}
