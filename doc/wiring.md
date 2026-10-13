# Hardware Reference

Revision A — schematic and PCB in [hardware/pcb/](../hardware/pcb/).
This document covers the circuit, external wiring, component mounting and
commissioning of the pix-frame carrier.

The current display is a 16 × 16 WS2815 matrix. Display dimensions and wiring
belong to the output layer; the [PXA format](PXA_spec.md) is independent of this
particular panel.

## 1. Components and connections

References in this document belong to the carrier. Components already fitted
on the ESP32 devkit, buck converter and sensor module have separate references.

### Carrier components

| Reference | Component | Function / mounting |
| --- | --- | --- |
| U1 | Waveshare ESP32-C6-DEV-KIT-N8 | ESP32-C6-WROOM-1-N8, 8 MB flash; direct castellated mounting |
| U2 | MINI560 buck converter | Switched 12 V input, output set to 5.0 V |
| Q1 | IRF4905 P-channel MOSFET | High-side power switch; horizontal TO-220, tab up |
| D1 | 1N5819 Schottky diode | Buck-output isolation; horizontal, cathode band on the left |
| R1 | 100 kΩ | Q1 gate-to-source pull-up |
| R2 | 10 kΩ | Series resistor between Q1 gate and the external switch |
| R3 | 330 Ω | Series resistor on LED data |
| R4 | 10 kΩ | Light-sensor divider resistor to GND |
| R5, R6 | 4.7 kΩ, DNP | Optional SDA/SCL pull-ups to 3.3 V; leave unpopulated with a suitably pulled-up sensor module |
| C1 | 1000 µF / 25 V electrolytic | Bulk capacitor on the switched 12 V rail; horizontal |
| C2 | 100 nF | Light-sensor ADC filter to GND; populated |
| J1 | Horizontal flat-tab DC jack | 5.5 × 2.1 mm centre-positive input plug |
| J2–J6 | Plated wire-solder pads | Switch, matrix and sensor wiring |
| H1–H4 | 3.2 mm non-plated holes | M3 carrier mounting |

DNP means **do not populate**. R5/R6 are the carrier's optional components.
The carrier contains no fuse and no LED-data level shifter.

### External parts

| Part | Connection |
| --- | --- |
| 12 V / 3 A DC adapter, centre positive | J1 |
| 16 × 16 WS2815 matrix, 256 LEDs | J3 for power and DI; J4 for the first pixel's BI |
| SPST on/off switch | J2 |
| GL5528 photoresistor | J5 |
| SHT31 sensor module, powered at 3.3 V | J6 |

### GPIO map

| GPIO | Function | Carrier connection |
| ---: | --- | --- |
| 2 | Ambient-light ADC input | J5 pad 2, `LIGHT_ADC`, with R4 and C2 to GND |
| 6 | I²C SDA | J6 pad 2; optional R5 pull-up |
| 7 | I²C SCL | J6 pad 3; optional R6 pull-up |
| 9 | Self-test button | Devkit BOOT button; input with internal pull-up |
| 18 | LED data, SPI2 MOSI with DMA | R3 → J3 pad 2, `LED_DATA` |

GPIO9 is also a boot-strapping input. Release BOOT for normal startup.

### External pad order

All directions below refer to the component side, with LIGHT/TEMP at the top
and the DC jack at the bottom. Identify wires by pad number and silkscreen.

| Reference / marking | Pad assignments | Physical order |
| --- | --- | --- |
| J1 / POWER 12V | 1 centre `+12V_IN`; 2 sleeve GND; 3 unused switched sleeve | Opening faces the bottom edge |
| J2 / SWITCH | 1 `SW_A`; 2 GND | Left to right |
| J3 / MATRIX | 1 `+12V_SW`; 2 DI; 3 GND | Bottom to top |
| J4 / BI/GND | 1 GND → first matrix pixel BI | Below J3 |
| J5 / LIGHT | 1 3.3 V; 2 ADC | Left to right |
| J6 / TEMP | 1 3.3 V; 2 SDA; 3 SCL; 4 GND | Left to right |

J2–J6 accept soldered wires rather than connector bodies. Provide strain relief
so cable loads are not carried by the solder joints.

## 2. Power distribution

```text
J1 centre +12 V → Q1 source/drain → +12V_SW ─┬→ J3 matrix +12 V
                                           ├→ C1 positive
                                           └→ U2 MINI560 input

U2 output, +5V_BUCK → carrier D1 → +5V → U1 devkit regulator → +3V3
PC USB             → devkit USB diode ─┘

GND: adapter, carrier, MINI560, devkit, matrix and sensors share one ground.
```

U1 provides the 3.3 V supply for the ESP32 and sensors. Its input bypass,
regulator output capacitors and ESP supply decoupling are on the assembled
devkit. Keep the assembled modules' local decoupling in place; the remote
SHT31 requires supply bypass at the sensor.
The [Waveshare schematic](https://files.waveshare.com/wiki/ESP32-C6-DEV-KIT-N8/ESP32-C6-DEV-KIT-N8-Schematic.pdf)
documents the devkit regulator and USB power path.

### 2.1 USB and buck isolation

Carrier D1 has its anode on `+5V_BUCK` and its cathode on `+5V`. It blocks
current from USB into the MINI560 output. The separate diode on the devkit
blocks current from the carrier's 5 V feed toward USB. Source selection depends
on the voltages after the two diode drops; the circuit does not impose USB
priority.

Set the MINI560 output to **5.0 V before connecting the devkit**. The devkit
input voltage is lower by the carrier diode's forward drop, which varies with
load. Measure the devkit input and 3.3 V rail during commissioning.

| Supply state | ESP32 and sensors | Matrix |
| --- | --- | --- |
| 12 V present, J2 switch closed | Powered through MINI560 and D1 | Powered |
| 12 V present, J2 switch open, no USB | Switched supply off | Switched supply off |
| USB connected, J2 switch open or 12 V absent | Powered from USB | No 12 V supply |
| USB and 12 V present, J2 switch closed | Both diode-isolated input paths available | Powered from 12 V |

The firmware has no input indicating the state of the matrix's 12 V supply.
USB can therefore power the data driver while the matrix is off. R3 limits
data-input injection current but does not electrically isolate the matrix.
Keep DI low whenever the matrix supply is off.

### 2.2 On/off switch

Q1 switches the positive supply; GND remains connected. Q1 pin 3 is the source
on `+12V_IN`, pin 2 is the drain on `+12V_SW`, and pin 1 is the gate.
R1 connects gate to source. R2 connects the gate to J2 pad 1; the external
switch connects that pad to J2 pad 2, GND.

With the switch open, R1 holds the gate at the source potential and Q1 turns
off. With the switch closed, R1/R2 pull the gate below the source and Q1 turns
on. From the 100 kΩ / 10 kΩ divider at 12 V, the nominal steady-state switch
current is `12 V / 110 kΩ ≈ 0.109 mA`. The external switch carries gate-control
current, not matrix current.

R2 limits gate charging current and affects the switching edge. The circuit
does not regulate startup current. C1 is connected between `+12V_SW` and GND,
so it charges when Q1 turns on.

Q1's exposed metal tab is connected to `+12V_SW`. Keep metal screws, inserts
and supports electrically clear of the tab and its package mounting hole.

### 2.3 Supply capacity

The adapter is rated **12 V / 3 A**. Its load includes the matrix and the
MINI560 input. Brightness settings are not current measurements; no measured
full-white current, startup peak or thermal limit is recorded for the assembled
carrier. Establish the permitted brightness from measurements with the actual
panel, wiring and enclosure.

The carrier has no fuse or dedicated current limiter. Adapter overload
protection and cable current ratings must be checked for the assembled frame.
Changing the supply rating or matrix requires re-evaluating the power path.

## 3. LED data signal

```text
ESP32-C6 GPIO18 → R3, 330 Ω → J3 pad 2 → first WS2815 DI
GND            → J3 pad 3 → matrix GND
GND            → J4 pad 1 → first WS2815 BI
```

The carrier drives DI directly at 3.3 V and has no level shifter. R3 is the
carrier's 330 Ω data resistor. Keep DI short and route it with a sound ground
connection. The first pixel's BI is tied to GND through J4, following the
[Worldsemi WS2815B application circuit, page 4](https://www.peace-corp.co.jp/data/WS2815B_V1.0_EN_18112616281473.pdf).

### Firmware waveform

The values below are generated by [firmware/src/leds.rs](../firmware/src/leds.rs).
They are nominal timing calculations from the configured SPI clock, not
oscilloscope measurements.

| Parameter | Value |
| --- | --- |
| SPI clock | 3.2 MHz |
| Encoding | Four SPI bits per LED bit, 800 kbit/s |
| LED bit 0 | `1000`: 312.5 ns high, 937.5 ns low |
| LED bit 1 | `1100`: 625 ns high, 625 ns low |
| Reset | 120 zero bytes, 300 µs low |
| Channel order | GRB, most-significant bit first |
| Output | SPI2 MOSI on GPIO18, DMA transfer; no clock or chip-select wire |

The [WS2815B V1.0 datasheet, page 3](https://www.peace-corp.co.jp/data/WS2815B_V1.0_EN_18112616281473.pdf)
specifies a bit-1 low time of 220–420 ns;
the firmware's 625 ns low time is outside that interval. It also does not
establish a guaranteed 3.3 V input margin. The current direct-drive circuit
and waveform must be checked with the actual panel; these settings are not
a claim of conformance to every WS2815 variant.

## 4. Physical LED layout

[firmware/src/main.rs](../firmware/src/main.rs) configures a 16 × 16 serpentine
matrix. [firmware/src/display.rs](../firmware/src/display.rs) maps logical
coordinates, with `(0, 0)` at the top-left, X increasing rightward and Y downward:

```text
even row: physical_index = y × width + x
odd row:  physical_index = y × width + (width - 1 - x)
```

This mapping requires pixel 0 at the top-left, row 0 running left to right,
and rows chained from top to bottom. Check the installed panel with the
self-test. A panel with a different orientation or wiring requires the
corresponding display mapping; PXA files retain logical row-major pixels.

## 5. Logical-to-physical output

```text
PXA palette index → RGB palette lookup → brightness processing
→ logical-coordinate mapping → physical LED index → GRB encoding
→ SPI DMA on GPIO18 → WS2815 chain
```

Panel dimensions and wiring are display parameters. Larger or chained panels
require appropriate mapping, output capacity and power sizing without changing
the animation format. The current firmware's `LAYOUT` is set to 16 × 16.

## 6. Self-test button

The Waveshare devkit's BOOT button connects GPIO9 to GND. The firmware enables
the internal pull-up and reads a low level as a press. A press runs the LED
self-test; animation playback then resumes. No external self-test button is
connected to the carrier.

Holding BOOT during reset selects download mode. RESET acts on the devkit's
enable/reset input rather than the self-test GPIO. Keep both buttons accessible
in the assembled frame.

## 7. Ambient light sensor

Connect the GL5528 between the two LIGHT pads. The carrier contains R4 and C2:

```text
J5 pad 1, 3.3 V → GL5528 → J5 pad 2, LIGHT_ADC → GPIO2 / ADC1
                                  ├→ R4, 10 kΩ → GND
                                  └→ C2, 100 nF → GND
```

The divider voltage is `3.3 V × R4 / (R_GL5528 + R4)`. It rises as the
photoresistor's resistance falls with increasing light. C2 filters the ADC
node. This circuit measures a voltage for brightness control; it has no lux
calibration.

Only the photoresistor is remote. Use two wires to J5 and shield the cell from
the matrix's own light. An opaque mounting tube facing the room prevents light
from the panel from feeding back into the brightness control.

[firmware/src/light.rs](../firmware/src/light.rs) averages and filters ADC
samples, logs the measured voltage every 10 seconds, and adjusts brightness
when a valid `[light]` section is present in the device configuration. Record
`dark_mv` and `bright_mv` with the frame assembled in its operating position.
These thresholds map to `min_brightness` and `max_brightness`. Without valid
calibration, playback retains its fixed brightness.

## 8. Temperature and humidity sensor

Connect the SHT31 module to TEMP/J6: **3.3 V, SDA, SCL, GND**, in that order.
SDA uses GPIO6 and SCL uses GPIO7. The firmware uses **100 kHz I²C**, address
**0x44**, and single-shot measurements approximately once per minute. Its
measurement command is `0x2400`, with high repeatability and no clock stretching.
See [firmware/src/indoor.rs](../firmware/src/indoor.rs).

Use a module configured for address 0x44, with suitable SDA/SCL pull-ups to
3.3 V and local supply bypass. R5 and R6 remain **unpopulated** when those
pull-ups are present. Fit the carrier's 4.7 kΩ pair only when the bus needs
them; account for any existing pull-ups in parallel.

R5/R6 pads are plated through-holes. Each hole connects its own front and back
copper even without a resistor or solder, so leaving the resistors out does
not interrupt the routed signals. Do not bridge the two pads of either
resistor: that would connect SDA or SCL directly to 3.3 V.

The sensor's bypass capacitor belongs at the remote sensor. Mount the module
where room air reaches it, away from heat from the matrix, buck and ESP32.
The indoor screen is omitted when no sufficiently recent sensor reading is
available; it does not require Wi-Fi.

## 9. PCB and mechanical dimensions

### Board geometry

| Feature | Dimension / specification |
| --- | --- |
| Carrier outline envelope | 54 × 145 mm |
| Copper layers | Two, F.Cu and B.Cu |
| Board thickness | 1.6 mm |
| Copper specified for the design | Nominal 35 µm per layer |
| Mounting holes | Four 3.2 mm NPTH holes |
| Hole copper/track/via keepout | 8.4 × 8.4 mm around each mounting hole |
| Available assembly height | 15 mm total, including screw heads and solder joints |

Coordinates in the following tables are in millimetres relative to the
carrier's **top-left corner**, viewed from the component side. KiCad's absolute
origin for that corner is `(100, 50)` mm: add that offset when using the editor.

| Feature | Position / extent |
| --- | --- |
| H1, H2 centres | (5, 5), (49, 5) |
| H3, H4 centres | (5, 140), (49, 140) |
| Mounting-centre rectangle | 44 × 135 |
| Antenna notch, open to the right | X = 46…54, Y = 38…52 |
| Antenna copper/track/via keepout, both layers | X = 43…54, Y = 36…53.8 |
| MINI560 footprint origin | (19.54, 85.00), rotation 90° |
| D1 cathode / anode | (7.00, 65.00) / (17.16, 65.00) |
| R3 pad 1 / pad 2 | (28.00, 65.00) / (38.16, 65.00) |
| Q1 gate / drain / source pads | (13.00, 105.00) / (15.54, 105.00) / (18.08, 105.00) |
| C1 positive / negative pads | (43.00, 101.00) / (38.00, 101.00) |

### Waveshare devkit mounting

The carrier footprint is for the **Waveshare ESP32-C6-DEV-KIT-N8**. It reserves
a 55 × 25.4 mm board envelope, with 21.8 mm between contact rows, 2.54 mm pitch
and the first contact 12 mm from the USB edge. Lands are 1.8 × 4.6 mm, front copper
and mask only. Remove any pre-fitted headers before direct castellated mounting.
Waveshare documents this mounting method on its
[product page](https://docs.waveshare.com/ESP32-C6-DEV-KIT-N8).

USB is accessible at the left edge. The antenna extends beyond the right edge
over the open notch. Keep metal, cables and supports away from the antenna;
verify Wi-Fi reception with the completed enclosure.

### MINI560 mounting

The module body measures **22.5 × 17 mm**. Four 4.9 × 5.0 mm SMD lands cover
the paired same-net corner contacts. The carrier has no header holes or
solder-paste apertures for U2. Lands extend 2.5 mm beyond the module's input
and output edges for hand soldering; the courtyard extends 0.25 mm past the lands.

In the installed orientation, **input is below, output is above, positive is
on the right, and negative is on the left**:

| Pad | Function | Position on module | Carrier coordinate |
| --- | --- | --- | --- |
| 1 | IN+ / `+12V_SW` | Bottom right | (25.54, 96.30) |
| 2 | IN− / GND | Bottom left | (13.54, 96.30) |
| 3 | OUT+ / `+5V_BUCK` | Top right | (25.54, 73.70) |
| 4 | OUT− / GND | Top left | (13.54, 73.70) |

The library footprint's unrotated coordinates are IN+ `(-11.30, +6.00)`,
IN− `(-11.30, -6.00)`, OUT+ `(+11.30, +6.00)` and OUT− `(+11.30, -6.00)`.
Those are component-side coordinates. Check underside clearance and solder
access before mounting the module.

### DC input jack

J1 uses `pixframe:BarrelJack_PJ102AH_Horizontal` for a flat-tab jack
with a **measured 9 × 14 mm body** and **0.35 mm terminal thickness**. The
table below defines its PCB mounting interface; the library name is an identifier.
The jack opening faces the bottom edge, approximately 0.5 mm inboard.

Local coordinates below refer to J1 before rotation; rear/front are relative
to its opening. Its footprint origin is carrier coordinate `(16, 130.8)` mm.

| Pad | Terminal / function | Local centre | Measured tab width | Plated slot | Copper pad |
| --- | --- | --- | --- | --- | --- |
| 1 | Rear, centre +12 V | (0, 0) | 2.5–2.6 mm | 3.0 × 0.8 mm | 4.0 × 2.6 mm, rectangular |
| 2 | Front, sleeve GND | (0, 6) | 2.2–2.3 mm | 3.0 × 0.8 mm | 4.0 × 2.6 mm, oval |
| 3 | Side, switched sleeve, unused | (4.7, 3) | 2.0 mm | 0.8 × 3.0 mm | 2.6 × 4.0 mm, oval |

The side terminal is connected to the sleeve without a plug and opens when
a plug is inserted. It is electrically unused on the carrier. Slots 1/2 run
along local X, and slot 3 along local Y. The nominal copper annulus is at
least 0.5 mm. Installed jack height is not recorded; measure it as part of
the complete 15 mm assembly-height check.

### Capacitor and wire pads

C1 mounts horizontally. Its footprint reserves a **maximum Ø10.5 × 26 mm
body**, **5 mm lead pitch**, and **2 mm clearance beyond the vent**. Leads face
the lower power section. Support the leads while bending, secure the body,
and leave the vent clear. These are footprint allowances; check the capacitor
being assembled against them.

| Wire pads | Copper / drill | Pitch |
| --- | --- | --- |
| J3 pads 1 and 3, matrix power | 4.5 mm / 1.8 mm | J3 pads spaced 6 mm |
| J3 pad 2, DI | 2.8 mm / 1.1 mm | J3 pads spaced 6 mm |
| J4, BI/GND | 2.8 mm / 1.1 mm | 6 mm below J3 pad 1 |
| J2, J5, J6 | 2.8 mm / 1.1 mm | 4 mm |

All wire pads and J1 slots are plated. H1–H4 and Q1's package mounting hole
are non-plated. Drill diameter alone does not specify a wire current rating.

### Routing

The board contains **68 track segments and 7 vias**, with filled GND zones on
both copper layers. Q1's drain feed to MINI560 IN+ is on F.Cu without a via.
The matrix feed is on F.Cu; the back plane also carries signal and 5 V routes.
The four devkit GND contacts have local ground vias.

| Net class | Nets | Preferred track width | Clearance | Via diameter / drill |
| --- | --- | --- | --- | --- |
| Power12V | `+12V_*`, GND | 2.5 mm | 0.5 mm | 1.2 / 0.6 mm |
| Power5V | `+5V`, `+5V_BUCK`, `+3V3` | 1.0 mm | 0.3 mm | 1.0 / 0.5 mm |
| Default | Signals | 0.3 mm | 0.25 mm | 0.8 / 0.4 mm |

Widths in the table are routing presets. Q1 has 1.8 mm wide source and drain
necks, 3 mm and 6.86 mm long respectively. The R1 branch is 1 mm wide; local
GND escapes use 0.8 mm tracks with 1.0 / 0.5 mm vias.

J1/J3/C1 power ground pads and MINI560 ground lands connect solidly to the
planes; allow sufficient heat when soldering. The rules and geometry do not
establish measured current capacity or enclosure temperature limits.

## 10. Assembly and commissioning

Print `target/pcb/revA/placement-1to1.pdf` at **100% / actual size** and check
the 54 × 145 mm outline with a ruler. Compare component bodies and terminals
with the print before soldering.

M3 screws enter from below into inserts in a support above the board. Include
screw heads, trimmed leads, PCB thickness, component bodies, insulation and
adhesive in the **15 mm total height**. Support the carrier beside components
and mounting holes. Check USB-plug and BOOT/RESET-button access.

Component 3D models are visual references. The capacitor model is a body
envelope and the jack model is nominal; they do not establish the assembled
height or module clearance. Mechanical CAD and printable parts belong in
[hardware/mechanical/](../hardware/mechanical/); no printable support model is
currently supplied there.

Commissioning measurements and assembled-carrier test results are not recorded
in this repository. Complete these checks on the physical frame:

1. Check mounting fit, module underside clearance, solder access, C1 polarity,
   jack insertion and the full assembly height. Keep Q1's live tab clear of metal.
2. Verify pad-to-wire connections, common GND and the first pixel's BI-to-GND
   connection. Check sensor-module pull-ups and local bypass; leave R5/R6 empty
   when suitable pull-ups are present.
3. Set MINI560 to 5.0 V. Test USB alone, switched 12 V alone and both together;
   measure devkit input/3.3 V rails during Wi-Fi activity and check data behavior
   with the matrix supply off.
4. Measure startup current, operating current, voltage drop at the matrix,
   Q1/track heating and brightness limits in the enclosure. Check cable ratings,
   strain relief and adapter overload behavior.
5. Run the self-test to check panel orientation, channel order and reliable
   data reception. Check DI voltage/timing with the actual panel. Calibrate
   ambient-light thresholds, compare the SHT31 reading with room conditions,
   and check Wi-Fi reception in the assembled frame.

## 11. Design files and verification

| Path | Contents |
| --- | --- |
| [hardware/pcb/pix-frame.kicad_pro](../hardware/pcb/pix-frame.kicad_pro) | KiCad project and design rules |
| [hardware/pcb/pix-frame.kicad_sch](../hardware/pcb/pix-frame.kicad_sch) | Electrical schematic |
| [hardware/pcb/pix-frame.kicad_pcb](../hardware/pcb/pix-frame.kicad_pcb) | Routed carrier with saved zone fills |
| `hardware/pcb/pixframe.kicad_sym`, `pixframe.pretty/` | Project symbols and footprints |
| `hardware/pcb/pixframe.3dshapes/` | Component models for the KiCad viewer |
| `target/pcb/revA/` | Review PDF, images and ERC/DRC reports; ignored by Git |
| `hardware/pcb/production/` | Generated fabrication outputs; ignored by Git |

Checks on the saved design, **2026-10-14, KiCad 10.0.7**:

| Check | Result |
| --- | --- |
| Schematic ERC | 0 errors, 0 warnings |
| PCB DRC with schematic parity | 0 errors, 0 warnings |
| Unconnected items | 0 |
| Schematic/PCB parity issues | 0 |

These results cover the configured KiCad checks on the saved files. They do
not constitute physical assembly, power, thermal or RF test results.

Open the project from the repository root:

```bash
kicad hardware/pcb/pix-frame.kicad_pro
```

Refill zones and save after layout changes. Run ERC/DRC on that saved design
and regenerate review/fabrication files before using them. From the repository
root, with absolute source paths for the parity check:

```bash
kicad-cli sch erc --severity-all -o /tmp/erc.rpt "$PWD/hardware/pcb/pix-frame.kicad_sch"
kicad-cli pcb drc --severity-all --schematic-parity -o /tmp/drc.rpt "$PWD/hardware/pcb/pix-frame.kicad_pcb"
kicad-cli sch export pdf -o /tmp/sch.pdf hardware/pcb/pix-frame.kicad_sch
kicad-cli pcb render -o /tmp/board.png --quality high hardware/pcb/pix-frame.kicad_pcb
```

A fabrication package must include copper, solder-mask, silkscreen, outline
and drill files. Preserve the distinction between plated holes/slots and
non-plated holes, and check the exported drill map and board preview.
R5/R6 use ordinary plated through-holes. The carrier has no castellated holes
on its outline; the devkit's castellations are part of the assembled devkit.

## 12. References

- [Waveshare devkit documentation](https://docs.waveshare.com/ESP32-C6-DEV-KIT-N8)
- [Waveshare schematic: regulator, USB diode and BOOT button](https://files.waveshare.com/wiki/ESP32-C6-DEV-KIT-N8/ESP32-C6-DEV-KIT-N8-Schematic.pdf)
- [Worldsemi WS2815B V1.0: waveform and application circuit](https://www.peace-corp.co.jp/data/WS2815B_V1.0_EN_18112616281473.pdf)
- [JLCPCB Gerber and drill export guide](https://jlcpcb.com/help/article/how-to-generate-gerber-and-drill-files-in-kicad-9)
