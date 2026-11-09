# Source notes

Compared the 1994 FAQ ship table, the GameFAQs / Frontierverse shipyard table, and the Amiga manual text (Abandonware DOS copy of the manual; the Lemon Amiga page itself is behind a bot check).

## Eagle Mk I

Manual: a 25 t ship, 20 t hull capacity, 15 t of equipment included, 4 t cargo bay, 1 t fuel, class 1, 8 ly, 25 g, 1 MW pulse, two KL760 missiles, atmospheric shielding, autopilot.

Those 15 t are the class 1 drive (10), pulse laser (1), two missiles (2), atmospheric shielding (1) and autopilot (1). Fuel is the separate 1 t. Fully laden mass is 5 t structure + 20 t capacity = 25 t, which is the GameFAQs Mass column. The 8 ly range is that laden ship. The FAQ equation `200 × h² / mass` gives 8 ly at 25 t and a longer jump when the 4 t bay is empty.

## Hawk and Kestrel

The FAQ lists hull 5 t for both, and says those masses do not add up. GameFAQs lists Hawk mass 18 t, capacity 7 t, jump 11.1 ly, and Kestrel mass 20 t, capacity 9 t, jump 10 ly. Both jumps match `200 / mass`. This build uses the GameFAQs mass (structure = mass − capacity) so the published jump is the laden jump. The FAQ hull of 5 t is not used.

## Boa and Panther

GameFAQs: Boa mass 1,500 t, capacity 1,300 t, class 8, jump 10.8 ly. Panther mass 2,000 t, capacity 1,775 t, class 8, jump 9.1 ly.

`200 × 81 / 1500` is 10.8 ly, so the Boa matches the FAQ's statement that a class 8 drive is a class 9 for fuel. `200 × 81 / 2000` is 8.1 ly, not the published Panther range of 9.1 ly. The FAQ already called the Panther class 8 figures inconsistent. The Panther keeps the published 9.1 ly at 2,000 t, and that range scales with the current mass. It is not replaced by 8.1 ly.

The FAQ hull/capacity pair for the Panther (400 / 2,000) is a different split from GameFAQs (mass 2,000, capacity 1,775). The shipyard uses the GameFAQs pair.

## Mamba and Imperial Explorer

Neither name is in the FAQ ship table, the GameFAQs/Frontierverse shipyard table, or the manual's Eagle chapter. They are left as they were. No specs were invented for them.

## MB4

The manual says the hopper holds 10 t and that you leave the rig and come back. It does not say how long a full hopper takes. The 60 day wait is a recreation assumption. The 10 t figure is from the manual.

## Equipment prices

The FAQ and the manual give weights and functions. They do not give a price list. Existing prices were not changed.

## Sirocco, 1-Jan-3200 (DOS original, read off screen)

- market.json: every Stockmarket price and stock figure.
- equipment.json: fit and removal prices and weights of the 13 items on the Sirocco Station Shipyard Upgrades list; the mines are named XB13 Dummy Prox. Mine and XB74 Proximity Mine as there.
- ships.json: Constrictor 143000, Puma Clipper 1265000, Tiger Trader 472000; part exchange for the starting Eagle is 36100.
- observed.json: the twelve bulletin-board adverts, the two Federal HQ missions and the four ships on sale.
- market.json prices are relative to Ross 154 as stars.json describes it (Mining, tech 6); other systems scale from there, which is not the original's formula.
- Guessed, not read off screen: sector coordinates for Epsilon Indi, Barnards Star, Delta Pavonis, Arcturus and Wolf 630; the name and reason tables used for random adverts at other ports. The registration codes of the Puma, the second Constrictor and the Tiger were not opened. The first Constrictor's code, read off its View screen, is VR-039.

## Confirmed by the manual (Section Four and Section Five)

These are the manual's instructions, not guesses. The Eagle Long Range Fighter Operations Manual, the Archive.org text of the 1993 manual:

- Upgrades: Fit or Remove, and the cash total on the left changes. The More... arrows page the list. Go Back returns to the Shipyard, and F4 returns to Services. Fitting and removal happen while you wait, and the process is fully automated.
- Repairs and servicing: Communications F4, then Shipyard, then Repairs and Servicing. That screen is a list of servicing options.
- New ships: Buy or View. View adds mountings, crew and hyperspace range. A new ship is stripped of extras; only the hyperdrive changes the trade-in. It comes with 1 t of fuel in the internal tank.
- Bulletin board: you choose one of the replies presented. The answer comes up on the console at the bottom. The acceptance button is "OK - agreed." Passengers then appear on the passenger list, and payment is normally on arrival, though some pay half or all up front. The same negotiation is used for packages.

The Sirocco View screen of the first Constrictor (read off the original, not the manual) is the layout now drawn: Mass (Fully laden) 120 t, Internal Capacity (no drive) 90 t, 2 gun mountings, 2 missile pylons, crew 2, main thrust 22.2 Earth g, retro 10.0 Earth g, Drive Fitted Class 2 Hyperdrive, registration VR-039. Hyperspace range is `200 × h² / 120`, truncated to two decimals: class 2 is 6.66, class 3 is 15.00, class 4 is 26.66. Classes 1 and 5 to 7 read 0.00 on that screen, so this hull mounts classes 2 to 4 only. Other hulls have not been opened on View, so a range is shown only for the drive they are sold with.

## Not in the manual, and not confirmed

The manual never quotes the console's reply after you fit, repair, buy, or talk. These lines are still unconfirmed, and their wording must not be treated as the original's:

- "Fitting X.", "Removing X.", "You cannot afford a new X.", and the other fit and remove refusals.
- "Service completed." and "You cannot afford that service."
- "Your new X is ready."
- "Thank you."
- "I don't believe you."

