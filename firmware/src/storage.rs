//! Read-only random access to the bytes holding the animation bundle.
//!
//! The player and catalog only depend on the [`ByteSource`] trait so they
//! stay free of hardware details. [`FlashPartition`] is the ESP32
//! implementation backed by a partition of the SPI flash.

use esp_storage::FlashStorage;

/// Failure while reading from a [`ByteSource`].
#[derive(Clone, Copy, Debug, PartialEq, Eq, defmt::Format)]
pub enum StorageError {
    /// The requested range does not fit in the source.
    OutOfBounds { offset: u32, len: usize },
    /// The underlying device reported an error.
    Device,
}

/// A bounded, seekable, read-only byte store.
pub trait ByteSource {
    /// Total number of readable bytes.
    fn len(&self) -> u32;

    /// Fills `buf` with the bytes starting at `offset`.
    ///
    /// Fails, without touching the device, when the range exceeds
    /// [`ByteSource::len`].
    fn read(&mut self, offset: u32, buf: &mut [u8]) -> Result<(), StorageError>;

    fn is_empty(&self) -> bool {
        self.len() == 0
    }
}

/// Checks that `offset..offset + len` lies within `capacity`.
pub fn check_range(offset: u32, len: usize, capacity: u32) -> Result<(), StorageError> {
    let end = u32::try_from(len)
        .ok()
        .and_then(|len| offset.checked_add(len));
    match end {
        Some(end) if end <= capacity => Ok(()),
        _ => Err(StorageError::OutOfBounds { offset, len }),
    }
}

/// A window of the SPI flash, typically one partition of the partition table.
pub struct FlashPartition<'a, 'd> {
    flash: &'a mut FlashStorage<'d>,
    offset: u32,
    len: u32,
}

impl<'a, 'd> FlashPartition<'a, 'd> {
    /// Wraps `len` bytes of flash starting at absolute `offset`.
    ///
    /// Fails when the window exceeds the flash capacity.
    pub fn new(
        flash: &'a mut FlashStorage<'d>,
        offset: u32,
        len: u32,
    ) -> Result<Self, StorageError> {
        let capacity = u32::try_from(flash.capacity()).unwrap_or(u32::MAX);
        check_range(offset, len as usize, capacity)?;
        Ok(Self { flash, offset, len })
    }
}

impl ByteSource for FlashPartition<'_, '_> {
    fn len(&self) -> u32 {
        self.len
    }

    fn read(&mut self, offset: u32, buf: &mut [u8]) -> Result<(), StorageError> {
        check_range(offset, buf.len(), self.len)?;
        // Cannot overflow: `offset + buf.len() <= len` and `self.offset + len`
        // was checked against the flash capacity in `new`.
        self.flash
            .read(self.offset + offset, buf)
            .map_err(|_| StorageError::Device)
    }
}

/// Adapter giving `esp-nvs` access to the flash through the `embedded-storage`
/// NOR traits, with the CRC32 computed in software.
pub struct NvsFlash<'a, 'd>(pub &'a mut FlashStorage<'d>);

impl embedded_storage::nor_flash::ErrorType for NvsFlash<'_, '_> {
    type Error = esp_storage::FlashStorageError;
}

impl embedded_storage::nor_flash::ReadNorFlash for NvsFlash<'_, '_> {
    // `FlashStorage::read` accepts any alignment, so advertise byte reads.
    const READ_SIZE: usize = 1;

    fn read(&mut self, offset: u32, bytes: &mut [u8]) -> Result<(), Self::Error> {
        self.0.read(offset, bytes)
    }

    fn capacity(&self) -> usize {
        self.0.capacity()
    }
}

impl embedded_storage::nor_flash::NorFlash for NvsFlash<'_, '_> {
    const WRITE_SIZE: usize = FlashStorage::WRITE_SIZE;
    const ERASE_SIZE: usize = FlashStorage::ERASE_SIZE;

    fn write(&mut self, offset: u32, bytes: &[u8]) -> Result<(), Self::Error> {
        self.0.write_nor(offset, bytes)
    }

    fn erase(&mut self, from: u32, to: u32) -> Result<(), Self::Error> {
        self.0.erase(from, to)
    }
}

impl esp_nvs::platform::Crc for NvsFlash<'_, '_> {
    fn crc32(init: u32, data: &[u8]) -> u32 {
        esp_nvs::platform::software_crc32(init, data)
    }
}
