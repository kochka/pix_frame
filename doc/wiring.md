# Electrical Wiring

**Status:** draft. Items marked *to confirm* have not been verified on the physical frame.

This document describes how the current frame is built electrically and how the
firmware maps logical pixels onto the physical LED chain. Animation data never
depends on anything written here; see `PXA_spec.md` section 11.

---

## 1. Bill of Materials

| Part | Role | Notes |
| --- | --- | --- |
| ESP32-C6-WROOM-1-N8 | Controller | 8 MB flash, RISC-V, 3.3 V logic |
| 16×16 WS2815 matrix | Display | 256 LEDs, 12 V, serpentine chain |
| 74AHCT125 | Level shifter | Quad buffer, 3.3 V in → 5 V out, powered from 5 V |
| MINI560 buck converter | 5 V supply | 12 V in → 5 V out for the ESP32 side |
| 12 V power supply | LED supply | Feeds the matrix and the MINI560 input |

---

## 2. Power Distribution

```text
12 V PSU ─┬──────────────────────────► WS2815 matrix +12 V
          │
          └──► MINI560 (12 V → 5 V) ─┬─► ESP32-C6 5 V input
                                     └─► 74AHCT125 VCC

GND: 12 V PSU, MINI560, ESP32-C6, 74AHCT125 and matrix share one ground.
```

- The 74AHCT125 must be powered from **5 V**, not 3.3 V. Its TTL input threshold
  (about 2.0 V) is what allows a 3.3 V GPIO to drive it reliably.
- Place a 100 nF ceramic capacitor across the 74AHCT125 VCC/GND pins, close to the chip.
- Place a large electrolytic capacitor (1000 µF or more, 16 V or higher) across the
  12 V rail at the matrix input to absorb current spikes when many LEDs switch. *To confirm.*
- Keep the ground connection between the ESP32 side and the matrix short and solid.
  A missing or weak shared ground is the most common cause of flicker or a dead chain.

### 2.1 Current budget

WS2815 datasheets list roughly 15 mA per LED at 12 V for full white. These are
datasheet figures, not measurements on this frame.

| Condition | Current at 12 V (datasheet) |
| --- | ---: |
| 1 LED full white | ~15 mA |
| 256 LEDs full white | ~3.8 A |
| 256 LEDs at self-test brightness 60/255 | ~0.9 A |

Measured values on the actual frame: *to record*.

The MINI560 only supplies the ESP32-C6 and the level shifter, so a few hundred mA
is enough. The 12 V supply must cover the matrix current above.

---

## 3. LED Data Signal

```text
ESP32-C6 GPIO18 ──► 74AHCT125 1A
                    74AHCT125 1Y ──► [ series R, 220–470 Ω ] ──► WS2815 DI (LED 0)
                    74AHCT125 1OE ──► GND (output always enabled)
```

- **GPIO18** is the data output used by the firmware. It is the MOSI line of
  SPI2, routed there through the GPIO matrix, and the frame is sent by DMA so
  the bit timing does not depend on the CPU. The clock and chip-select lines
  are not used. An earlier version used the RMT peripheral; it needed the CPU
  to refill its 48-symbol memory during transmission, which the scheduler and
  the Wi-Fi driver disturbed once the firmware went async.
- The 74AHCT125 raises the 3.3 V signal to 5 V, which the 12 V WS2815 accepts at its
  data input.
- A series resistor of a few hundred ohms next to the first LED reduces ringing and
  protects the data input. *To confirm whether one is fitted.*
- WS2815 has a second data input, **BI** (backup input). If a LED fails, the next LED
  keeps receiving data through BI. Each BI is normally wired to the DI of the previous
  LED on the strip itself. For LED 0, BI is left unconnected or tied to GND. *To confirm.*
- Unused 74AHCT125 gates: tie their inputs (2A, 3A, 4A) to GND and their OE pins high
  or low consistently so they do not float.

### 3.1 Timing used by the firmware

| Parameter | Value | Where |
| --- | --- | --- |
| SPI clock | 3.2 MHz, 4 SPI bits per LED bit (800 kbit/s) | `firmware/src/leds.rs` |
| Bit `0` on the wire | `1000`: 312 ns high, 937 ns low | `firmware/src/leds.rs` |
| Bit `1` on the wire | `1100`: 625 ns high, 625 ns low | `firmware/src/leds.rs` |
| Reset pulse | 120 zero bytes, 300 µs low | `firmware/src/leds.rs` |
| Channel order on the wire | GRB | `firmware/src/leds.rs` |

These pulses sit inside the WS2812B/WS2815 windows (0: 220–380 ns high,
1: 580–1000 ns high, 1.25 µs per bit). WS2815 requires a longer reset than the
original 50 µs; 300 µs works on the current frame.

---

## 4. Physical LED Layout

The matrix is a single chain of 256 LEDs wired in **serpentine** order: each row is
wired in the opposite direction of the previous one.

```text
logical (x, y), origin top-left, x → right, y → down

row 0:   0 →  1 →  2 → ... → 15
row 1:  31 ← 30 ← 29 ← ... ← 16
row 2:  32 → 33 → 34 → ... → 47
row 3:  63 ← 62 ← ...
...
```

Mapping implemented today (see `physical_index` in `firmware/src/display.rs`, configured by `LAYOUT` in `firmware/src/main.rs`):

```text
if y is even:  physical = y × width + x
if y is odd:   physical = y × width + (width - 1 - x)
```

Assumptions behind this mapping, all *to confirm* by running the self-test and
watching where the first lit pixel appears:

- LED 0 is at the top-left corner when the frame is in its normal viewing orientation.
- Row 0 runs left to right.
- Rows are chained top to bottom.

If the frame is mounted rotated or mirrored, this is corrected in the firmware's
display mapping, never in the animation files.

---

## 5. Logical to Physical Pipeline

```text
PXA palette index
      ↓
RGB palette lookup
      ↓
brightness / gamma / calibration
      ↓
logical (x, y) → serpentine physical index      (section 4)
      ↓
RGB → GRB byte order                            (section 3.1)
      ↓
SPI DMA on GPIO18 → 74AHCT125 → WS2815 DI       (section 3)
```

Only the last three steps depend on this specific frame. Larger frames (32×32,
32×64, ...) or panels chained side by side need a different mapping in the same
place, not a different file format.

---

## 6. Self-Test Button

Pressing a button runs the LED self-test once; playback then resumes with the
next animation. The firmware reads **GPIO9** as an input with the internal
pull-up enabled and treats a low level as "pressed".

On the Espressif ESP32-C6 dev boards (DevKitC-1, DevKitM-1) GPIO9 is the
**BOOT** button, so nothing has to be added. GPIO9 is a strapping pin: holding
it during reset enters the serial download mode, which is harmless here, and
once the chip has booted it behaves as a normal GPIO. The RESET button is wired
to EN, not to a GPIO, and cannot be read by the firmware.

For a bare module, wire a push button between the chosen GPIO and GND and
change the pin in `firmware/src/main.rs`; the internal pull-up makes an
external resistor unnecessary.

## 7. Wi-Fi

The module's on-board antenna is used; nothing to wire. Expect peaks of a few
hundred milliamperes on the 3.3 V rail while the radio transmits, on top of
the LED supply. The first connection result is flashed on the matrix after
boot and at the end of the self-test.

## 8. To Confirm on the Physical Frame

- [ ] How the ESP32-C6 module gets 3.3 V (dev board regulator or external LDO).
- [ ] Series resistor on the data line and its value.
- [ ] Bulk capacitor on the 12 V rail near the matrix.
- [ ] Position of LED 0 and direction of row 0 in viewing orientation.
- [ ] Whether BI of LED 0 is connected.
- [ ] Measured current at full white and at typical animation brightness.
- [ ] Whether a single 12 V injection point is enough for 256 LEDs (voltage drop across the chain).
