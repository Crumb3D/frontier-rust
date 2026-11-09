# Frontier: Elite II (Python)

A playable recreation of the 1993 space sim: Kepler orbits on a stardate clock,
Newtonian flight with origin rebasing, flat-shaded software rendering, and the
dock / trade / fight / jump loop.

Original meshes, UI, and copy. Ship specs, equipment list, and trade goods follow
Frontier: Elite II design (public manual / community ship tables). No commercial
game assets.

## Run

```bash
python -m pip install -r requirements.txt
python main.py
```

Starts on the **title screen** (640×400, integer-scaled like Amiga FE2). Enter = start at Sirocco.

## Controls

| Key | Action |
|-----|--------|
| F1 | Flight / station |
| F2 | System orrery |
| F3 | Galactic chart |
| F4 | Status |
| L | Launch (docked) / request dock (flight) |
| W/S | Main / retro thrust |
| A/D | Roll |
| Q/E | Yaw |
| R/F | Vertical thrust |
| Mouse | Pitch / yaw (flight) |
| Z / X | Set-speed up / zero |
| Space | Laser |
| M | Missile |
| Tab | Cycle target |
| +/- | Time compression |
| J | Hyperspace to chart selection |
| F5 / F9 | Save / load |
| Esc | Pause |

Docked: Left/Right tabs, Up/Down select, Enter/B buy, S sell, F fit, R remove, A accept mission.

## Content

- **29 ships** from Shuttle through Panther Clipper (Frontierverse specs)
- **Equipment**: pulse/beam/mining/plasma guns, missiles & mines, ECM, shields,
  drives (class 1–8 + military), fuel scoop, life support, cabins, etc.
- **31 commodities** including disposal goods (radioactives, rubbish) and
  life-support cargo (slaves, live animals)
- Economy bias by Agricultural / Industrial / High-Tech / Extraction / Tourist / Colony
- Shipyard stock gated by system tech level

## Tests

```bash
python tests/test_core.py
```
