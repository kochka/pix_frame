# pix-frame

Firmware and tooling for playing animated pixel art on an RGB LED matrix driven
by an ESP32-C6.

Animations are authored in the usual formats (GIF, APNG, WebP, PNG), converted
on a PC into a small purpose-built binary format called **PXA**, and streamed
by the firmware from flash with a fixed, predictable memory footprint. The
ESP32 never decodes GIF or PNG.

```text
GIF / APNG / WebP / PNG
          │
          ▼
   pxa convert (PC)
          │
          ▼
    animation.pxa  ──►  ESP32-C6 firmware  ──►  WS2815 LED matrix
```

## Hardware

The current frame is:

- an ESP32-C6-WROOM-1-N8 module (8 MB flash);
- a 16×16 WS2815 12 V LED matrix, wired as a single serpentine chain;
- a 74AHCT125 level shifter for the LED data line on GPIO18;
- a MINI560 buck converter supplying 5 V to the ESP32 side.

The 16×16 size is a configuration, not a limit. Matrix dimensions and wiring
are declared in one place in the firmware and the file format is dimension
agnostic. See [`doc/wiring.md`](doc/wiring.md) for the electrical details.

## Repository layout

| Path | Contents |
| --- | --- |
| `crates/pxa-core` | `no_std`, allocation-free implementation of the PXA format, shared by the firmware and the converter |
| `firmware` | ESP32-C6 application built on `esp-hal` |
| `tools/pxa` | Desktop CLI that converts animations to PXA and inspects PXA files |
| `doc/PXA_spec.md` | Authoritative specification of the PXA binary format |
| `doc/wiring.md` | Electrical wiring and physical LED layout |
| `AGENTS.md` | Development guidelines for humans and coding agents |

## The PXA format in short

A PXA v1 file is a 16-byte header, a global RGB888 palette of 1 to 256 colors,
then one 8-byte header and one payload per frame. Pixels are palette indices
packed at 1, 2, 4 or 8 bits per pixel depending only on the palette size.
Every frame is complete and has its own duration. There is no compression,
no transparency and no metadata, which keeps the decoder tiny.

A 16×16 animation with 16 colors costs 136 bytes per frame. Five seconds at
20 FPS is about 13 KiB.

Full details, byte layouts and validation rules are in
[`doc/PXA_spec.md`](doc/PXA_spec.md).

## Building

Requirements: a stable Rust toolchain (pinned by `rust-toolchain.toml`, which
also pulls the `riscv32imac-unknown-none-elf` target), and
[`espflash`](https://github.com/esp-rs/espflash) to flash the board.
[`mise`](https://mise.jdx.dev/) is optional but provides the canonical tasks.

Host-side checks (format, tests, lints, converter build):

```bash
mise run check
```

Or with plain Cargo:

```bash
cargo test -p pxa-core -p pxa
cargo clippy -p pxa-core -p pxa --all-targets -- -D warnings
cargo build -p pxa --release
```

Firmware:

```bash
mise run build-firmware
# or
cargo build -p pix-frame-firmware --release --target riscv32imac-unknown-none-elf
```

Flash and open the serial monitor (uses the `espflash` runner configured in
`.cargo/config.toml`):

```bash
cargo run -p pix-frame-firmware --release --target riscv32imac-unknown-none-elf
```

A plain `cargo build` at the workspace root only builds the host packages; the
firmware always needs the explicit `-p` and `--target`.

## Using the `pxa` converter

The binary is produced at `target/release/pxa`. It has three commands.

### `pxa convert`

Converts one or more source animations to PXA. Supported inputs: GIF, PNG,
APNG and WebP. Each output takes the input's name with a `.pxa` extension.

```bash
pxa convert cat.gif
```

```text
cat.gif -> cat.pxa: 16x16, 12 frames, 9 colors (4 bpp), 1675 bytes
```

Several inputs are converted in turn. A file that fails is reported and
skipped, the others are still written, and the exit status is non-zero. Inputs
that would share an output name, such as `cat.gif` and `cat.png`, are rejected
up front:

```bash
pxa convert *.gif -o animations/
```

Options:

| Option | Effect |
| --- | --- |
| `-o, --output PATH` | Output file for a single input, or an existing directory for any number of inputs. Default: next to each input. |
| `--size WxH` | Resize with nearest-neighbour sampling, e.g. `--size 16x16`. By default the source dimensions are kept. |
| `--background RRGGBB` | Color composited under transparent pixels. Default `000000` (black). |
| `--default-duration MS` | Duration given to source frames whose delay is zero, and to static images. Without it, zero-delay frames are omitted. |
| `--colors N` | Maximum palette size, 1 to 256. Sources with more colors are quantized. Default 256. |

The converter keeps the exact source colors whenever the whole animation fits
the palette, which is the case for almost all pixel art. Quantization only
happens when it must, and a note is printed when it does.

Typical cases:

```bash
# 32×32 source scaled down to the frame, transparent areas over dark grey
pxa convert ghost.png --size 16x16 --background 202020

# Old GIF where every frame has a delay of 0: keep the frames at 100 ms each
pxa convert old.gif --default-duration 100

# Force a 16-color palette (4 bpp) to halve the file size
pxa convert photo.webp --colors 16 -o /tmp/photo16.pxa
```

Two conversions of the same input with the same options produce identical
files.

### `pxa inspect`

Prints the header, the palette and every frame with its duration.

```bash
pxa inspect cat.pxa
```

```text
file:        cat.pxa (439 bytes)
magic:       PXA1
size:        16x16
frames:      3
palette:     5 colors
depth:       4 bpp
payload:     128 bytes per frame
expected:    439 bytes
palette:
    0: #FF0000
    1: #000000
    ...
frames:
     0:   100 ms, 128 payload bytes
     1:   200 ms, 128 payload bytes
     2:    50 ms, 128 payload bytes
total:       350 ms per loop
```

### `pxa validate`

Checks that a file is a conforming PXA v1 file and exits with a non-zero
status otherwise. Errors name the frame and the field at fault.

```bash
pxa validate cat.pxa
```

```text
cat.pxa: valid PXA v1, 16x16, 3 frames, 5 colors
```

```text
error: broken.pxa: invalid PXA file: frame 2: pixel 17 uses palette index 9, outside the palette
```

## Status

Done:

- PXA v1 specification;
- `pxa-core` encoder, decoder and validation, with byte-exact tests;
- `pxa` converter with `convert` (single or bulk), `inspect` and `validate`;
- firmware LED self-test with a configurable matrix layout.

Not yet:

- firmware PXA player and LittleFS animation storage;
- `pxa preview`;
- Aseprite and Divoom importers;
- uploading animations to the frame without reflashing.
