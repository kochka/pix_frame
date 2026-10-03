# hardware

KiCad 10 project for the pix-frame carrier board: a Waveshare ESP32-C6-DEV-KIT
on two 1×16 headers, the 12 V supply and its on/off switch, a MINI560 buck
converter, and the connectors for the WS2815 matrix and the two sensors.

```bash
sudo pacman -S kicad kicad-library      # once
kicad pix-frame.kicad_pro
```

The board is **placed but not routed**, apart from the two ground pours which
are filled. Opening it and seeing 38 unconnected items is the expected starting
state; [`../doc/pcb.md`](../doc/pcb.md) explains
the placement, the net classes and the routing order.

| File | Contents |
| --- | --- |
| `pix-frame.kicad_sch` | Schematic, one A3 sheet, ERC clean |
| `pix-frame.kicad_pcb` | Board, 100 × 90 mm, DRC clean apart from the unrouted nets |
| `pixframe.kicad_sym` | Symbols for the devkit and the MINI560 |
| `pixframe.pretty/` | Footprints for the same two parts |
| `gen/` | The scripts the files were generated from. One-shot, not a build step: re-running them overwrites the KiCad files and discards any routing |

Before ordering anything, read section 12 of `doc/pcb.md`. Two footprints are
drawn from published drawings rather than from a part in hand and need to be
checked with calipers.
