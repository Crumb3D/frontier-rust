"""Delivery missions against the stardate clock."""

from __future__ import annotations

import random
from dataclasses import dataclass

from sim.galaxy import Galaxy, StarSystem
from sim.market import Market


@dataclass
class Mission:
    id: str
    description: str
    dest_system: str
    dest_location: str
    dest_name: str
    cargo_id: str
    cargo_name: str
    quantity: int
    reward: int
    deadline_day: float
    taken: bool = False
    completed: bool = False
    kind: str = "cargo"  # cargo | passenger
    speaker: str = ""
    pay_each: float = 0.0


class MissionBoard:
    def __init__(self, galaxy: Galaxy, market: Market) -> None:
        self.galaxy = galaxy
        self.market = market
        self.active: list[Mission] = []

    def generate(self, system: StarSystem, day: int, count: int = 4) -> list[Mission]:
        rng = random.Random(system.seed ^ (day * 9973))
        neighbors = self.galaxy.neighbors(system.id, 25.0)
        if not neighbors:
            return []
        goods = [g for g in self.market.goods if g.legal]
        missions: list[Mission] = []
        for i in range(count):
            dest_id, dist = neighbors[rng.randrange(len(neighbors))]
            dest = self.galaxy.get(dest_id)
            # Prefer a station or inhabited planet starport
            loc_id = dest.id
            loc_name = dest.name
            if dest.stations:
                st = dest.stations[0]
                loc_id = st.id
                loc_name = f"{st.name}, {dest.name}"
            else:
                for p in dest.planets:
                    if p.starports:
                        sp = p.starports[0]
                        loc_id = sp.id
                        loc_name = f"{sp.name}, {p.name}"
                        break
                else:
                    for p in dest.planets:
                        if p.inhabited:
                            loc_id = p.id
                            loc_name = f"{p.name}, {dest.name}"
                            break
            g = goods[rng.randrange(len(goods))]
            prices = self.market.price_map(system, day)
            unit = round(float(prices.get(g.id, g.base_price)) * 2.0, 1)
            reward = int(50 + dist * 80 + g.base_price)
            deadline = day + 10 + dist * 2
            kind = "cargo"
            if i == 0 and system.government not in ("Anarchy",):
                kind = "passenger"
            names = (
                "Sarah Davies", "John Smith", "Anna Kirov", "Paul Evans",
                "Maria Chen", "David Okonkwo", "Helen Wright", "Carlos Vega",
            )
            speaker = names[rng.randrange(len(names))]
            missions.append(
                Mission(
                    id=f"m_{system.id}_{day}_{i}",
                    description=(
                        f"WANTED: passage for a small group to the {dest.name} system. Will pay ${reward}."
                        if kind == "passenger"
                        else f"WANTED: {g.name}. Will pay ${unit:.1f}."
                    ),
                    dest_system=dest_id,
                    dest_location=loc_id,
                    dest_name=loc_name,
                    cargo_id="passenger" if kind == "passenger" else g.id,
                    cargo_name="Passenger" if kind == "passenger" else g.name,
                    quantity=1 if kind == "passenger" else 1,
                    reward=reward,
                    deadline_day=deadline,
                    kind=kind,
                    speaker=speaker,
                    pay_each=0.0 if kind == "passenger" else unit,
                )
            )
        return missions

    def accept(self, mission: Mission) -> None:
        mission.taken = True
        self.active.append(mission)

    def check_delivery(
        self,
        system_id: str,
        location_id: str,
        cargo: dict[str, int],
        day: float,
    ) -> list[Mission]:
        done: list[Mission] = []
        for m in list(self.active):
            if m.completed:
                continue
            if day > m.deadline_day:
                self.active.remove(m)
                continue
            if m.dest_system == system_id and (
                m.dest_location == location_id or location_id.startswith(m.dest_location)
            ):
                have = cargo.get(m.cargo_id, 0)
                if have >= m.quantity:
                    cargo[m.cargo_id] = have - m.quantity
                    if cargo[m.cargo_id] <= 0:
                        del cargo[m.cargo_id]
                    m.completed = True
                    done.append(m)
                    self.active.remove(m)
        return done
