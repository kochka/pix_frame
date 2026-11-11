# Mechanical parts

3D-printed rear structure of the frame, which also holds the carrier board.
Each part has an editable CAD source (`.sc`) and a Bambu Studio project
(`.3mf`) holding the print settings; every project contains a single part.
Print one of each side and four corners.

| Part | Role | Qty | Supports |
|---|---|---|---|
| `top_side` | Top edge of the rear structure | 1 | Yes |
| `bottom_side` | Bottom edge of the rear structure | 1 | No |
| `left_side` | Left edge, carries the PCB | 1 | Yes |
| `right_side` | Right edge, holds the switch and the temperature sensor | 1 | Yes |
| `corner` | Corner piece joining two sides | 4 | No |

## Print settings

All projects share the same settings, made for a Bambu Lab X2D with a 0.4 mm
nozzle:

- PLA
- 0.2 mm layer height
- 4 walls
- 25 % gyroid infill
- tree supports where the table says so, as set in each project

Electronic component models used only by KiCad's 3D viewer remain in
[`../pcb/pixframe.3dshapes/`](../pcb/pixframe.3dshapes/).
