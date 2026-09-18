//! Boot-time index of the animations stored in the bundle partition.
//!
//! The bundle is a plain concatenation of PXA files (see `doc/PXA_spec.md`
//! section 4). No directory is stored: every file's size is derived from its
//! header, so the catalog simply walks the files until the magic stops
//! matching. This module is hardware-free; it only needs a [`ByteSource`].

use pxa_core::{HEADER_SIZE, Header, MAGIC};

use crate::storage::{ByteSource, StorageError, check_range};

/// Upper bound on the number of animations the firmware keeps track of.
/// Bounded by the playlist, which stores indices as `u8`.
pub const MAX_ANIMATIONS: usize = 256;

/// One animation of the bundle.
#[derive(Clone, Copy, Debug)]
pub struct Entry {
    /// Offset of the file header from the start of the source.
    pub offset: u32,
    /// Complete file size, derived from the header.
    pub len: u32,
    /// Validated file header.
    pub header: Header,
}

/// Why the scan stopped. Only the first three are normal.
#[derive(Clone, Copy, Debug)]
pub enum ScanEnd {
    /// An end marker (zeros or erased flash) follows the last file.
    Terminator,
    /// There is no room for another header before the end of the source.
    EndOfSource,
    /// [`MAX_ANIMATIONS`] entries were found; the rest is ignored.
    Full,
    /// Bytes without the PXA magic that are not an end marker.
    Garbage { offset: u32 },
    /// A header with the right magic but invalid fields.
    InvalidHeader { offset: u32, error: pxa_core::Error },
    /// A file whose declared size runs past the end of the source.
    Truncated { offset: u32, size: u64 },
    /// The source could not be read.
    Storage(StorageError),
}

impl defmt::Format for ScanEnd {
    fn format(&self, f: defmt::Formatter<'_>) {
        match self {
            Self::Terminator => defmt::write!(f, "end marker"),
            Self::EndOfSource => defmt::write!(f, "end of partition"),
            Self::Full => defmt::write!(f, "catalog full ({} entries)", MAX_ANIMATIONS),
            Self::Garbage { offset } => defmt::write!(f, "unexpected bytes at {=u32:#x}", offset),
            Self::InvalidHeader { offset, error } => defmt::write!(
                f,
                "invalid header at {=u32:#x}: {}",
                offset,
                defmt::Display2Format(error)
            ),
            Self::Truncated { offset, size } => defmt::write!(
                f,
                "file at {=u32:#x} declares {=u64} bytes past the end",
                offset,
                size
            ),
            Self::Storage(error) => defmt::write!(f, "read error: {}", error),
        }
    }
}

/// The animations found in a bundle, in storage order.
pub struct Catalog {
    entries: [Option<Entry>; MAX_ANIMATIONS],
    count: usize,
}

impl Catalog {
    /// Walks the bundle from offset 0 and records every file header.
    ///
    /// Headers are validated by `pxa-core`; frames are only checked when
    /// played. The scan stops at the first anomaly, which is reported so the
    /// caller can log it.
    pub fn scan<S: ByteSource>(source: &mut S) -> (Self, ScanEnd) {
        let mut catalog = Self {
            entries: [None; MAX_ANIMATIONS],
            count: 0,
        };
        let mut offset = 0u32;

        let end = loop {
            if catalog.count == MAX_ANIMATIONS {
                break ScanEnd::Full;
            }
            if check_range(offset, HEADER_SIZE, source.len()).is_err() {
                break ScanEnd::EndOfSource;
            }

            let mut bytes = [0u8; HEADER_SIZE];
            if let Err(error) = source.read(offset, &mut bytes) {
                break ScanEnd::Storage(error);
            }
            let magic = &bytes[..MAGIC.len()];
            if magic != MAGIC {
                let blank = magic.iter().all(|b| *b == 0x00) || magic.iter().all(|b| *b == 0xFF);
                break if blank {
                    ScanEnd::Terminator
                } else {
                    ScanEnd::Garbage { offset }
                };
            }

            let header = match Header::parse(&bytes) {
                Ok(header) => header,
                Err(error) => break ScanEnd::InvalidHeader { offset, error },
            };
            let size = match header.file_size() {
                Ok(size) => size,
                Err(error) => break ScanEnd::InvalidHeader { offset, error },
            };
            let fits = usize::try_from(size)
                .is_ok_and(|len| check_range(offset, len, source.len()).is_ok());
            if !fits {
                break ScanEnd::Truncated { offset, size };
            }
            // `size` fits in the source, so it fits in `u32`.
            let len = size as u32;

            catalog.entries[catalog.count] = Some(Entry {
                offset,
                len,
                header,
            });
            catalog.count += 1;
            offset += len;
        };

        (catalog, end)
    }

    pub fn len(&self) -> usize {
        self.count
    }

    pub fn is_empty(&self) -> bool {
        self.count == 0
    }

    pub fn get(&self, index: usize) -> Option<&Entry> {
        self.entries.get(index)?.as_ref()
    }
}
