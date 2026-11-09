# Frontier — what is in the build

A Python / pygame recreation of Frontier: Elite II. Newtonian flight, a clock-driven galaxy, and an Amiga-style console. Run it with `python main.py` from this folder (needs `pygame` and `numpy`).

The window is a 640×400 view, scaled up. Save file is `savegame.json`.

## Opening

The game starts on a **FRONTIER / ELITE II** title. Any key or click skips the fade-in. The menu is **START NEW GAME**, **LOAD GAME**, and **QUIT**. Load stays grey, marked **NO SAVED POSITION**, when `savegame.json` is missing.

**START NEW GAME** opens the career screen: Ross 154, Sol, and Lave. The panel is filled from the simulation (system, planet, moon, starport, Eagle Mk I, 100 Cr). **BEGIN CAREER** or Enter starts that career. Escape returns to the title. Short fades sit between these screens.

You begin docked. Launch clearance comes from **F4 Comms → Launch Request**, then **F7**. Launching without clearance is allowed and draws a fine.

## Flight

- Pitch: **A** / **Z**. Roll: **comma** / **period**. Thrust: **Enter** forward, **Right Shift** back. **W** / **S** also thrust.
- Right mouse button plus mouse movement steers.
- **Space** fires the laser. **M** fires a missile. **Tab** cycles targets.
- **F1** front view, **F2** rear, **F3** external.
- **F9** landing gear. **F10** toggles name and distance labels on bodies.
- **L** requests docking when you are slow and close to a station or starport. Gear-down and slow over a surface starport lands you.
- Set speed pushes only along the nose. Lateral momentum is left alone, so cutting the engines leaves you coasting.
- Extra mass from fittings, cargo, and fuel reduces acceleration.
- Direction markers in the view: star, planets, moons, stations, starports. Boxes when they are on screen, chevrons when they are off the edge. The autopilot target is highlighted.
- The centre of the bottom console is the 3D scanner (ships, stations, starports within about 120 km).

Traffic depends on the system government. Anarchy spawns pirates and traders. Democracy and Corporate spawn police and traders, and a pirate only if you are wanted. Other governments are mixed. Police attack only if you are wanted. Destroying a pirate pays a bounty. Kills advance the combat rating: Harmless, Mostly Harmless, Poor, Below Average, Average, Above Average, Competent, Dangerous, Deadly, ELITE (6,000 kills).

## Ships, mass, and fuel

Hyperdrive range and fuel follow the Frontier FAQ. Maximum range is `200 × h² / mass` light years. A class 8 drive is treated as class 9. Fuel for a jump is `ceil(distance × mass / 200)` tonnes, taken from hydrogen or military fuel **in the hold**. A full-range jump takes 7 days. Shorter jumps take a proportional share of that time.

The internal tank feeds the engines only. The autorefueller, if fitted, moves 1 t from the hold into that tank when it runs low. A fuel scoop fills the hold first, then the tank, while you are in a gas giant's atmosphere. Military fuel leaves 1 t of radioactives in the hold.

Equipment uses the FAQ weights, so a class 1 drive is 10 t and heavier weapons take real hold space. Free cargo is what remains after fittings and fuel. Shields are 16 t each. A pulse laser does 12.5 t of damage per MW.

The manual calls the Eagle a 25 t ship with 20 t of hull capacity. The included fit is 15 t (class 1 drive, 1 MW pulse laser, two homing missiles, atmospheric shielding, and the autopilot) plus 1 t of fuel, which leaves a 4 t cargo bay. Fully laden, that is 25 t and an 8 ly class 1 jump. A lighter load jumps further. Acceleration stays at 25 g forward and 10 g reverse. Hyperspace fuel is hydrogen in the hold; the 1 t internal tank is for the engines.

Other fittings that change play: atmospheric shielding (re-entry), escape capsule (a new Eagle at the nearest port if the ship is lost), hull auto-repair, energy booster (faster shield recharge), passenger cabin, and the 30 MW mining laser. The manual's MB4 hopper holds 10 t. Press **D** on an uninhabited world with no starport to unload it, then **D** again to take the hopper and the rig. The wait before the hopper is full is not given in the manual, so this build uses 60 days. The 30 MW mining laser is for breaking asteroids, not for creating cargo by itself.

## Autopilot and maps

**F6** opens the system map. Click a planet, moon, station, or starport to lock it. The star itself is only a view centre. **F7** engages autopilot: it turns to face the target, then flies, and docks when it reaches a station or starport. **F7** again returns to manual.

**F5** or **F8** opens the galactic chart. Click a star other than your current system to set the hyperspace target. **H**, **Enter**, or **F8** again jumps if the drive can reach it and the hold has the fuel. If the target is beyond one jump, the message names the next hop on a route. The jump plays a star-streak tunnel, then drops you in deep space, several AU past the outermost planet. **Space**, **Enter**, or **Esc** skips the animation. From there, use **F6** and **F7** to reach a world.

Docking and surface landing play a short bay or pad sequence before the station screens.

## Station

Tabs: Comms, Stock, Upgrades, Ships, Bulletin board, Pads, Status, Police.

- Stock: **B** buy, **S** sell. Stations pay you to take rubbish and radioactives.
- Upgrades and ships use the usual fit / buy keys. A class 8 hyperdrive fits only a Boa or a Panther.
- The bulletin board mixes cargo deliveries and passenger runs. Passengers need a free cabin. Cargo contracts need free hold space. Accepted contracts stay in the save.
- **I** opens the cargo inventory, docked or in flight. **Jettison** (or **J**) dumps one tonne. Dumping rubbish or radioactives while docked, or within 40 km of a port, is a 100 Cr fine. In open space it is clean.
- On the Rubbish line, a click on **Jettison** dumps it. A click just underneath that button, where no control is drawn, adds 1 tonne of cargo capacity and leaves the rubbish in the hold. It can be repeated.

## Time

The Stardreamer buttons on the left of the console (and **Shift+F1**–**F5**) set ×1, ×10, ×100, ×1,000, and ×10,000. The first button pauses. The clock only drops back to ×1 when a hostile ship is within 20 km, or you are within 8 km of a surface. Hyperspace time is added to the stardate when the jump completes.

## Cheat console

**`** (backtick) or **F12** opens it. The first time you open it, purchases become free. Selling cargo, removing equipment, and trading in a ship still pay. The top entry turns free purchases off again.

The header shows credits, current jump range, hydrogen in the hold, combat rating, and legal status.

Also in the list: credits, 1 t rubbish, internal tanks plus 30 t hydrogen, 10 t military fuel, 5 t minerals, full repair, god mode, infinite fuel (jumps do not spend hold fuel while this is on), clear or set wanted status, launch clearance, gem stones, a test fitting kit (autopilot, autorefueller, atmospheric shielding, fuel scoop, escape capsule, passenger cabin, radar mapper, energy booster, shield, hull repair), a nose mining laser, a passenger, a delivery contract with 1 t of computers, the next combat rank, ELITE, a fitted Panther Clipper with 50 ly range, a 50 ly range override, teleport to the nearest station, a pirate raid, wipe pirates for their bounty, and advance the stardate 30 days.

## Save

`savegame.json` stores credits, ship, fittings, cargo, bonus hold, fuel, hull, legal status, fines, kills, accepted contracts, system, dock or flight state, and the stardate.

## Layout

- `main.py` — game loop, flight, docking, hyperspace, cheats
- `sim/frontier.py` — FAQ range, fuel, shield, and combat-rank formulas
- `sim/` — galaxy, flight, combat, AI, market, shipyard, missions, navigation, clock
- `render/` — console, HUD, scanner, chart, orrery, title, cinematics, icons
- `ui/screens.py` — station screens and cargo inventory
- `data/` — stars, ships, equipment, goods
- `audio/` — music and effects
