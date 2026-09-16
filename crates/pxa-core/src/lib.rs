#![no_std]
#![forbid(unsafe_code)]

//! Shared, allocation-free primitives for the PXA animation format.
//!
//! This crate implements the binary rules of `doc/PXA_spec.md` (PXA v1) and
//! is the single source of truth for both the ESP32 firmware and the desktop
//! converter. It never allocates, never touches the file system, and never
//! panics on malformed input.
//!
//! The main entry points are:
//!
//! - [`Header`] and [`FrameHeader`] for the fixed-size headers;
//! - [`BitsPerPixel`] for the palette-size to bit-depth rule;
//! - [`pack`], [`unpack`], [`validate_payload`] and [`Unpacker`] for the
//!   `RAW_INDEXED` pixel encoding;
//! - [`PxaFile`] for validating and reading a complete file held in memory;
//! - [`write`] and [`encoded_size`] for producing a file into a caller-provided
//!   buffer.

mod bpp;
mod error;
mod file;
mod header;
mod packing;
mod writer;

pub use bpp::BitsPerPixel;
pub use error::{Error, FrameError};
pub use file::{Frame, Frames, Palette, PxaFile};
pub use header::{FrameHeader, Header};
pub use packing::{Unpacker, pack, unpack, validate_payload};
pub use writer::{FrameSource, encoded_size, write};

/// First four bytes of every PXA v1 file.
pub const MAGIC: [u8; 4] = *b"PXA1";

/// Size in bytes of the file header.
pub const HEADER_SIZE: usize = 16;

/// Size in bytes of every frame header.
pub const FRAME_HEADER_SIZE: usize = 8;

/// Largest palette allowed by PXA v1.
pub const MAX_PALETTE_SIZE: u16 = 256;

/// Frame encoding identifier for packed palette indices.
pub const ENCODING_RAW_INDEXED: u8 = 0x00;

/// One palette entry, in R, G, B order.
pub type Rgb = [u8; 3];
