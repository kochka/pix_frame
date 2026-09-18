# PXA — Pixel Animation Format Specification

**Version:** 1.0 draft  
**File extension:** `.pxa`  
**Primary target:** RGB LED matrices driven by an ESP32  
**Current hardware:** ESP32-C6-WROOM-1-N8, 8 MB flash, 16×16 WS2815 matrix

---

## 1. Purpose

PXA is a small binary format for animated pixel art. It is designed to be:

- independent of a particular matrix size;
- compact and deterministic;
- trivial to decode on a microcontroller;
- streamable directly from flash with little RAM;
- independent of LED wiring and channel order;
- easy to generate from GIF, animated WebP, PNG spritesheets, Aseprite exports, or Divoom assets.

PXA is the deployment format. Common authoring formats are converted on a PC before being copied to the device:

```text
GIF / WebP / PNG / Aseprite / Divoom
                  ↓
             PXA compiler
                  ↓
            animation.pxa
                  ↓
             ESP32 player
```

The ESP32 does not need GIF, PNG, WebP, MP4, or Aseprite decoders.

---

## 2. Scope of PXA v1

PXA v1 supports:

- dimensions from 1×1 to 65,535×65,535, subject to payload and implementation limits;
- one global RGB888 palette containing 1 to 256 colors;
- complete, independent frames;
- palette indices packed at 1, 2, 4, or 8 bits per pixel;
- an independent duration for every frame;
- logical row-major pixel order;
- little-endian multi-byte integers;
- sequential streaming from a file or raw flash partition.

PXA v1 deliberately does not define:

- RLE or delta compression;
- local palettes per frame;
- transparency or alpha;
- embedded metadata;
- an embedded frame index;
- a loop count;
- playlists;
- checksums;
- a network upload protocol.

These can be added in a later version without complicating the first player.

---

## 3. Why PXA Instead of GIF on the ESP32

GIF remains a good interchange and authoring format. PXA is intended for playback.

GIF decoding requires support for LZW compression, image sub-rectangles, global and local palettes, transparency, disposal modes, and possibly interlacing. PXA frames have a fixed and predictable representation tailored to LED matrices.

| Property | GIF | PXA v1 |
| --- | --- | --- |
| Existing authoring tools | Excellent | Requires a converter |
| Compression | Often better | Predictable indexed packing |
| Decoder complexity | Relatively high | Very low |
| RAM and CPU use | Higher and data-dependent | Low and deterministic |
| Frame independence | Not guaranteed | Yes |
| Matrix-specific simplicity | No | Yes |
| Streaming from flash | Possible but complex | Direct |

PXA is not guaranteed to be smaller than GIF. Its primary advantage is simple, predictable decoding.

---

## 4. Storage on the ESP32

Animations are stored independently from the firmware in a dedicated data partition so they can be replaced without rebuilding the application.

### 4.1 Partition layout

The reference firmware uses the ESP-IDF partition table in `partitions.csv` (8 MB flash):

```text
name        type  subtype    offset    size
nvs         data  nvs        0x9000    0x6000      24 KiB, reserved
phy_init    data  phy        0xF000    0x1000      4 KiB, reserved
factory     app   factory    0x10000   0x200000    2 MiB firmware
(free)                       0x210000  0x200000    reserved for a future OTA slot and otadata
animations  data  undefined  0x410000  0x3F0000    3.94 MiB PXA bundle
```

The gap before `animations` keeps its offset stable if OTA is added later. The firmware locates the partition by its label at boot rather than by a compiled-in offset; the offset and size are still repeated in the `flash-assets` task of `.mise.toml`, which must be kept in sync with `partitions.csv`.

### 4.2 Bundle format

The `animations` partition holds a **bundle**: complete PXA files concatenated back to back, in the order they should be indexed, with no padding, alignment or directory, followed by an **end marker** of four `0x00` bytes.

```text
offset 0        PXA file 1
offset size_1   PXA file 2
...
offset Σ size   00 00 00 00
```

No directory is needed because every file's size follows from its header (section 15). A reader walks the bundle as follows:

1. If fewer than 16 bytes remain, stop.
2. Read the 16-byte header. If it does not start with `PXA1`, stop. Four `0x00` or four `0xFF` bytes (erased flash) are the normal end; anything else indicates a damaged bundle.
3. Validate the header and compute `file_size`. If the file would run past the end of the partition, stop.
4. Record `(offset, file_size)` and continue at `offset + file_size`.

Only headers are checked while indexing; frames are validated when played (section 18). The reference firmware indexes at most 256 files, ignores the rest and logs a warning when that happens. The end marker matters because a new, shorter bundle written over an old one would otherwise be followed by stale but well-formed files.

The bundle is produced by `pxa bundle` (section 17) and written to the partition with `espflash write-bin` after erasing it. LittleFS remains an option for a future version with per-file updates over USB or Wi-Fi; the firmware reads the bundle through a small byte-source abstraction so the storage backend can change without touching the player.

`include_bytes!()` remains suitable for a few built-in or recovery assets, but it is not the default storage strategy.

---

## 5. File Structure

A PXA v1 file contains:

```text
+--------------------------+
| File header (16 bytes)   |
+--------------------------+
| Global RGB888 palette    |
+--------------------------+
| Frame 0 header (8 bytes) |
+--------------------------+
| Frame 0 payload          |
+--------------------------+
| Frame 1 header (8 bytes) |
+--------------------------+
| Frame 1 payload          |
+--------------------------+
| ...                      |
+--------------------------+
```

There is no alignment padding between these sections.

---

## 6. Byte Order and Bit Order

PXA uses two separate conventions.

### 6.1 Multi-byte integer byte order

Every `u16` and `u32` is stored in **little-endian** order: the least-significant byte comes first.

```text
u16 value 0x1234  →  34 12
u32 value 0x12345678  →  78 56 34 12
```

Little-endian is native to ESP32 and common desktop CPUs. More importantly, declaring one byte order makes files portable and unambiguous. Readers should still decode fields explicitly, for example:

```rust
let duration_ms = u16::from_le_bytes([buf[0], buf[1]]);
```

A decoder should not cast arbitrary file bytes directly to a C or Rust structure because alignment, padding, and unaligned access rules may differ.

### 6.2 Packed pixel bit order

Pixel packing is not endianness. Within each byte, the first logical pixel occupies the most-significant available bits (**MSB-first**).

For example, at 4 bpp:

```text
pixel 0 = bits 7..4
pixel 1 = bits 3..0
```

Multi-byte integers are therefore little-endian while packed pixels are ordered MSB-first inside each byte.

---

## 7. File Header

The file header is exactly 16 bytes.

| Offset | Size | Type | Field | PXA v1 value or meaning |
| ---: | ---: | --- | --- | --- |
| `0x00` | 4 | bytes | `magic` | ASCII `PXA1` |
| `0x04` | 2 | `u16` | `width` | Logical width in pixels |
| `0x06` | 2 | `u16` | `height` | Logical height in pixels |
| `0x08` | 4 | `u32` | `frame_count` | Number of frames |
| `0x0C` | 2 | `u16` | `palette_size` | Number of RGB entries, 1–256 |
| `0x0E` | 2 | `u16` | `flags` | Must be `0x0000` in v1 |

Illustrative C layout:

```c
struct PxaHeader {
    char     magic[4];       // "PXA1"
    uint16_t width;
    uint16_t height;
    uint32_t frame_count;
    uint16_t palette_size;
    uint16_t flags;
};
```

The offset table, not the host compiler's structure layout, is authoritative.

### 7.1 Magic and versioning

The first four bytes are:

```text
50 58 41 31
 P  X  A  1
```

The major format version is encoded in the magic. A reader implementing this specification accepts `PXA1` and rejects other magic values.

### 7.2 Dimensions

`width` and `height` must both be non-zero. They describe a logical image and do not describe the physical wiring.

The 16-bit fields cover matrices such as:

```text
16×16
32×32
32×64
64×64
128×128
```

Implementations may enforce smaller limits according to available RAM, LED count, and driver capabilities.

### 7.3 Frame count

`frame_count` must be non-zero. Its type is `u32` to avoid an arbitrary 65,535-frame format limit, although players may impose a lower implementation limit.

### 7.4 Flags

All file flags are reserved in v1 and must be written as zero. A v1 reader must reject a file whose `flags` field is non-zero.

---

## 8. Global Palette

The header is followed immediately by `palette_size` RGB888 entries.

Each entry is exactly three bytes:

```text
R G B
```

Palette length:

```text
palette_bytes = palette_size × 3
```

Examples:

| Colors | Palette bytes |
| ---: | ---: |
| 2 | 6 |
| 8 | 24 |
| 16 | 48 |
| 256 | 768 |

Example four-color palette:

```text
00 00 00   # index 0: black
FF 00 00   # index 1: red
00 FF 00   # index 2: green
00 00 FF   # index 3: blue
```

The palette always uses RGB order. Brightness, gamma correction, color calibration, and conversion to GRB or another hardware-specific channel order belong to the output driver.

Duplicate palette entries are valid but wasteful. The encoder should deduplicate colors before choosing `palette_size` so that the smallest bit depth is selected and identical inputs produce identical files. A reader must not reject duplicates.

PXA v1 has no alpha channel. The compiler must first reconstruct complete source images using the source format's composition and disposal rules. Any remaining transparency must then be composited against an explicit background color, commonly represented by palette index 0. See section 17 for the conversion order.

---

## 9. Bits Per Pixel

Frames contain palette indices, not RGB values. The storage depth is derived exclusively from `palette_size`:

| Palette size | Bits per pixel | Pixels per byte |
| ---: | ---: | ---: |
| 1–2 | 1 | 8 |
| 3–4 | 2 | 4 |
| 5–16 | 4 | 2 |
| 17–256 | 8 | 1 |

PXA v1 intentionally uses only 1, 2, 4, and 8 bpp. It does not use tightly packed 3, 5, 6, or 7-bit values. This keeps every pixel within one byte and makes the player simpler.

For `RAW_INDEXED`, the exact payload size is:

```text
pixel_count = width × height
payload_size = ceil(pixel_count × bits_per_pixel / 8)
```

Using integer arithmetic:

```text
payload_size = (pixel_count × bits_per_pixel + 7) / 8
```

The calculation must use a sufficiently wide integer and must be checked for overflow.

---

## 10. Frame Header

Every frame starts with an 8-byte header.

| Offset within frame | Size | Type | Field | Meaning |
| ---: | ---: | --- | --- | --- |
| `0x00` | 2 | `u16` | `duration_ms` | Display duration in milliseconds |
| `0x02` | 1 | `u8` | `encoding` | Payload encoding identifier |
| `0x03` | 1 | `u8` | `flags` | Must be `0x00` in v1 |
| `0x04` | 4 | `u32` | `payload_size` | Number of following payload bytes |

Illustrative C layout:

```c
struct PxaFrameHeader {
    uint16_t duration_ms;
    uint8_t  encoding;
    uint8_t  flags;
    uint32_t payload_size;
};
```

The payload starts immediately after this header.

### 10.1 Duration

`duration_ms` must be in the range 1–65,535.

```text
50 ms   = 20 FPS
100 ms  = 10 FPS
200 ms  = 5 FPS
```

Per-frame timing is supported because imported animations may use variable durations.

`duration_ms` is the intended interval from this frame's presentation to the next frame's presentation, or to the end of the animation for the last frame. File I/O, decoding, and LED transmission must fit within the playback schedule rather than being added to each duration. Section 16 describes cumulative presentation deadlines.

On import, source durations are normalized before encoding. Durations above 65,535 ms are clamped to 65,535 ms. A zero source duration is handled by the compiler's zero-duration policy described in section 17: by default the frame is omitted, and when a default duration is configured the frame is emitted with that duration instead. These conversion rules do not make a zero duration valid in a PXA file: a v1 reader must reject it. See section 17 for composition requirements when omitting source frames.

### 10.2 Encoding identifiers

PXA v1 defines:

```text
0x00 = RAW_INDEXED
```

All other values are unsupported in v1 and must be rejected.

### 10.3 Frame flags

All frame flags are reserved in v1 and must be zero. A v1 reader must reject non-zero frame flags.

### 10.4 Payload size

For `RAW_INDEXED`, the stored `payload_size` must exactly match the value calculated from the dimensions and derived bits per pixel.

Using a `u32` makes the frame layout suitable for matrices much larger than 16×16. A valid v1 frame can never contain more payload bytes than can be represented by this field.

---

## 11. Logical Pixel Order

The logical origin `(0, 0)` is the top-left pixel. The `x` coordinate increases to the right, and the `y` coordinate increases downward.

Pixels form one continuous row-major stream:

```text
(0, 0), (1, 0), ... (width - 1, 0),
(0, 1), (1, 1), ... (width - 1, 1),
...
(width - 1, height - 1)
```

The logical index is:

```text
logical_index = y × width + x
```

Packing continues across row boundaries. Rows are not separately padded.

The file contains no serpentine, rotation, panel-chain, or wiring information. The firmware maps logical coordinates to physical LED indices:

```text
PXA logical (x, y)
        ↓
matrix mapping / rotation / serpentine layout
        ↓
physical LED index
```

This allows the same animation to be used on differently wired displays of the same logical dimensions.

---

## 12. RAW_INDEXED Packing

The palette indices are packed MSB-first into a continuous byte stream.

Every decoded index must be lower than `palette_size`, even when the chosen bit depth can represent additional values. For example, a 10-color palette uses 4 bpp, but indices 10–15 are invalid.

### 12.1 8 bpp

For palettes containing 17–256 colors, one byte stores one pixel:

```text
pixels:  3, 12, 128, 255
bytes:   03 0C 80 FF
```

### 12.2 4 bpp

For palettes containing 5–16 colors, one byte stores two pixels:

```text
bits 7..4 = first pixel
bits 3..0 = second pixel
```

Example:

```text
pixels:  3, 12
binary:  0011 1100
byte:    0x3C
```

A sequence of eight indices:

```text
3, 12, 1, 7, 2, 2, 0, 15
```

is stored as:

```text
3C 17 22 0F
```

### 12.3 2 bpp

For palettes containing 3–4 colors, one byte stores four pixels:

```text
pixel 0 → bits 7..6
pixel 1 → bits 5..4
pixel 2 → bits 3..2
pixel 3 → bits 1..0
```

Example:

```text
pixels:  0, 1, 2, 3
binary:  00 01 10 11
byte:    0x1B
```

### 12.4 1 bpp

For palettes containing 1–2 colors, one byte stores eight pixels:

```text
pixel 0 → bit 7
pixel 1 → bit 6
...
pixel 7 → bit 0
```

Example:

```text
pixels:  1, 0, 1, 1, 0, 0, 1, 0
binary:  10110010
byte:    0xB2
```

### 12.5 Final-byte padding

If the final byte is only partially used, valid pixels occupy its most-significant bits. All unused least-significant bits must be zero.

Example: five 1-bpp pixels `1, 0, 1, 1, 0` become:

```text
10110 000
      ^^^ unused bits, written as zero

= 0xB0
```

A v1 reader must reject non-zero padding bits.

---

## 13. Generic Unpacking Algorithm

Because v1 only allows depths that divide 8, every pixel is contained in one byte.

```text
pixels_per_byte = 8 / bits_per_pixel
mask = (1 << bits_per_pixel) - 1

for logical_index in 0 .. pixel_count:
    byte_index = logical_index / pixels_per_byte
    slot       = logical_index % pixels_per_byte
    shift      = 8 - bits_per_pixel × (slot + 1)
    index      = (payload[byte_index] >> shift) & mask
```

The decoder then validates `index < palette_size` and looks up the RGB value.

---

## 14. Size Examples

### 14.1 Per-frame payload sizes

| Matrix | Pixels | 1 bpp | 2 bpp | 4 bpp | 8 bpp | RGB888 reference |
| ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| 16×16 | 256 | 32 B | 64 B | 128 B | 256 B | 768 B |
| 32×32 | 1,024 | 128 B | 256 B | 512 B | 1,024 B | 3,072 B |
| 32×64 | 2,048 | 256 B | 512 B | 1,024 B | 2,048 B | 6,144 B |
| 64×64 | 4,096 | 512 B | 1,024 B | 2,048 B | 4,096 B | 12,288 B |

Each frame also has an 8-byte header.

### 14.2 Five-second 16×16 example

For a 16-color, 16×16 animation at 20 FPS for 5 seconds:

```text
100 frames × 128-byte payload       = 12,800 bytes
100 frames × 8-byte frame header    =    800 bytes
16 colors × 3-byte palette entry    =     48 bytes
file header                         =     16 bytes
---------------------------------------------------
total                               = 13,664 bytes
```

This is approximately 13.34 KiB, excluding filesystem overhead.

### 14.3 Five-second 32×64 example

For the same timing and 16-color palette on a 32×64 matrix:

```text
100 frames × 1,024-byte payload     = 102,400 bytes
100 frames × 8-byte frame header    =     800 bytes
16 colors × 3-byte palette entry    =      48 bytes
file header                         =      16 bytes
---------------------------------------------------
total                               = 103,264 bytes
```

This is approximately 100.84 KiB.

---

## 15. Decoder Memory and Streaming

The complete animation never needs to be loaded into RAM.

For a 16×16 animation using the largest v1 palette, a straightforward player may use:

```text
palette:             256 × RGB888 = 768 bytes
index framebuffer:   256 bytes
packed payload:      at most 256 bytes
```

Approximately 1.25 KiB (1,280 bytes) is enough, excluding driver state and general application memory.

For larger matrices, the payload and framebuffer grow with `width × height`. A decoder can reduce temporary storage by unpacking a chunk at a time or decoding directly into the output framebuffer.

Every v1 frame is complete and independent, and all frames in a valid file have the same payload size. A seekable source therefore allows direct access by frame number without scanning preceding frame headers or storing a frame index:

```text
frame_stride = 8 + payload_size
frame_offset(i) = 16 + 3 × palette_size + i × frame_stride
file_size = 16 + 3 × palette_size + frame_count × frame_stride
```

Here, `i` is zero-based and must be lower than `frame_count`; `frame_offset(i)` points to the frame header. `payload_size` is calculated from the validated dimensions and palette size, not trusted from a stored frame header. Offset and file-size calculations must use sufficiently wide integers, such as checked `u64` arithmetic, and respect the storage API's limits.

The calculated file size allows an early length check but does not replace validation of frame headers, indices, and padding. A non-seekable source still requires sequential reading. Seeking by playback time requires reading preceding frame durations or using a separately built timing index because durations may vary.

---

## 16. Recommended Firmware Architecture

```text
AnimationPlayer
├── open(path)
├── read_header()
├── read_palette()
├── next_frame()
├── rewind()
└── close()
```

Playback:

```text
open file
  ↓
validate PXA1 header
  ↓
read global palette
  ↓
for each frame (prepare ahead of its presentation time)
    ↓
read and validate frame header
    ↓
read / stream packed indices
    ↓
unpack and validate palette indices
    ↓
palette lookup
    ↓
brightness / gamma / calibration
    ↓
logical-to-physical LED mapping
    ↓
schedule LED transmission to meet the presentation deadline
```

The player should use a monotonic clock and cumulative presentation deadlines. Let `t0` be the presentation time of the first frame:

```text
presentation_time(0) = t0
presentation_time(i + 1) = presentation_time(i) + duration_ms(i)
```

For durations of 100, 200, and 50 ms, frames are scheduled at `t0`, `t0 + 100 ms`, and `t0 + 300 ms`; the animation ends or loops at `t0 + 350 ms`. The last frame retains its scheduled duration before a loop restart or completion.

Reading and decoding the next frame should happen while the current frame is displayed. The player must account for LED transmission time when scheduling the next visible change. For a 100 ms interval with 8 ms of preparation and transmission, approximately 92 ms remain for waiting; waiting another 100 ms would introduce drift. Deadlines should advance from the previous scheduled deadline, not from the time processing finishes.

Durations describe the intended timing; they do not guarantee that every display can meet it. A player must document its policy for missed deadlines, such as presenting late or skipping expired frames. That policy is an implementation choice, not part of the PXA binary format.

The player decides whether the animation loops. Looping is not encoded in PXA v1. When looping on schedule, the next cycle starts at the previous cycle's scheduled end without adding file I/O or decoding time to the loop duration.

---

## 17. PC-side Compiler

Suggested command-line interface:

```bash
pxa convert cat.gif                      # writes cat.pxa next to the source
pxa convert animation.png -o out/anim.pxa
pxa convert *.gif -o animations/         # bulk: one .pxa per input, same name
pxa bundle *.pxa -o animations.bin       # image for the animations partition
pxa inspect cat.pxa
pxa preview cat.pxa
pxa validate cat.pxa
```

`convert` accepts one or more inputs. Each output takes the input's name with a `.pxa` extension, next to the input by default or inside the directory given with `--output`. Inputs that would produce the same output path are rejected before any conversion starts. With several inputs, a failing file is reported and skipped, the remaining files are still converted, and the exit status is non-zero. Conversion options apply to every input of the invocation.

`bundle` validates every input as a complete PXA file, concatenates them in the given order and appends the end marker described in section 4.2. Any invalid input aborts the whole bundle. It prints the offset of each file inside the image and the total size, so the caller can check it against the partition size.

Possible inputs:

- GIF;
- animated WebP;
- PNG spritesheet;
- Aseprite export;
- Divoom animation or `.divoom16`.

Suggested conversion pipeline:

```text
source animation
      ↓
decode source frames and durations
      ↓
reconstruct complete images using source composition / disposal rules
      ↓
normalize durations: clamp to 65,535 ms, omit or substitute zero durations
      ↓
resize / crop to target dimensions
      ↓
resolve remaining transparency against a background color
      ↓
RGB conversion
      ↓
global palette generation / quantization
      ↓
map RGB pixels to palette indices
      ↓
choose 1 / 2 / 4 / 8 bpp from palette size
      ↓
pack indices MSB-first
      ↓
write PXA1
```

Each emitted PXA frame must represent the complete image intended for display. Importers must apply source frame offsets, sub-rectangles, blending, and disposal rules as defined by the source format. A source decoder may already perform this reconstruction; importers must ensure it is complete before resizing, background compositing, and quantization. Transparent areas in a source update may preserve previous image content and must not be replaced with the background before reconstruction.

Source durations are handled as follows:

- By default, a zero-duration source frame emits no PXA frame, but its composition and disposal effects must still be processed when reconstructing subsequent source images.
- When the user supplies a default duration, every zero-duration source frame is emitted with that duration instead of being omitted. This matters for older GIF files in which every frame has a delay of zero; browsers typically display such files at about 100 ms per frame, and omitting every frame would otherwise make conversion fail. Example:

  ```bash
  pxa convert old.gif --default-duration 100
  ```
- A source duration greater than 65,535 ms is clamped to 65,535 ms. The compiler does not duplicate frames to preserve the excess duration.
- Durations from 1 to 65,535 ms are preserved.

The file header's `frame_count` must count emitted PXA frames. If no frames remain after omitting zero-duration frames, conversion must fail with an actionable error because PXA v1 requires at least one frame.

The compiler should select the smallest acceptable global palette. For photographic or high-color inputs it may quantize to a configurable maximum:

```bash
pxa convert source.gif --colors 16
```

---

## 18. Validation and Error Handling

A v1 reader must validate at least:

- the file contains the full 16-byte header;
- `magic == "PXA1"`;
- `width != 0` and `height != 0`;
- `frame_count != 0`;
- `1 <= palette_size <= 256`;
- file flags are zero;
- the complete palette is present;
- pixel-count and payload-size calculations do not overflow;
- every frame contains a complete 8-byte header;
- `duration_ms != 0`;
- `encoding == 0x00`;
- frame flags are zero;
- `payload_size` exactly matches the calculated RAW payload size;
- the complete payload is inside file bounds;
- every decoded palette index is lower than `palette_size`;
- unused final-byte padding bits are zero.

After exactly `frame_count` frames, a conforming v1 file must end. A v1 reader must reject trailing data. A tolerant diagnostic tool may continue inspection and report trailing data as a warning, but it must identify the file as non-conforming; the v1 encoder must not write such data. When the file length is available, readers can check it against the calculated size in section 15 before reading the frames. For assets stored in a raw flash partition, this check applies to the asset's declared byte range, not to unused partition space.

Malformed or unsupported files must fail gracefully without out-of-bounds reads, oversized allocations, integer overflow, or partially trusted dimensions.

---

## 19. Complete Minimal Example

This example describes a 2×2, two-color, two-frame animation.

```text
dimensions:     2×2
frame count:    2
palette:        black, red
bits per pixel: 1
frame duration: 100 ms each
```

Header:

```text
50 58 41 31   # "PXA1"
02 00         # width = 2
02 00         # height = 2
02 00 00 00   # frame_count = 2
02 00         # palette_size = 2
00 00         # flags = 0
```

Palette:

```text
00 00 00      # index 0: black
FF 00 00      # index 1: red
```

Frame 0, pixels `0, 1, 1, 0`:

```text
64 00         # duration_ms = 100
00            # encoding = RAW_INDEXED
00            # flags = 0
01 00 00 00   # payload_size = 1
60            # 0110 0000, four pixels then zero padding
```

Frame 1, pixels `1, 0, 0, 1`:

```text
64 00         # duration_ms = 100
00            # encoding = RAW_INDEXED
00            # flags = 0
01 00 00 00   # payload_size = 1
90            # 1001 0000, four pixels then zero padding
```

Complete file:

```text
50 58 41 31 02 00 02 00 02 00 00 00 02 00 00 00
00 00 00 FF 00 00
64 00 00 00 01 00 00 00 60
64 00 00 00 01 00 00 00 90
```

Total size: 40 bytes.

---

## 20. Divoom Compatibility and Inspiration

PXA is conceptually inspired by reverse-engineered Divoom animation encoding:

- indexed colors;
- palettes;
- bit-packed palette indices;
- per-frame timing.

PXA is an independent format. Divoom-specific download, decryption, parsing, and conversion belong in PC-side tooling. The firmware only reads PXA.

Possible future support:

```bash
pxa convert animation.divoom16
```

---

## 21. Future Encodings

Future format versions may add encoding identifiers such as:

```text
0x01 = RLE
0x02 = DELTA
```

They are not valid PXA v1 encodings.

### 21.1 RLE

Run-length encoding may help frames with large flat regions. The PC compiler can compare it with packed RAW data and select the smaller representation.

### 21.2 Delta frames

Pixel-art animations often modify only a small part of the previous frame. A future generic delta representation could contain:

```text
ceil(width × height / 8)-byte changed-pixel bitmap
+
packed new palette indices for changed pixels
```

For 16×16, the bitmap is 32 bytes. For 32×64, it is 256 bytes.

If delta frames are introduced, periodic full keyframes should be used for seeking, looping, and recovery. The exact encoding, bit order, padding rules, and keyframe semantics must be specified by the future format version rather than inferred from this draft.

---

## 22. Procedural Effects

Animations such as rainbow, plasma, fire, matrix rain, snow, stars, particles, Game of Life, clocks, and audio visualizers are generally better generated procedurally by the firmware.

PXA is primarily intended for authored or imported pixel-art animations.

---

## 23. PXA v1 Summary

```text
PXA1 HEADER
    magic "PXA1"
    u16 width, little-endian
    u16 height, little-endian
    u32 frame_count, little-endian
    u16 palette_size, little-endian
    u16 flags = 0
        ↓
GLOBAL RGB888 PALETTE
        ↓
FRAME HEADER
    u16 duration_ms, little-endian
    u8 encoding = RAW_INDEXED
    u8 flags = 0
    u32 payload_size, little-endian
        ↓
PACKED PALETTE INDICES
    1 / 2 / 4 / 8 bpp
    logical row-major order
    MSB-first within each byte
        ↓
NEXT FRAME
```

The core v1 player only needs to parse fixed-size headers, load one RGB palette, unpack palette indices, map logical pixels to the physical matrix, and respect each frame's duration.
