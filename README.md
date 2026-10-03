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
    animation.pxa
          │
          ▼
    pxa bundle (PC)  ──►  animations partition  ──►  ESP32-C6 firmware  ──►  WS2815 LED matrix
```

## Hardware

The current frame is:

- an ESP32-C6-WROOM-1-N8 module (8 MB flash, Wi-Fi);
- a 16×16 WS2815 12 V LED matrix, wired as a single serpentine chain;
- the LED data line on GPIO18 (SPI with DMA), driven directly at 3.3 V;
- a MINI560 buck converter supplying 5 V to the ESP32 side.

The 16×16 size is a configuration, not a limit. Matrix dimensions and wiring
are declared in one place in the firmware and the file format is dimension
agnostic. See [`doc/wiring.md`](doc/wiring.md) for the electrical details and
[`doc/pcb.md`](doc/pcb.md) for the carrier board that implements them.

## Repository layout

| Path | Contents |
| --- | --- |
| `crates/pxa-core` | `no_std`, allocation-free implementation of the PXA format, shared by the firmware and the converter |
| `firmware` | ESP32-C6 application built on `esp-hal` |
| `tools/pxa` | Desktop CLI that converts animations to PXA, bundles them and inspects PXA files |
| `tools/pixframe` | Desktop CLI that builds the device configuration image |
| `config.example.toml` | Template for the device configuration |
| `partitions.csv` | Flash partition table: firmware, reserved OTA space, `animations` data partition |
| `doc/PXA_spec.md` | Authoritative specification of the PXA binary format |
| `doc/wiring.md` | Electrical wiring and physical LED layout |
| `doc/pcb.md` | Carrier board: design decisions, KiCad procedure, manufacturing |
| `hardware` | KiCad 10 project for the carrier board |

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

Flash and open the serial monitor. The `espflash` runner in
`.cargo/config.toml` applies `partitions.csv`, so run this from the workspace
root:

```bash
mise run flash
# or
cargo run -p pix-frame-firmware --release --target riscv32imac-unknown-none-elf
```

A plain `cargo build` at the workspace root only builds the host packages; the
firmware always needs the explicit `-p` and `--target`.

### Loading animations onto the frame

Animations live in the `animations` partition, separate from the firmware,
as a bundle of PXA files (see the spec, section 4). Put your `.pxa` files in
`assets/` (ignored by git) and run:

```bash
mise run flash-assets
```

This builds the bundle with `pxa bundle assets/*.pxa -o target/animations.bin`,
erases the partition and writes the image at its offset with `espflash`.
Reflashing the firmware does not touch the animations, and vice versa.

### Configuring the frame

Optional features (information screens, and later Wi-Fi and weather) are
driven by a configuration stored in the `nvs` partition. Copy
`config.example.toml` to `config.toml` (ignored by git, it holds the Wi-Fi
password), edit it, then:

```bash
mise run flash-config
```

This runs `pixframe config build config.toml -o target/nvs.bin` and writes the
image at the partition offset. `pixframe config dump target/nvs.bin` prints
what an image contains, without the password. A frame without configuration
just plays animations.

### Firmware behavior

At boot the firmware reads the partition table, indexes the bundle (up to 256
animations) and logs what it found over the serial port, including a warning
when indexing stopped before the end of the bundle. It then plays in a
random order: each cycle is a shuffle of every animation, and a new cycle never
starts with the animation that just ended. An animation loops until at least
3 seconds have been scheduled at a loop boundary, so short loops repeat and long
ones play once in full.

Frames are scheduled on cumulative deadlines from the first frame: the next
frame is decoded while the current one is displayed, then sent when its
predecessor's duration has elapsed. Decoding and LED transmission time do not
accumulate as drift. A late frame is shown as soon as it is ready and the
schedule is not rebased.

Pressing the BOOT button of the dev board (GPIO9) runs the LED self-test once,
after which playback resumes with the next animation. The self-test ends with
a preview of every screen: text, clock, the seven weather icons in quick
succession, and the Wi-Fi status.

With a `[wifi]` section in the configuration, the firmware joins the network
at boot and keeps reconnecting after drops. The first result is flashed on the
matrix for a moment ("WIFI" then "OK" in green or "KO" in red), and the
self-test ends with the same screen. Once connected, the clock is set from
`pool.ntp.org` and refreshed every hour. The local offset comes from the
configured time zone: built-in rules cover the European Union and UTC, and
the weather service supplies the exact offset for any zone once it has
answered. Without Wi-Fi the frame only plays animations.

With a `[location]` section, the current outdoor conditions are fetched from
[Open-Meteo](https://open-meteo.com/) (free, no key, plain HTTP) every
15 minutes. The weather screen shows an icon for the sky condition (clear,
partly cloudy, cloudy, fog, rain, snow, thunderstorm) above the temperature
in whole degrees, colored from blue to red. It is skipped when the last
answer is older than an hour.

When `info_every` is set, an information screen is shown for 5 seconds after
that many animations. Each screen has an availability condition, so it is
skipped until its data exists: the clock waits for the first time sync, the
weather for the first Open-Meteo answer. Available screens alternate. Hours
are drawn above minutes in large digits.

Files whose dimensions differ from the matrix, or that fail validation, are
logged and skipped. Without any playable animation the LED self-test runs
instead. Output brightness and the minimum play time are constants at the top
of `firmware/src/main.rs`.

## Using the `pxa` converter

The binary is produced at `target/release/pxa`. It has four commands.

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

### `pxa bundle`

Validates PXA files and concatenates them, in the given order, into the raw
image expected by the `animations` partition. The image ends with a four-byte
marker so the firmware stops at the last file even if the partition previously
held a larger bundle. Any invalid input aborts the bundle. Files whose dimensions differ from the
rest of the bundle are reported, since the firmware skips them.

```bash
pxa bundle assets/*.pxa -o animations.bin
```

```text
0x000000     3871 bytes  16x16, 14 frames  assets/batman.pxa
0x000f1f      581 bytes  16x16, 4 frames  assets/bissie.pxa
animations.bin: 2 animations, 4456 bytes
```

Write it to the frame with `mise run flash-assets`, or by hand:

```bash
espflash erase-region --chip esp32c6 0x410000 0x3F0000
espflash write-bin --chip esp32c6 0x410000 animations.bin
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
- `pxa` converter with `convert` (single or bulk), `bundle`, `inspect` and `validate`;
- firmware PXA player streaming from a dedicated flash partition, random
  playlist with a minimum play time;
- firmware LED self-test with a configurable matrix layout;
- device configuration in NVS with the `pixframe` tool;
- Wi-Fi station, SNTP time synchronisation, clock and outdoor weather screens
  shown every N animations.

Not yet:

- indoor temperature screen (needs a sensor);

- `pxa preview`;
- Aseprite and Divoom importers;
- uploading animations to the frame without reflashing.
