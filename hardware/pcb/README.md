# Carrier PCB

KiCad 10 project for the pix-frame revision A carrier: **54 × 145 mm**, two
copper layers, **1.6 mm** board thickness. The Waveshare ESP32-C6-DEV-KIT-N8
mounts directly by its castellations. The carrier includes an IRF4905 high-side
switch, MINI560 5 V converter, 1N5819 isolation diode, flat-tab DC jack and
matrix/sensor/switch wire-solder pads. C1 and Q1 mount horizontally.

The [hardware reference](../../doc/wiring.md) documents component values,
pad assignments, module polarities, mechanical dimensions and commissioning.

Open the project from the repository root:

```bash
kicad hardware/pcb/pix-frame.kicad_pro
```

| File | Contents |
| --- | --- |
| `pix-frame.kicad_sch` | Electrical schematic |
| `pix-frame.kicad_pcb` | Routed carrier with filled GND planes |
| `pix-frame.kicad_pro` | Net classes and design rules |
| `pixframe.kicad_sym` | Devkit and MINI560 symbols |
| `pixframe.pretty/` | Module, horizontal capacitor, DC jack and wire-pad footprints |
| `pixframe.3dshapes/` | Component models for the KiCad 3D viewer |
| `fp-lib-table`, `sym-lib-table` | Project-local library registrations using `${KIPRJMOD}` |

Generated review files belong in `target/pcb/revA/` at the repository root.
Fabrication outputs belong in `hardware/pcb/production/`. Both directories,
local editor state and backups are ignored by Git. Refill zones, save and
run ERC/DRC with schematic parity before regenerating outputs.

Printable parts and their CAD sources belong in [../mechanical/](../mechanical/).
