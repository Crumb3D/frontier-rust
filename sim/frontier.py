"""Documented Frontier: Elite II formulas (FAQ VII-C, XI-B, XI-F).

Hyperdrive (FAQ VII-C), with class 8 treated as class 9:

    c = ceil(d * m / 200)          fuel, tonnes
    t = 168/200 * d * m / h^2      hours
    d_max = 200 * h^2 / m          light years, at t = 168 h

Shields: 16 tonnes each. A pulse laser does 12.5 tonnes per MW.
Combat rank: 256 points per kill. ELITE at 6000 kills.
"""

from __future__ import annotations

import math
from collections.abc import Callable

# Civil class -> hyperdrive rating h. Class 8 is a class 9 drive (81 t at full range).
_CIVIL_H = {1: 1, 2: 2, 3: 3, 4: 4, 5: 5, 6: 6, 7: 7, 8: 9}
_MIL_H = {1: 1, 2: 2, 3: 3}

# FAQ XI-C weights. Prices are left as already shipped; only documented masses.
FAQ_MASS_T = {
    "mine_dummy": 1,
    "mine_proximity": 1,
    "energy_bomb": 4,
    "missile_homing": 1,
    "missile_smart": 1,
    "missile_navy": 1,
    "mining_laser": 10,
    "pulse_1mw": 1,
    "pulse_5mw": 3,
    "beam_1mw": 5,
    "beam_4mw": 20,
    "beam_20mw": 75,
    "beam_100mw": 200,
    "laser_cooler": 1,
    "plasma_small": 500,
    "plasma_large": 900,
    "ecm": 2,
    "ecm_naval": 2,
    "life_support": 1,
    "passenger_cabin": 5,
    "autopilot": 1,
    "auto_refueller": 1,
    "radar_mapper": 1,
    "cloud_analyser": 1,
    "shield_gen": 4,
    "energy_booster": 5,
    "escape_capsule": 5,
    "atmo_shield": 1,
    "hull_repair": 40,
    "fuel_scoop": 6,
    "cargo_scoop": 2,
    "mining_machine": 30,
    "drive_ip": 4,
    "drive_class1": 10,
    "drive_class2": 25,
    "drive_class3": 45,
    "drive_class4": 80,
    "drive_class5": 150,
    "drive_class6": 250,
    "drive_class7": 400,
    "drive_class8": 600,
    "drive_mil1": 6,
    "drive_mil2": 12,
    "drive_mil3": 24,
}

COMBAT_RANKS = (
    (0, "Harmless"),
    (4, "Mostly Harmless"),
    (8, "Poor"),
    (16, "Below Average"),
    (32, "Average"),
    (64, "Above Average"),
    (128, "Competent"),
    (1000, "Dangerous"),
    (3000, "Deadly"),
    (6000, "ELITE"),
)

SHIELD_TONS = 16.0
PULSE_TONS_PER_MW = 12.5
# Worked example recharges 5% between pulse hits. The only per-second figure
# is the energy booster at about 3%. Base is "a few percent", so 2%.
SHIELD_RECHARGE = 0.02
BOOSTER_RECHARGE = 0.03


def drive_h(drive_id: str | None) -> float:
    """Hyperdrive rating h. Zero for an interplanetary drive."""
    if not drive_id or drive_id in ("interplanetary", "drive_ip"):
        return 0.0
    if drive_id.startswith("military_") or drive_id.startswith("drive_mil"):
        n = int("".join(ch for ch in drive_id if ch.isdigit()) or "0")
        return float(_MIL_H.get(n, n))
    if drive_id.startswith("class_") or drive_id.startswith("drive_class"):
        n = int("".join(ch for ch in drive_id if ch.isdigit()) or "0")
        return float(_CIVIL_H.get(n, n))
    return 0.0


def max_range_ly(mass_t: float, h: float) -> float:
    """d_max = 200 * h^2 / m."""
    if h <= 0 or mass_t <= 0:
        return 0.0
    return 200.0 * (h * h) / mass_t


def jump_fuel_t(distance_ly: float, mass_t: float) -> int:
    """c = ceil(d * m / 200). At least 1 t for any real jump."""
    if distance_ly <= 0 or mass_t <= 0:
        return 0
    return max(1, math.ceil(distance_ly * mass_t / 200.0 - 1e-9))


def jump_hours(distance_ly: float, mass_t: float, h: float) -> float:
    """t = 168/200 * d * m / h^2 hours."""
    if h <= 0 or mass_t <= 0:
        return 0.0
    return (168.0 / 200.0) * distance_ly * mass_t / (h * h)


def combat_rank(kills: int) -> str:
    name = "Harmless"
    for need, label in COMBAT_RANKS:
        if kills >= need:
            name = label
    return name


def apply_shield_hit(shield_tons: float, max_tons: float, damage_tons: float) -> tuple[float, float]:
    """Subtract tonnes from the shield. Overflow is hull damage in tonnes."""
    if max_tons <= 0 or shield_tons <= 0:
        return 0.0, damage_tons
    absorbed = min(shield_tons, damage_tons)
    return shield_tons - absorbed, damage_tons - absorbed


def laser_damage_tons(megawatts: float, beam: bool = False, mining: bool = False) -> float:
    """Pulse: 12.5 t per MW. Beam packets are sized so a beam's higher rate is ~5x a pulse of the same MW.
    Mining laser: one hit in the FAQ's 750-1500 t band (midpoint, not invented outside that band).
    """
    if mining:
        return 1000.0
    per_shot = PULSE_TONS_PER_MW * megawatts
    if beam:
        return per_shot * 0.8
    return per_shot


def plan_route(
    start: str,
    goal: str,
    range_of,
    neighbors: Callable[[str, float], list[tuple[str, float]]],
    max_hops: int = 8,
) -> list[str] | None:
    """Shortest hop-count path where each jump is inside range_of(system)."""
    if start == goal:
        return [start]
    reach = max(range_of(start), 0.1)
    frontier: list[tuple[str, list[str]]] = [(start, [start])]
    seen = {start}
    while frontier:
        cur, path = frontier.pop(0)
        if len(path) > max_hops:
            continue
        hop = range_of(cur)
        for nid, dist in neighbors(cur, max(hop, 0.1)):
            if nid in seen or dist > hop + 1e-6:
                continue
            nxt = path + [nid]
            if nid == goal:
                return nxt
            seen.add(nid)
            frontier.append((nid, nxt))
        reach = max(reach, hop)
    return None
