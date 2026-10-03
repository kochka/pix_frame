# PCB Design

**Status:** draft. The board has never been manufactured. Everything marked
*to verify* must be checked before sending files to a fab.

This document describes the carrier board in `hardware/`: what it does, why each
part is there, and how to finish it in KiCad. The circuit it implements is the
one in [`wiring.md`](wiring.md); that document stays the authority on the
electrical design, this one only explains how it is turned into copper.

---

## 1. What the board is

A **carrier**: the ESP32-C6 development board is not redesigned, it sits on the
carrier on two 1×16 female headers (or is soldered flat by its castellations).
The carrier holds everything around it — the 12 V input, the on/off switch, the
buck converter, the matrix connector, the sensor connectors.

What this buys, compared to soldering a bare ESP32-C6-WROOM-1 module:

- no USB-C connector, no USB-serial chip, no 3.3 V regulator, no auto-reset
  circuit, no RF layout to get right;
- the devkit stays replaceable, which matters for a first board;
- only two components on the board are not passive (the MOSFET and the buck
  module), so a mistake costs a jumper wire, not a respin.

What it costs: the frame is 5 mm thicker, and the antenna sits over the carrier
(see section 4.3).

### 1.1 The devkit

The board in use is the **Waveshare ESP32-C6-DEV-KIT-N8**, not the Espressif
DevKitC-1. Waveshare states the two are pin compatible, and the pinouts match
pin for pin; the mechanical outline does not.

| | Waveshare ESP32-C6-DEV-KIT-N8 | Espressif ESP32-C6-DevKitC-1 |
| --- | --- | --- |
| Board | 55.00 × 25.40 mm | 51.80 × 25.40 mm |
| Pad rows, centre to centre | 21.80 mm | 22.86 mm |
| Pads | 2 × 16 at 2.54 mm | 2 × 16 at 2.54 mm |
| First pad from the USB edge | 12.00 mm | 12.12 mm |
| Free board past the last pad, antenna side | 4.90 mm | 1.58 mm |
| Castellated half holes | yes | no |
| USB-serial | CH343 + CH334 | native USB-Serial-JTAG |

**The two are not mechanically interchangeable on this carrier.** The footprint
`pixframe:ESP32-C6-DEV-KIT_Waveshare_2x16_P2.54mm` is drawn for the Waveshare
dimensions above.

---

## 2. Prerequisites

```bash
sudo pacman -S kicad kicad-library
sudo pacman -S kicad-library-3d         # only for the 3D view, 3.2 GiB installed
kicad-cli --version                     # 10.0.6 here
```

Without `kicad-library` the standard symbols and footprints are missing and the
project opens with every part unresolved. `kicad-library-3d` changes nothing
electrically; without it the 3D viewer shows a bare board with pads and no
component bodies.

Open the project with:

```bash
kicad hardware/pix-frame.kicad_pro
```

The project carries its own `sym-lib-table` and `fp-lib-table`, so the
`pixframe` symbol and footprint libraries resolve from `${KIPRJMOD}` with
nothing to configure.

---

## 3. Repository layout

| Path | Contents |
| --- | --- |
| `hardware/pix-frame.kicad_pro` | Project: net classes, trace widths, design rules |
| `hardware/pix-frame.kicad_sch` | Schematic, one A3 sheet |
| `hardware/pix-frame.kicad_pcb` | Board: footprints placed, **not routed** |
| `hardware/pixframe.kicad_sym` | Project symbols: the devkit, the MINI560 |
| `hardware/pixframe.pretty/` | Project footprints: the devkit landing pattern, the MINI560 |
| `hardware/gen/` | The scripts the files were generated from, see section 11 |

---

## 4. Design decisions

### 4.1 The devkit is powered on its 5 V pin

The MINI560 output goes to the devkit's **5V** pin, and the devkit's own
SGM2212-3.3 LDO produces the 3.3 V that the board's sensors use.

Feeding 5 V while a USB cable is plugged in is safe: on the Waveshare board
`VBUS` reaches the 5 V rail through a **B5819WS Schottky** (D1 on its
schematic), so nothing can flow back into the USB host. When both sources are
present the MINI560 at 5.0 V wins over VBUS minus the diode drop, and the
Schottky simply stops conducting. **No series diode is needed on the carrier**,
and none is fitted.

A second consequence, visible on the same schematic: the CH343/CH334 USB chips
are powered through a P-MOSFET enabled by VBUS, so they draw nothing when the
frame runs from the 12 V supply.

### 4.2 3.3 V comes from the devkit, not from the carrier

The light sensor divider is ratiometric: the ADC compares the divider output
against the same reference that powers the divider. Generating a second 3.3 V
rail on the carrier would put the divider on one rail and the ADC on another,
and every difference between them would read as a change in ambient light.
Pin 31 of the devkit feeds the sensors, and C6/C7 sit next to it.

### 4.3 The PCB antenna

The WROOM-1 module's antenna is at the far end of the devkit, over the last
~7 mm of its PCB. On this carrier that area lands over copper, which detunes the
antenna and costs range.

Three things are done about it:

1. **A cutout.** `Edge.Cuts` carries a second rectangle, 46.5 → 57.0 mm in x and
   16.0 → 30.0 mm in y, which removes the board between the two pad rows under
   the antenna. Only two narrow bridges remain, carrying pads 31/32 (3V3 and
   GND) that the devkit itself puts there.
2. **A keepout.** A rule area covering 44.5 → 58.5 mm in x and 8.0 → 38.0 mm in
   y forbids tracks, vias and copper pour. Pads stay allowed, since pads 31/32
   have to live there.
3. **Silkscreen.** `ANTENNA` is printed next to the area so the rule survives a
   future edit.

This is a compromise, not a fix. The clean answer is a carrier that ends before
the antenna, but the devkit puts its 3V3 pad 2 mm inside the antenna zone, so
some board has to reach under it. *To verify: measure the Wi-Fi RSSI with the
frame assembled and compare it with the bare devkit.*

### 4.4 The high-side switch

Exactly the circuit of `wiring.md` section 2.2: `J1 → F1 → Q1(S)`, gate held at
source by R1 (100 k), pulled down through R2 (10 k) and the external switch on
J2. C1 is a footprint for an optional 100 nF gate-source capacitor, **not
fitted**, there to soften the switch-on edge if the inrush into C2 turns out to
be harsh.

Q1 is an IRF4905 in TO-220. At 4 A its 20 mΩ dissipates 0.32 W, which raises the
junction by roughly 20 °C above ambient with no heatsink — comfortable. A
cheaper P-MOSFET at 150 mΩ would dissipate 2.4 W and need one.

### 4.5 The fuse

F1 is a 5 × 20 mm slow-blow fuse, 5 A, on the 12 V input ahead of everything.
It is **not** in `wiring.md`; it is added here because a 12 V supply able to
deliver several amps into a wooden frame deserves one cheap part between it and
the wiring. If you would rather not fit it, solder a wire link across the clips;
nothing else in the design depends on it.

Size it just above the real load: with 256 LEDs at 3.8 A worst case, 5 A is the
right choice. *To confirm once the current is measured on the frame.*

### 4.6 Connectors

| Ref | Part | Why |
| --- | --- | --- |
| J1 | 2-pin screw terminal, 5.00 mm | 12 V in, 10 A rated, accepts a cut barrel-jack pigtail |
| J2 | 2-pin header, 2.54 mm | On/off switch. Carries 0.1 mA, so thin wire is fine |
| J3 | 3-pin screw terminal, 5.00 mm | Matrix: +12 V, DI, GND |
| J4 | 2-pin screw terminal, 5.00 mm | Second 12 V injection for the far end of the chain |
| J5 | 2-pin header | GL5528 photoresistor, deported |
| J6 | 4-pin header | SHT31 breakout: 3V3, SDA, SCL, GND |
| J7 | 2-pin header | External self-test button on GPIO9 |

No barrel jack is fitted. A screw terminal takes the wires from any supply,
including a barrel-jack pigtail, and one connector is cheaper than two.

J7 matters more than it looks: the devkit's own BOOT button ends up buried
behind the panel once the frame is assembled, and GPIO9 is exactly the pin the
firmware reads (`wiring.md` section 6). The Waveshare board wires its BOOT
button between GPIO9 and GND with a footprint for a parallel capacitor, so an
external button in parallel is electrically the same press.

### 4.7 Parts not fitted

Three footprints are marked DNP and show crossed out on the schematic:

| Ref | Value | Fit it when |
| --- | --- | --- |
| C1 | 100 n | Switch-on is harsh and the bulk capacitor inrush needs slowing |
| R5 | 4k7 | The SHT31 is a bare sensor rather than a breakout board |
| R6 | 4k7 | Same |

A breakout board already carries I²C pull-ups; a second pair in parallel would
drop the effective value too far (`wiring.md` section 8).

---

## 5. Placement

Board outline: **100 × 90 mm**, which stays inside the cheapest panel size at
most fabs. Four M3 mounting holes.

```text
  0                                                           100 mm
 0┌───────────────────────────────────────────────────────────────┐
  │ (H1)                            ANTENNA       [J1 12V IN]     │
  │ ┌──────────────────────────────────────┐    ┌──────────────┐  │
  │ │ USB-C     ESP32-C6 devkit (U1)   ┌───┐│    │  F1 fuse     │  │
  │ │                                  │cut││    └──────────────┘  │
  │ │                                  └───┘│      R1  R2   [Q1]   │
  │ └──────────────────────────────────────┘               C1     │
  │                                              [J2 ON/OFF]      │
  │  (C2)   C3        ┌──────────────┐    C4 C5          (H2)     │
  │ 1000uF            │   MINI560    │      R3                    │
  │                   └──────────────┘                            │
  │                                          R5                   │
  │                                          R6                   │
  │            R4   C8                            C9              │
  │                                             C6  C7            │
  │ (H3) [J3 MATRIX] [J4 INJECT]  J5  J7  J6            (H4)      │
90└───────────────────────────────────────────────────────────────┘
```

The reasoning:

- **The devkit is flush with the left edge**, so its USB-C connector is
  reachable without taking the frame apart. Its pad 1 is at (12.5, 12.0).
- **The 12 V chain runs along the top right**, J1 → F1 → Q1, so the highest
  current stays in one corner and never crosses the signal area.
- **The matrix connector is at the bottom left**, next to the bulk capacitor C2,
  because the current step when the panel switches is drawn from C2 and should
  not travel across the board first.
- **The sensor connectors are along the bottom edge**, where their deported
  cables leave the frame, and away from the 12 V path.
- **The MINI560 is in the middle**, short 12 V path from Q1, short 5 V path to
  the devkit, with C4/C5 between it and the devkit's 5 V pin.

Every component is at rotation 0. Screw terminals in particular may want to be
turned so their openings face the board edge: select the part in Pcbnew and
press `R`. That is a five-second edit and worth doing before routing.

---

## 6. Net classes and trace widths

Set in `pix-frame.kicad_pro`, applied by pattern:

| Net class | Pattern | Track | Clearance | Via |
| --- | --- | ---: | ---: | --- |
| `Power12V` | `+12V_*`, `GND` | 2.5 mm | 0.5 mm | 1.2 / 0.6 mm |
| `Power5V` | `+5V`, `+3V3` | 1.0 mm | 0.3 mm | 1.0 / 0.5 mm |
| `Default` | everything else | 0.3 mm | 0.25 mm | 0.8 / 0.4 mm |

Where 2.5 mm comes from. IPC-2221 for an external layer,
`I = 0.048 · ΔT^0.44 · A^0.725` with A in mil²:

| Current | Minimum width, 35 µm copper, ΔT = 10 °C |
| ---: | ---: |
| 1 A | 0.30 mm |
| 2 A | 0.75 mm |
| 4 A | 2.03 mm |
| 5 A | 2.75 mm |

2.5 mm carries about 4.7 A at a 10 °C rise, which covers the 3.8 A worst case of
`wiring.md` section 2.1 with margin. Its resistance is 0.20 mΩ/mm, so the
longest run on this board, about 100 mm, drops 79 mV at 4 A — irrelevant on a
12 V rail, and the same again on the return.

The 5 V rail only feeds the devkit, a few hundred milliamps including Wi-Fi
peaks, so 1.0 mm is already generous; it is chosen for stiffness during transmit
bursts, not for heating.

---

## 7. Routing, step by step

The board ships **placed but not routed**. That is deliberate: placement is the
part that is hard to undo and easy to get wrong, routing is the part where
Pcbnew's interactive router is faster and better than anything written by hand.
Opening the board and seeing 38 unconnected items in DRC is the expected
starting state. The two ground pours are already filled, which is why GND does
not appear in that count.

1. **Open Pcbnew** and check `File → Board Setup → Net Classes`: the three
   classes above should be listed and the patterns assigned.
2. **Turn the connectors** that need it (`R`), then lock the placement:
   `Edit → Select All`, right click, `Locking → Lock`. This stops parts from
   being dragged while routing. Unlock later if you want to move something.
3. **Route the 12 V path first**, on **F.Cu**, with the `Power12V` width:
   `J1.1 → F1.1`, `F1.2 → Q1.3 (source)`, `Q1.2 (drain) → C2.1 → U2.1 →
   J3.1 → J4.1`. Keep it as one fat spine along the top and down the left side.
   High current first means everything else routes around it, not through it.
4. **Route 5 V**: `U2.3 → C4.1 → C5.1 → U1.5`. Keep it short; C4 and C5 sit
   between the converter and the devkit for that reason.
5. **Route 3.3 V**: `U1.31 → C6.1, C7.1, J5.1, J6.1, R5.2, R6.2`. Note that
   `U1.31` is inside the antenna keepout, so leave that pad going straight down
   and away before turning.
6. **Route the signals** last, 0.3 mm: `LED_DATA_MCU` (U1.14 → R3),
   `LED_DATA` (R3 → J3.2), `LIGHT_ADC`, `SDA`, `SCL`, `BTN`, `GATE`, `SW_A`.
   - Keep `LED_DATA` short and away from the 12 V spine.
   - `LIGHT_ADC` is a high-impedance node; route it directly from J5 to R4 to
     C8 to U1.9, and surround it with ground pour.
7. **Do not route GND.** It comes from the pour, next section.
8. **Fill the zones**: `Edit → Fill All Zones`, or press `B`.
9. **Run DRC** (`Inspect → Design Rules Checker`) with *Test for parity between
   PCB and schematic* ticked. Target: zero unconnected items, zero violations.

### 7.1 Ground

Two ground pours are already in the file, one on F.Cu and one on B.Cu, covering
the board minus 0.3 mm and minus the antenna keepout. Ground is not routed as
tracks at all.

For the return current to behave:

- Route **all signals on F.Cu** so B.Cu stays a continuous plane. Every signal
  track on the bottom layer cuts the return path of everything crossing it.
- **Stitch the two pours** with vias, roughly every 10 mm along the 12 V spine
  and around the board perimeter. `Place → Add Via` on the pour, or select the
  zone and use the via stitching tool.
- Check the result after filling: `View → Show Filled Areas`, then look for
  islands the fill could not reach. The devkit and the cutout split the top pour
  into regions; each one needs a via to the bottom plane.

The 3.8 A return runs through the plane, not through a track. A continuous
35 µm plane over 90 mm is far below any width that would heat.

---

## 8. Verification

Everything below runs headless and is what was used to check the files as
committed:

```bash
# Schematic: electrical rules
kicad-cli sch erc --severity-all -o /tmp/erc.rpt hardware/pix-frame.kicad_sch

# Board: design rules plus agreement with the schematic
kicad-cli pcb drc --severity-all --schematic-parity -o /tmp/drc.rpt \
    hardware/pix-frame.kicad_pcb

# Eyeball the schematic and the board
kicad-cli sch export pdf -o /tmp/sch.pdf hardware/pix-frame.kicad_sch
kicad-cli pcb render -o /tmp/board.png --quality high hardware/pix-frame.kicad_pcb
```

State of the files as committed:

| Check | Result |
| --- | --- |
| ERC | 0 violations |
| DRC, design rules | 0 violations |
| DRC, schematic parity | 0 problems |
| DRC, connectivity | 38 unconnected items, i.e. everything except GND is unrouted |

---

## 9. Manufacturing

| Parameter | Value |
| --- | --- |
| Layers | 2 |
| Size | 100 × 90 mm |
| Thickness | 1.6 mm |
| Copper | 35 µm (1 oz) both sides |
| Minimum track / clearance used | 0.3 mm / 0.25 mm |
| Minimum drill | 1.0 mm (header pads) |
| Surface finish | HASL is fine, everything here is through-hole |

Nothing on this board is near any fab's capability limits; the constraining
feature is the 2.5 mm power track, which is the opposite of a constraint.

Export:

```bash
kicad-cli pcb export gerbers -o /tmp/gerbers hardware/pix-frame.kicad_pcb
kicad-cli pcb export drill   -o /tmp/gerbers hardware/pix-frame.kicad_pcb
kicad-cli pcb export bom     -o /tmp/bom.csv hardware/pix-frame.kicad_pcb
cd /tmp/gerbers && zip -r ../pix-frame-gerbers.zip .
```

Check the outline in the fab's viewer before paying: the antenna cutout is a
second closed contour on `Edge.Cuts`, and some fabs need to be told it is an
internal cutout rather than a second board.

---

## 10. Bill of materials

| Ref | Value | Package | Notes |
| --- | --- | --- | --- |
| U1 | ESP32-C6-DEV-KIT-N8 | Waveshare devkit | On 2 × 1×16 female headers, or soldered flat |
| U2 | MINI560, 5 V out | 29 × 18 mm module | Verify the footprint, section 12 |
| Q1 | IRF4905 | TO-220-3 | P-channel, 55 V, 20 mΩ |
| F1 | 5 A slow-blow | 5 × 20 mm + clips | Optional, wire link otherwise |
| C1 | 100 n | disc, 5 mm | **Not fitted** |
| C2 | 1000 µF / 25 V | radial, 10 mm | Bulk on the switched 12 V rail |
| C3, C5, C7, C8, C9 | 100 n | disc, 5 mm | |
| C4, C6 | 10 µF / 16 V | radial, 5 mm | |
| R1 | 100 k | axial, 10.16 mm | Gate to source |
| R2 | 10 k | axial | Gate series |
| R3 | 330 | axial | WS2815 data series resistor |
| R4 | 10 k | axial | Light divider |
| R5, R6 | 4k7 | axial | **Not fitted**, I²C pull-ups |
| J1, J4 | 2-pin screw terminal | 5.00 mm | |
| J3 | 3-pin screw terminal | 5.00 mm | |
| J2, J5, J7 | 2-pin header | 2.54 mm | |
| J6 | 4-pin header | 2.54 mm | |
| H1–H4 | M3 mounting hole | 3.2 mm | Not a part |

Off board: the 12 V supply, the SPST switch, the GL5528 photoresistor, the
SHT31-D breakout, a momentary push button, and the matrix itself.

---

## 11. How the files were generated

`hardware/gen/` holds the scripts that produced the symbol library, the two
footprints, the schematic and the board:

| File | Produces |
| --- | --- |
| `design.py` | The netlist and the board placement — the single description everything else reads |
| `sexp.py` | A small s-expression reader and writer for KiCad files |
| `gen_fp.py` | `pixframe.pretty/` |
| `gen_sym.py` | `pixframe.kicad_sym` |
| `gen_sch.py` | `pix-frame.kicad_sch` |
| `gen_pcb.py` | `pix-frame.kicad_pcb`, reading the netlist back from Eeschema so the board cannot drift from the schematic |

**These are one-shot generators, not a build step.** From here on the KiCad
files are the source of truth; re-running the scripts overwrites them and throws
away any routing. They are kept so the origin of every coordinate is on record,
and so a second board — a larger matrix, a different devkit — can start from a
description rather than from a copy.

The schematic they produce is wired with labels rather than with drawn nets:
each pin gets a short stub and a global label. It is not how a schematic drawn
by hand looks, and rewiring blocks graphically in Eeschema is a normal thing to
do from here.

---

## 12. To verify before ordering

The first four are the ones that would make the board unusable.

- [ ] **The MINI560 footprint.** Its pad geometry is not published. The
      footprint uses 29 × 18 mm overall, pad groups at x = ±13 mm, two holes per
      group 2.54 mm apart at y = ±4.42 and ±6.96 mm, taken from a third-party
      footprint and a published outline. Measure your module with calipers:
      overall length and width, distance between the pad groups on the long
      axis, distance between the two holes of one group. Confirm which edge is
      IN and which is OUT from the module's own silkscreen.
- [ ] **The devkit pad row spacing, 21.80 mm.** Taken from Waveshare's mechanical
      drawing. Measure the real board between the two rows, centre to centre.
- [ ] **The devkit is the Waveshare one**, not the Espressif DevKitC-1; they do
      not have the same outline (section 1.1).
- [ ] **The antenna cutout** clears the module's antenna in both axes once the
      devkit is dropped onto its headers. Print the board at 1:1 and lay the
      devkit on the paper.
- [ ] Connector orientations: every screw terminal's opening faces a board edge.
- [ ] Fuse rating, once the real matrix current is measured.
- [ ] Whether C2 at 1000 µF is enough, or whether a second one is needed at the
      far end of the chain next to J4.
- [ ] The Wi-Fi range with the frame assembled, against the bare devkit.

---

## 13. Open questions carried over from `wiring.md`

These are properties of the frame, not of the board, and this design does not
settle them. They are repeated here because two of them change the copper:

- The measured current at full white decides the fuse and confirms the 2.5 mm
  track width.
- Whether a single 12 V injection point is enough for 256 LEDs decides whether
  J4 is populated at all.
