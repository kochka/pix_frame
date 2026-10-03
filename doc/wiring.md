# Electrical Wiring

**Status:** draft. Items marked *to confirm* have not been verified on the physical frame.

This document describes how the current frame is built electrically and how the
firmware maps logical pixels onto the physical LED chain. Animation data never
depends on anything written here; see `PXA_spec.md` section 11.

The carrier board that implements this circuit lives in `hardware/`; its
design decisions and the KiCad procedure are in [`pcb.md`](pcb.md).

---

## 1. Bill of Materials

| Part | Role | Notes |
| --- | --- | --- |
| ESP32-C6-WROOM-1-N8 | Controller | 8 MB flash, RISC-V, 3.3 V logic |
| 16×16 WS2815 matrix | Display | 256 LEDs, 12 V, serpentine chain |
| 74AHCT125 | Level shifter | *Not fitted*, kept as a fallback; see section 3 |
| MINI560 buck converter | 5 V supply | 12 V in → 5 V out for the ESP32 side |
| 12 V power supply | LED supply | Feeds the matrix and the MINI560 input |
| P-channel MOSFET | On/off switch | IRF4905 or similar, low Rds(on), see section 2.2 |
| SPST switch | On/off control | Signal rated (0.5 A, 30 V), carries 0.1 mA |
| GL5528 photoresistor | Ambient light | *Planned*; 5 mm CdS cell, see section 7 |
| SHT31-D | Temperature, humidity | I²C sensor (any SHT3x), see section 8 |

### 1.1 GPIO map

| GPIO | Function | Notes |
| ---: | --- | --- |
| 2 | Ambient light | ADC1 channel, *planned*, section 7 |
| 6 | I2C SDA | SHT31, section 8 |
| 7 | I2C SCL | SHT31, section 8 |
| 9 | Self-test button | BOOT button, internal pull-up, section 6 |
| 18 | LED data | SPI2 MOSI sent by DMA, section 3 |

GPIO4, GPIO5, GPIO8, GPIO9 and GPIO15 are strapping pins on the ESP32-C6. Only
GPIO9 is used here, and only as an input read after boot.

---

## 2. Power Distribution

```text
12 V PSU ──► [ P-MOSFET, section 2.2 ] ─┬──► WS2815 matrix +12 V
                                        │
                                        └──► MINI560 (12 V → 5 V) ──► ESP32-C6 5 V

GND: 12 V PSU, MINI560, ESP32-C6 and matrix share one ground.
```

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

The MINI560 only supplies the ESP32-C6, so a few hundred mA is enough. The
12 V supply must cover the matrix current above.

### 2.2 On/off switch

The whole frame is switched on the **12 V rail, upstream of everything**, so
that "off" means off: matrix, converter and ESP32 all lose power. A small
signal switch cannot carry the matrix current, so it drives the gate of a
P-channel MOSFET and the MOSFET carries the load.

```text
12 V PSU + ──┬──────────────┤S      D├──┬──► WS2815 matrix +12 V
             │               P-MOSFET   └──► MINI560
          [ 100 kΩ ]         ╷G
             └───────────────┤
                             │
                         [ 10 kΩ ]
                             │
                          switch
                             │
                            GND
```

- Switch **open**: the 100 kΩ holds the gate at the source potential, Vgs = 0,
  the MOSFET is blocked and the frame is dark.
- Switch **closed**: the divider pulls the gate to about 1 V, Vgs is roughly
  −11 V and the MOSFET conducts with a few tens of milliohms.
- The switch itself carries **12 V / 110 kΩ, about 0.1 mA**, far below the rating
  of any signal switch. Only the gate wiring reaches it, so it can sit anywhere
  on the frame on two thin wires.
- A **P**-channel device on the high side, not an N-channel one on the low side:
  switching the ground would break the ground shared with the ESP32, and the
  data line would then push current into the matrix through its protection
  diodes.
- Pick the MOSFET on its Rds(on), since the dissipation is Rds(on) × I². An
  IRF4905 (55 V, about 20 mΩ) dissipates 0.18 W at 3 A and needs no heatsink.
  Common P-channel parts in the 100–200 mΩ range would waste 1 to 2 W instead.
  Vgs is rated ±20 V on these devices, so 12 V on the gate needs no clamp.
- The 10 kΩ in series with the switch slows the turn-on edge, which limits the
  inrush while the bulk capacitor charges. Adding 100 nF between gate and source
  slows it further if switch-on turns out to be harsh.

An alternative was considered and rejected: wiring the switch to a GPIO and
letting the firmware blank the matrix. It needs no power components and keeps
Wi-Fi alive, but the supply stays loaded and the WS2815 controllers keep drawing
their idle current, so the frame would never really be off.

---

## 3. LED Data Signal

```text
ESP32-C6 GPIO18 ──► [ series R, 220–470 Ω ] ──► WS2815 DI (LED 0)
```

- **GPIO18** is the data output used by the firmware. It is the MOSI line of
  SPI2, routed there through the GPIO matrix, and the frame is sent by DMA so
  the bit timing does not depend on the CPU. The clock and chip-select lines
  are not used. An earlier version used the RMT peripheral; it needed the CPU
  to refill its 48-symbol memory during transmission, which the scheduler and
  the Wi-Fi driver disturbed once the firmware went async.
- The 3.3 V GPIO drives the WS2815 data input **directly**; no level shifter is
  needed. The WS2815 logic runs on an internal regulator instead of on the 12 V
  rail, and its input threshold sits comfortably below 3.3 V. This is unlike the
  5 V WS2812B, whose 0.7 × VDD threshold is above what a 3.3 V GPIO delivers.
  Only LED 0 ever sees the ESP32 signal: each LED regenerates the data for the
  next one, so a marginal level could only corrupt the first pixel. Direct drive
  with a shared ground has been verified on the frame, without artifacts.
- A 74AHCT125 buffer powered from 5 V was part of an earlier design and is kept
  as a fallback, should LED 0 alone ever misbehave. It is not fitted: it would
  add an active part on the only critical path, a dependency on the 5 V rail,
  and a pull resistor to keep its input defined while GPIO18 is high impedance
  during boot.
- Keep the wire from GPIO18 to LED 0 short. Without a 5 V buffer the noise margin
  on that link is smaller, which only matters over long runs.
- A series resistor of a few hundred ohms next to the first LED reduces ringing and
  limits the current into the data input protection diode when the ESP32 is powered
  over USB alone while the matrix is off, which happens on every flash.
  *To confirm whether one is fitted.*
- WS2815 has a second data input, **BI** (backup input). If a LED fails, the next LED
  keeps receiving data through BI. Each BI is normally wired to the DI of the previous
  LED on the strip itself. For LED 0, BI is left unconnected or tied to GND. *To confirm.*

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
SPI DMA on GPIO18 → WS2815 DI                   (section 3)
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

## 7. Ambient Light Sensor

*Planned, not fitted. No firmware support yet.*

The goal is to adapt the matrix brightness to the room: full brightness in
daylight, dim at night, without the frame ever being the brightest thing in a
dark room.

```text
3.3 V ──┤ GL5528 ├──┬──────────────► GPIO2 (ADC1)
                    │
                 [ 10 kΩ ]
                    │
                   GND
```

- The GL5528 is a photoresistor: roughly 10–20 kΩ at 10 lux and about 1 MΩ in
  the dark. It produces no voltage of its own, hence the divider. The node
  voltage rises with light: about 0.03 V in the dark, 2 V under normal room
  lighting, 3 V in daylight.
- The fixed 10 kΩ is not critical; 4.7 kΩ to 22 kΩ all work. Its value decides
  which light level lands in the middle of the ADC range.
- **GPIO2 belongs to ADC1.** On the ESP32-C6, ADC1 covers GPIO0 to GPIO6, and
  ADC2 cannot be used while the radio is on. The sensor must stay on ADC1.
- An optional 100 nF from the ADC pin to GND filters noise picked up by the
  wires. The firmware averages samples anyway, so it can be left out and added
  later if readings jump around.
- Only the cell is deported: the divider resistor and the capacitor belong next
  to the ESP32, so two thin twisted wires are enough to reach it.
- **The cell must not see the matrix.** Light from the panel reaching the sensor
  creates a feedback loop: the LEDs light the cell, the firmware dims them, the
  cell goes dark, the firmware brightens them again. It is mounted in an opaque
  3D-printed tube through the wooden frame, head flush with the outer face and
  facing the room. Print it in a dark, opaque filament: natural PLA is
  translucent and leaks light along the tube walls.
- Keep the tube short and its opening at least as wide as the 5 mm head, or the
  cell becomes directional and measures one spot on a wall instead of the room.
- Thresholds are per-frame. GL55xx units vary widely from one to the next, and
  so does the room, so the dark and bright reference readings belong in the
  device configuration rather than in the firmware.
- The GL5516 is an acceptable substitute with the same 10 kΩ resistor. Avoid the
  GL5539 and GL5549: far more resistive, they would need a 100 kΩ resistor and a
  high-impedance node that picks up noise.

---

## 8. Temperature and Humidity Sensor

Read by the firmware once a minute for the indoor screen
(`firmware/src/indoor.rs`). Any SHT3x (SHT30, SHT31, SHT35) works the same
way. Without a sensor, or when it stops answering, the screen is skipped.

```text
ESP32-C6 GPIO6 ───────────────────► SHT31 SDA
ESP32-C6 GPIO7 ───────────────────► SHT31 SCL

3.3 V ────────────────────────────► SHT31 VDD (VIN on a breakout board)
GND ──────────────────────────────► SHT31 GND
```

- The SHT31-D runs from 2.4 V to 5.5 V, so it sits on the ESP32 3.3 V rail and
  needs no level shifting. Typical accuracy is ±0.2 °C and ±2 %RH.
- Its I2C address is **0x44** with ADDR tied to GND, 0x45 with ADDR tied to VDD.
  Breakout boards usually pull ADDR low on the board, giving 0x44.
- **No pull-up resistors are added here: the breakout board in use already
  carries them** (10 kΩ is the usual value). The I2C lines are open drain, so
  the high level comes from a pull-up; a bare sensor would need one 4.7 kΩ
  resistor per line to 3.3 V. Fitting a second pair in parallel would lower the
  effective value too far and keep the sensor from pulling the lines down.
- A 100 nF capacitor across VDD and GND close to the sensor. Breakout boards
  normally have one already.
- The bus runs at **100 kHz**. The sensor is read once a minute, and
  slower edges survive a long cable far better than a fast bus would.
- The firmware uses single-shot measurements with the internal heater off. Continuous
  high-repeatability sampling warms the die by a few tenths of a degree, which
  is exactly the error this screen is supposed to avoid.
- **Placement decides the accuracy**, not the sensor. The frame itself is a heat
  source: the panel warms the air behind it, which rises. Mount the sensor on
  the lower edge, deported on its wires, with an opening to the outside air. A
  sealed cavity behind the panel ends up reporting the frame temperature.
- GPIO6 and GPIO7 have no boot function on the ESP32-C6 and are free. GPIO6 is
  also an ADC1 channel; using it for SDA is why the light sensor takes GPIO2.

---

## 9. Wi-Fi

The module's on-board antenna is used; nothing to wire. Expect peaks of a few
hundred milliamperes on the 3.3 V rail while the radio transmits, on top of
the LED supply. The first connection result is flashed on the matrix after
boot and at the end of the self-test. Network traffic is limited to SNTP
(UDP, hourly) and one small HTTP request to Open-Meteo every 15 minutes.

## 10. To Confirm on the Physical Frame

- [ ] How the ESP32-C6 module gets 3.3 V (dev board regulator or external LDO).
- [ ] Series resistor on the data line and its value.
- [ ] Bulk capacitor on the 12 V rail near the matrix.
- [ ] Position of LED 0 and direction of row 0 in viewing orientation.
- [ ] Whether BI of LED 0 is connected.
- [ ] Measured current at full white and at typical animation brightness.
- [ ] Whether a single 12 V injection point is enough for 256 LEDs (voltage drop across the chain).
- [ ] Inrush behaviour at switch-on once the bulk capacitor is fitted.
- [ ] Ambient light readings in the dark and in full daylight, to set the calibration thresholds.
- [ ] Offset between the SHT31 reading and the real room temperature once the frame is assembled.
