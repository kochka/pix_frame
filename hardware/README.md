# Hardware

- [`pcb/`](pcb/): KiCad carrier-board project, symbols, footprints and component
  models for the PCB's 3D view.
- [`mechanical/`](mechanical/): CAD sources and printable parts for the frame and
  PCB support.

Open `hardware/pcb/pix-frame.kicad_pro` in KiCad from the workspace root.
See the [hardware reference](../doc/wiring.md) for the circuit, wiring,
carrier dimensions, assembly and commissioning.

Version the design sources and required project libraries. Generated review
files belong under `target/pcb/`; fabrication exports belong under
`hardware/pcb/production/`. Both output directories, local editor state,
backups and Fabrication Toolkit preferences are ignored by Git.
