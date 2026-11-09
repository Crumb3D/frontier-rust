"""Shipyard: buy ships, fit/remove equipment, capacity accounting."""

from __future__ import annotations

import json
import random
from dataclasses import dataclass, field
from pathlib import Path
from typing import Any

from sim.flight import ShipStats
from sim.galaxy import StarSystem

DATA = Path(__file__).resolve().parent.parent / "data"

# G -> approx kN thrust scale for our Newtonian model (mass * g * 9.81 / 1000 roughly)
_G_TO_KN = 9.81  # thrust_kn ≈ thrust_g * mass_t * 9.81 / roughly — use mass * g * factor


def _norm_allegiance(tag: str) -> str:
    """Accept both short codes (I/F/E) and full names from ships.json."""
    t = tag.strip().lower()
    if t in ("i", "independent"):
        return "I"
    if t in ("f", "federation"):
        return "F"
    if t in ("e", "empire"):
        return "E"
    return tag.upper()[:1]


@dataclass
class EquipDef:
    id: str
    name: str
    category: str
    mass_t: float
    price: int
    tech_min: int
    slot: str
    raw: dict[str, Any] = field(default_factory=dict)

    @classmethod
    def from_dict(cls, d: dict) -> "EquipDef":
        known = {"id", "name", "category", "mass_t", "price", "tech_min", "slot"}
        return cls(
            id=d["id"],
            name=d["name"],
            category=d["category"],
            mass_t=float(d["mass_t"]),
            price=int(d["price"]),
            tech_min=int(d["tech_min"]),
            slot=d["slot"],
            raw={k: v for k, v in d.items() if k not in known},
        )


@dataclass
class ShipDef:
    id: str
    name: str
    allegiance: list[str]
    mass_t: float
    capacity_t: float
    price: int
    crew: int
    missiles: int
    drive: str
    jump_ly_base: float
    thrust_g: float
    retro_g: float
    gun_mounts: list[str]
    scoop_capable: bool
    tech_min: int

    @classmethod
    def from_dict(cls, d: dict) -> "ShipDef":
        return cls(
            id=d["id"],
            name=d["name"],
            allegiance=list(d["allegiance"]),
            mass_t=float(d["mass_t"]),
            capacity_t=float(d["capacity_t"]),
            price=int(d["price"]),
            crew=int(d["crew"]),
            missiles=int(d["missiles"]),
            drive=d["drive"],
            jump_ly_base=float(d["jump_ly_base"]),
            thrust_g=float(d["thrust_g"]),
            retro_g=float(d["retro_g"]),
            gun_mounts=list(d["gun_mounts"]),
            scoop_capable=bool(d["scoop_capable"]),
            tech_min=int(d["tech_min"]),
        )


def load_ships() -> dict[str, ShipDef]:
    with open(DATA / "ships.json", encoding="utf-8") as f:
        return {s["id"]: ShipDef.from_dict(s) for s in json.load(f)}


def load_equipment() -> dict[str, EquipDef]:
    with open(DATA / "equipment.json", encoding="utf-8") as f:
        return {e["id"]: EquipDef.from_dict(e) for e in json.load(f)}


@dataclass
class FittedShip:
    """Player-owned hull with fitted equipment and cargo."""

    ship_id: str
    equipment: list[str] = field(default_factory=list)  # equip ids (stackable repeats)
    guns: dict[str, str] = field(default_factory=dict)  # mount -> equip id
    missile_load: list[str] = field(default_factory=list)  # missile equip ids on pylons
    cargo: dict[str, int] = field(default_factory=dict)
    bonus_capacity: float = 0.0  # extra tonnes from the rubbish-inventory quirk
    fuel_t: float = 1.0
    hull: float = 100.0
    max_hull: float = 100.0
    credits_paid: int = 0
    last_drive_service: float | None = None
    last_equip_service: float | None = None

    def hull_mass(self, ships: dict[str, ShipDef]) -> float:
        return ships[self.ship_id].mass_t

    def equip_mass(self, equip: dict[str, EquipDef]) -> float:
        # Count unique fitted items once: drives/utilities from equipment,
        # guns from mounts, missiles from pylons (equipment list may mirror them)
        seen_guns = set(self.guns.values())
        seen_missiles = list(self.missile_load)
        total = 0.0
        for eid in self.equipment:
            e = equip[eid]
            if e.slot == "gun" and eid in seen_guns:
                continue  # counted via guns
            if e.slot == "missile":
                continue  # counted via missile_load
            total += e.mass_t
        for eid in self.guns.values():
            total += equip[eid].mass_t
        for eid in seen_missiles:
            total += equip[eid].mass_t
        return total

    def cargo_mass(self) -> float:
        return float(sum(self.cargo.values()))

    def used_capacity(self, ships: dict[str, ShipDef], equip: dict[str, EquipDef]) -> float:
        # In FE2: capacity is hull free space; drive/guns/equip eat capacity; leftover = cargo room
        return self.equip_mass(equip) + self.cargo_mass() + self.fuel_t

    def free_cargo(self, ships: dict[str, ShipDef], equip: dict[str, EquipDef]) -> float:
        cap = ships[self.ship_id].capacity_t + self.bonus_capacity
        return max(0.0, cap - self.used_capacity(ships, equip))

    def has(self, equip_id: str) -> bool:
        return equip_id in self.equipment or equip_id in self.guns.values()

    def count(self, equip_id: str) -> int:
        return self.equipment.count(equip_id) + sum(1 for g in self.guns.values() if g == equip_id)


def default_eagle() -> FittedShip:
    """Starting ship: Eagle Mk I with 1MW pulse, class 1 drive, 1t fuel."""
    return FittedShip(
        ship_id="eagle_mk1",
        equipment=["drive_class1", "pulse_1mw", "missile_homing", "missile_homing", "atmo_shield", "autopilot", "scanner"],
        guns={"front": "pulse_1mw"},
        missile_load=["missile_homing", "missile_homing"],
        cargo={},
        fuel_t=1.0,
        hull=80.0,
        max_hull=80.0,
    )


def fitted_to_stats(fitted: FittedShip, ships: dict[str, ShipDef], equip: dict[str, EquipDef]) -> ShipStats:
    """Derive live flight/combat stats from hull + fittings."""
    hull = ships[fitted.ship_id]
    # Thrust from g-rating
    # FAQ IX-B: acceleration in g is a constant of the ship, not of its load.
    thrust_kn = hull.thrust_g * 9.81
    retro_kn = hull.retro_g * 9.81
    lateral_kn = thrust_kn * 0.2

    laser_power = 8.0
    fire_rate = 1.6
    if fitted.guns:
        front = fitted.guns.get("front") or next(iter(fitted.guns.values()))
        g = equip[front]
        laser_power = float(g.raw.get("power", 8))
        fire_rate = float(g.raw.get("fire_rate", 1.6))

    from sim.frontier import SHIELD_TONS
    shield = fitted.equipment.count("shield_gen") * SHIELD_TONS

    from sim.frontier import drive_h, max_range_ly

    max_fuel = 1.0
    drive_id = "interplanetary"
    for eid in fitted.equipment:
        e = equip[eid]
        if e.slot == "drive":
            drive_id = str(e.raw.get("drive_id", "interplanetary"))
            max_fuel = float(e.raw.get("internal_fuel_t", 1))
    # FAQ VII-C: d_max = 200 h^2 / m, m = hull + fittings + cargo + internal fuel
    total_mass = (
        hull.mass_t
        + fitted.equip_mass(equip)
        + fitted.cargo_mass()
        + min(fitted.fuel_t, max_fuel)
    )
    h = drive_h(drive_id)
    laden = hull.mass_t + hull.capacity_t
    formula = max_range_ly(total_mass, h)
    formula_laden = max_range_ly(laden, h)
    # Use the FAQ mass formula when it reproduces the published jump.
    # Panther's published 9.1 ly does not match 200*81/2000, so scale that figure.
    if h > 0 and formula_laden > 0 and abs(formula_laden - hull.jump_ly_base) > 0.2:
        jump = hull.jump_ly_base * laden / max(total_mass, 1.0)
    else:
        jump = formula

    has_dock = "docking_computer" in fitted.equipment or "autopilot" in fitted.equipment
    has_life = "life_support" in fitted.equipment
    has_scoop = "fuel_scoop" in fitted.equipment
    has_atmo = "atmo_shield" in fitted.equipment
    ecm = 0
    for eid in fitted.equipment:
        ecm = max(ecm, int(equip[eid].raw.get("ecm_level", 0)))

    cargo_cap = int(fitted.free_cargo(ships, equip) + fitted.cargo_mass())

    return ShipStats(
        name=hull.name,
        mass_t=total_mass,
        thrust_kn=thrust_kn,
        retro_kn=retro_kn,
        lateral_kn=lateral_kn,
        torque=max(0.6, 2.5 - hull.mass_t / 800.0),
        max_fuel_t=max_fuel,
        fuel_t=min(fitted.fuel_t, max_fuel),
        jump_range_ly=jump,
        hull=fitted.hull,
        max_hull=fitted.max_hull,
        shield=shield,
        max_shield=shield,
        energy=100.0,
        max_energy=100.0,
        laser_power=laser_power,
        has_missile=hull.missiles > 0,
        missiles=len(fitted.missile_load),
        has_docking_computer=has_dock,
        cargo_capacity=max(0, cargo_cap),
        cargo=dict(fitted.cargo),
        price=hull.price,
        # Extended flags stashed via dynamic attrs
    )


# Attach extended flags after — ShipStats is a dataclass; extend it
def apply_equipment_flags(stats: ShipStats, fitted: FittedShip, equip: dict[str, EquipDef]) -> ShipStats:
    stats._has_life_support = "life_support" in fitted.equipment  # type: ignore[attr-defined]
    stats._has_fuel_scoop = "fuel_scoop" in fitted.equipment  # type: ignore[attr-defined]
    stats._has_atmo_shield = "atmo_shield" in fitted.equipment  # type: ignore[attr-defined]
    stats._has_cargo_scoop = "cargo_scoop" in fitted.equipment  # type: ignore[attr-defined]
    stats._ecm_level = 0  # type: ignore[attr-defined]
    stats._laser_heat_reduction = 0.0  # type: ignore[attr-defined]
    stats._hull_repair = 0.0  # type: ignore[attr-defined]
    stats._energy_booster = False  # type: ignore[attr-defined]
    stats._drive_h = 0.0  # type: ignore[attr-defined]
    stats._fuel_goods = "hydrogen_fuel"  # type: ignore[attr-defined]
    stats._fuel_type = "hydrogen_fuel"  # type: ignore[attr-defined]
    stats._passenger_cabins = 0  # type: ignore[attr-defined]
    stats._fire_rate = 1.6  # type: ignore[attr-defined]
    stats._beam_laser = False  # type: ignore[attr-defined]
    stats._mining_laser = False  # type: ignore[attr-defined]
    for eid in fitted.equipment:
        e = equip[eid]
        stats._ecm_level = max(stats._ecm_level, int(e.raw.get("ecm_level", 0)))  # type: ignore
        stats._laser_heat_reduction += float(e.raw.get("heat_reduction", 0))  # type: ignore
        stats._hull_repair += float(e.raw.get("hull_repair_rate", 0))  # type: ignore
        if eid == "energy_booster":
            stats._energy_booster = True  # type: ignore
        if e.slot == "drive":
            from sim.frontier import drive_h
            stats._fuel_type = str(e.raw.get("fuel_type", "hydrogen_fuel"))  # type: ignore
            stats._fuel_goods = stats._fuel_type  # type: ignore
            stats._drive_h = drive_h(str(e.raw.get("drive_id", "")))  # type: ignore
        stats._passenger_cabins += int(e.raw.get("passengers", 0))  # type: ignore
    if fitted.guns:
        g = equip[next(iter(fitted.guns.values()))]
        stats._fire_rate = float(g.raw.get("fire_rate", 1.6))  # type: ignore
        stats._beam_laser = bool(g.raw.get("beam", False))  # type: ignore
        stats._mining_laser = bool(g.raw.get("mining", False))  # type: ignore
    return stats


class Shipyard:
    def __init__(self) -> None:
        self.ships = load_ships()
        self.equipment = load_equipment()

    def available_ships(self, system: StarSystem) -> list[ShipDef]:
        """Stock depends on tech level; Federation/Empire hulls rarer off-allegiance."""
        rng = random.Random(system.seed ^ 0x53484950)
        out = []
        for s in self.ships.values():
            if s.tech_min > system.tech_level:
                continue
            alleg = {_norm_allegiance(a) for a in s.allegiance}
            chance = 0.9
            if system.tech_level < s.tech_min + 2:
                chance = 0.5
            # Empire-only hulls scarce outside high-tech authoritarian systems
            if alleg == {"E"}:
                if system.government not in ("Dictatorship", "Feudal") and system.tech_level < 8:
                    chance *= 0.3
            # Federation-only hulls scarce in anarchies
            if alleg == {"F"} and system.government == "Anarchy":
                chance *= 0.35
            if rng.random() < chance:
                out.append(s)
        out.sort(key=lambda x: x.price)
        return out

    def available_equipment(self, system: StarSystem) -> list[EquipDef]:
        rng = random.Random(system.seed ^ 0x45515000 ^ system.tech_level)
        out = []
        for e in self.equipment.values():
            if e.tech_min > system.tech_level:
                continue
            # Military gear scarce
            if e.raw.get("military") or "Naval" in e.name or "Navy" in e.name:
                if system.tech_level < 8 or rng.random() < 0.4:
                    if e.tech_min > system.tech_level - 1:
                        continue
            out.append(e)
        out.sort(key=lambda x: (x.category, x.price))
        return out

    def fit_price(self, equip_id: str) -> int:
        return self.equipment[equip_id].price

    def service_prices(self, fitted: FittedShip) -> tuple[int, int, int]:
        """Deluxe, drive-only, equipment-only. 8% and 5% of the hull price.

        An Eagle Mk I at 38,000 Cr is 3,040 / 1,900 / 1,900.
        """
        price = self.ships[fitted.ship_id].price
        part = price * 5 // 100
        return price * 8 // 100, part, part

    def can_fit(self, fitted: FittedShip, equip_id: str) -> tuple[bool, str]:
        e = self.equipment[equip_id]
        hull = self.ships[fitted.ship_id]
        # Drive: only one
        if e.slot == "drive":
            if fitted.ship_id == "lifter":
                return False, "The Lifter's interplanetary drive cannot be changed"
            if fitted.ship_id in ("imperial_courier", "imperial_trader"):
                return False, "This ship's hyperdrive is fixed"
            if e.id == "drive_class8" and fitted.ship_id not in ("boa", "panther"):
                return False, "Class 8 hyperdrive is built into the Boa and Panther only"
            old = next((x for x in fitted.equipment if self.equipment[x].slot == "drive"), None)
            old_mass = self.equipment[old].mass_t if old else 0.0
            free = fitted.free_cargo(self.ships, self.equipment) + old_mass
            if e.mass_t > free + 0.01:
                return False, "Insufficient free capacity"
            return True, ""
        if e.slot == "gun":
            if not hull.gun_mounts:
                return False, "No gun mounts"
            return True, ""
        if e.slot == "missile":
            if len(fitted.missile_load) >= hull.missiles:
                return False, "No free missile pylons"
            return True, ""
        if e.slot == "special":
            free = fitted.free_cargo(self.ships, self.equipment)
            if e.mass_t > free + 0.01:
                return False, "Insufficient free capacity"
            return True, ""
        # Capacity check
        free = fitted.free_cargo(self.ships, self.equipment)
        if e.mass_t > free + 0.01:
            return False, "Insufficient free capacity"
        return True, ""

    def fit(self, fitted: FittedShip, equip_id: str, mount: str | None = None) -> tuple[bool, str]:
        ok, reason = self.can_fit(fitted, equip_id)
        if not ok:
            return False, reason
        e = self.equipment[equip_id]
        hull = self.ships[fitted.ship_id]
        if e.slot == "drive":
            # Replace existing drive
            fitted.equipment = [x for x in fitted.equipment if self.equipment[x].slot != "drive"]
            fitted.equipment.append(equip_id)
            fitted.fuel_t = min(fitted.fuel_t, float(e.raw.get("internal_fuel_t", 1)))
            return True, f"Fitted {e.name}"
        if e.slot == "gun":
            m = mount or (hull.gun_mounts[0] if hull.gun_mounts else None)
            if not m:
                return False, "No gun mount"
            # Free capacity of old gun
            if m in fitted.guns:
                old = fitted.guns[m]
                if old in fitted.equipment:
                    fitted.equipment.remove(old)
            fitted.guns[m] = equip_id
            if equip_id not in fitted.equipment:
                fitted.equipment.append(equip_id)
            return True, f"Fitted {e.name} on {m}"
        if e.slot == "missile":
            fitted.missile_load.append(equip_id)
            fitted.equipment.append(equip_id)
            return True, f"Loaded {e.name}"
        # Stackable utilities
        if not e.raw.get("stackable") and equip_id in fitted.equipment:
            return False, "Already fitted"
        fitted.equipment.append(equip_id)
        return True, f"Fitted {e.name}"

    def remove(self, fitted: FittedShip, equip_id: str) -> tuple[bool, str, int]:
        e = self.equipment[equip_id]
        refund = e.price // 2
        if e.slot == "gun":
            mounts = [m for m, g in fitted.guns.items() if g == equip_id]
            if not mounts and equip_id not in fitted.equipment:
                return False, "Not fitted", 0
            for m in mounts:
                del fitted.guns[m]
            while equip_id in fitted.equipment:
                fitted.equipment.remove(equip_id)
            return True, f"Removed {e.name}", refund
        if e.slot == "missile":
            if equip_id in fitted.missile_load:
                fitted.missile_load.remove(equip_id)
            if equip_id in fitted.equipment:
                fitted.equipment.remove(equip_id)
            return True, f"Removed {e.name}", refund
        if equip_id not in fitted.equipment:
            return False, "Not fitted", 0
        if e.slot == "drive":
            return False, "Cannot remove drive (fit a replacement)", 0
        fitted.equipment.remove(equip_id)
        return True, f"Removed {e.name}", refund

    def buy_ship(
        self,
        ship_id: str,
        trade_in: FittedShip | None,
        credits: int,
        *,
        free: bool = False,
    ) -> tuple[FittedShip | None, int, str]:
        hull = self.ships[ship_id]
        trade_value = 0
        if trade_in:
            trade_value = self.ships[trade_in.ship_id].price // 2
            # Add half of fitted gear
            for eid in set(trade_in.equipment):
                trade_value += self.equipment[eid].price // 4
        cost = 0 if free else hull.price - trade_value
        # Free purchases still pay the trade-in, so selling the old hull makes money
        payout = trade_value if free else 0
        if credits + payout < cost:
            return None, credits, "Insufficient credits"
        # Stock drive matching hull.drive
        drive_map = {
            "interplanetary": "drive_ip",
            "class_1": "drive_class1",
            "class_2": "drive_class2",
            "class_3": "drive_class3",
            "class_4": "drive_class4",
            "class_5": "drive_class5",
            "class_6": "drive_class6",
            "class_7": "drive_class7",
            "class_8": "drive_class8",
            "military_1": "drive_mil1",
            "military_2": "drive_mil2",
            "military_3": "drive_mil3",
        }
        drive_id = drive_map.get(hull.drive, "drive_class1")
        fitted = FittedShip(
            ship_id=ship_id,
            equipment=[drive_id],
            guns={},
            missile_load=[],
            cargo={},
            fuel_t=float(self.equipment[drive_id].raw.get("internal_fuel_t", 1)),
            hull=100.0 + hull.mass_t * 0.05,
            max_hull=100.0 + hull.mass_t * 0.05,
        )
        if hull.gun_mounts:
            fitted.guns[hull.gun_mounts[0]] = "pulse_1mw"
            fitted.equipment.append("pulse_1mw")
        note = f"Purchased {hull.name}"
        if free and trade_value:
            note += f"  (free, +{trade_value} Cr trade-in)"
        elif free:
            note += "  (free)"
        return fitted, credits + payout - cost, note
