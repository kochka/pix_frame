//! Section 12 of `doc/PXA_spec.md`.

use pxa_core::{BitsPerPixel, FrameError, Unpacker, pack, unpack, validate_payload};

fn pack_vec(indices: &[u8], bpp: BitsPerPixel, palette_size: u16) -> Vec<u8> {
    let mut out = vec![0xFF; 64];
    let len = pack(indices, bpp, palette_size, &mut out).unwrap();
    out.truncate(len);
    out
}

#[test]
fn bits_per_pixel_from_palette_size() {
    assert_eq!(BitsPerPixel::from_palette_size(0), None);
    assert_eq!(BitsPerPixel::from_palette_size(1), Some(BitsPerPixel::One));
    assert_eq!(BitsPerPixel::from_palette_size(2), Some(BitsPerPixel::One));
    assert_eq!(BitsPerPixel::from_palette_size(3), Some(BitsPerPixel::Two));
    assert_eq!(BitsPerPixel::from_palette_size(4), Some(BitsPerPixel::Two));
    assert_eq!(BitsPerPixel::from_palette_size(5), Some(BitsPerPixel::Four));
    assert_eq!(
        BitsPerPixel::from_palette_size(16),
        Some(BitsPerPixel::Four)
    );
    assert_eq!(
        BitsPerPixel::from_palette_size(17),
        Some(BitsPerPixel::Eight)
    );
    assert_eq!(
        BitsPerPixel::from_palette_size(256),
        Some(BitsPerPixel::Eight)
    );
    assert_eq!(BitsPerPixel::from_palette_size(257), None);
}

#[test]
fn packed_len_matches_section_14() {
    assert_eq!(BitsPerPixel::One.packed_len(256), Some(32));
    assert_eq!(BitsPerPixel::Two.packed_len(256), Some(64));
    assert_eq!(BitsPerPixel::Four.packed_len(256), Some(128));
    assert_eq!(BitsPerPixel::Eight.packed_len(256), Some(256));
    assert_eq!(BitsPerPixel::Four.packed_len(2048), Some(1024));
    assert_eq!(BitsPerPixel::One.packed_len(5), Some(1));
    assert_eq!(BitsPerPixel::One.packed_len(9), Some(2));
    assert_eq!(BitsPerPixel::Eight.packed_len(u32::MAX), Some(u32::MAX));
    // Largest PXA frame: 65535 × 65535 at 8 bpp still fits the u32 field.
    assert_eq!(
        BitsPerPixel::Eight.packed_len(65535 * 65535),
        Some(65535 * 65535)
    );
}

#[test]
fn packs_8bpp() {
    assert_eq!(
        pack_vec(&[3, 12, 128, 255], BitsPerPixel::Eight, 256),
        [0x03, 0x0C, 0x80, 0xFF]
    );
}

#[test]
fn packs_4bpp() {
    assert_eq!(pack_vec(&[3, 12], BitsPerPixel::Four, 16), [0x3C]);
    assert_eq!(
        pack_vec(&[3, 12, 1, 7, 2, 2, 0, 15], BitsPerPixel::Four, 16),
        [0x3C, 0x17, 0x22, 0x0F]
    );
}

#[test]
fn packs_2bpp() {
    assert_eq!(pack_vec(&[0, 1, 2, 3], BitsPerPixel::Two, 4), [0x1B]);
}

#[test]
fn packs_1bpp() {
    assert_eq!(
        pack_vec(&[1, 0, 1, 1, 0, 0, 1, 0], BitsPerPixel::One, 2),
        [0xB2]
    );
}

#[test]
fn packs_final_byte_padding_as_zero() {
    assert_eq!(pack_vec(&[1, 0, 1, 1, 0], BitsPerPixel::One, 2), [0xB0]);
    assert_eq!(pack_vec(&[3, 1, 2], BitsPerPixel::Two, 4), [0b1101_1000]);
    assert_eq!(pack_vec(&[9], BitsPerPixel::Four, 16), [0x90]);
}

#[test]
fn pack_rejects_index_outside_palette() {
    let mut out = [0u8; 8];
    // 10 colors use 4 bpp, but indices 10..=15 are invalid.
    assert_eq!(
        pack(&[0, 9, 10], BitsPerPixel::Four, 10, &mut out),
        Err(FrameError::InvalidPaletteIndex {
            pixel: 2,
            index: 10
        })
    );
}

#[test]
fn pack_rejects_small_output_buffer() {
    let mut out = [0u8; 1];
    assert!(pack(&[0; 9], BitsPerPixel::One, 2, &mut out).is_err());
}

#[test]
fn unpack_roundtrips_every_depth() {
    for (bpp, palette_size) in [
        (BitsPerPixel::One, 2),
        (BitsPerPixel::Two, 4),
        (BitsPerPixel::Four, 16),
        (BitsPerPixel::Eight, 256),
    ] {
        // 37 pixels: not a multiple of any pixels-per-byte value except 1.
        let indices: Vec<u8> = (0..37u32)
            .map(|i| ((i * 7 + 3) % u32::from(palette_size)) as u8)
            .collect();
        let packed = pack_vec(&indices, bpp, palette_size);
        assert_eq!(packed.len() as u32, bpp.packed_len(37).unwrap());

        let mut out = vec![0xFF; 37];
        unpack(&packed, bpp, 37, palette_size, &mut out).unwrap();
        assert_eq!(out, indices, "{bpp:?}");

        let iter: Vec<u8> = Unpacker::new(&packed, bpp, 37).collect();
        assert_eq!(iter, indices);
        assert_eq!(Unpacker::new(&packed, bpp, 37).len(), 37);
    }
}

#[test]
fn unpack_rejects_index_outside_palette() {
    // 4 bpp, 10 colors, second pixel is 10.
    assert_eq!(
        validate_payload(&[0x1A], BitsPerPixel::Four, 2, 10),
        Err(FrameError::InvalidPaletteIndex {
            pixel: 1,
            index: 10
        })
    );
}

#[test]
fn unpack_rejects_non_zero_padding() {
    assert_eq!(
        validate_payload(&[0xB1], BitsPerPixel::One, 5, 2),
        Err(FrameError::NonZeroPadding)
    );
    assert_eq!(
        validate_payload(&[0b1101_1001], BitsPerPixel::Two, 3, 4),
        Err(FrameError::NonZeroPadding)
    );
    assert_eq!(
        validate_payload(&[0x91], BitsPerPixel::Four, 1, 16),
        Err(FrameError::NonZeroPadding)
    );
    // No padding at 8 bpp or when the last byte is full.
    assert_eq!(
        validate_payload(&[0xFF], BitsPerPixel::Eight, 1, 256),
        Ok(())
    );
    assert_eq!(validate_payload(&[0xFF], BitsPerPixel::One, 8, 2), Ok(()));
}

#[test]
fn unpack_rejects_wrong_payload_length() {
    assert_eq!(
        validate_payload(&[0x00], BitsPerPixel::One, 9, 2),
        Err(FrameError::TruncatedPayload {
            expected: 2,
            actual: 1
        })
    );
    assert!(validate_payload(&[0x00, 0x00], BitsPerPixel::One, 8, 2).is_err());
}

#[test]
fn unpack_rejects_small_output_buffer() {
    let mut out = [0u8; 3];
    assert!(unpack(&[0x00], BitsPerPixel::One, 4, 2, &mut out).is_err());
}

#[test]
fn unpacker_never_reads_out_of_bounds() {
    let unpacker = Unpacker::new(&[], BitsPerPixel::Eight, 3);
    assert_eq!(unpacker.collect::<Vec<_>>(), [0, 0, 0]);
    assert_eq!(Unpacker::new(&[0xAB], BitsPerPixel::Eight, 1).get(1), None);
}
