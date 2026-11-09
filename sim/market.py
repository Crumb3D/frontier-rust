"""Commodity markets — FE2 trade goods, economy bias, legality."""

from __future__ import annotations

import json
import random
from dataclasses import dataclass
from pathlib import Path

from sim.galaxy import StarSystem

DATA = Path(__file__).resolve().parent.parent / "data"


@dataclass
class Good:
    id: str
    name: str
    base_price: int
    legal: bool
    unit: str = "t"
    needs_life_support: bool = False
    disposal: bool = False
    restricted: bool = False
    illegal_in: list[str] = None  # type: ignore[assignment]
    is_drive_fuel: bool = False

    def __post_init__(self) -> None:
        if self.illegal_in is None:
            self.illegal_in = []


@dataclass
class MarketListing:
    good: Good
    price: int
    stock: int


def load_goods() -> list[Good]:
    with open(DATA / "goods.json", encoding="utf-8") as f:
        raw = json.load(f)
    out = []
    for g in raw:
        out.append(
            Good(
                id=g["id"],
                name=g["name"],
                base_price=int(g["base_price"]),
                legal=bool(g["legal"]),
                unit=g.get("unit", "t"),
                needs_life_support=bool(g.get("needs_life_support", False)),
                disposal=bool(g.get("disposal", False)),
                restricted=bool(g.get("restricted", False)),
                illegal_in=list(g.get("illegal_in", [])),
                is_drive_fuel=bool(g.get("is_drive_fuel", False)),
            )
        )
    return out


def system_faction(system: StarSystem) -> str:
    """Map government to FE2-style faction for illegal_in checks."""
    if system.government in ("Democracy", "Corporate", "Communist"):
        return "Federation"
    if system.government in ("Dictatorship", "Feudal"):
        return "Empire"
    return "Independent"


# Economy: negative = cheap export, positive = expensive import demand
_ECONOMY_BIAS: dict[str, dict[str, float]] = {
    "Agricultural": {
        "grain": -0.4,
        "fruit_veg": -0.35,
        "animal_meat": -0.3,
        "fertilizer": -0.2,
        "farm_machinery": 0.35,
        "computers": 0.3,
        "industrial_parts": 0.25,
        "medicines": 0.15,
        "luxury_goods": 0.2,
    },
    "Industrial": {
        "industrial_parts": -0.35,
        "heavy_plastics": -0.3,
        "metal_alloys": -0.25,
        "computers": -0.15,
        "farm_machinery": -0.1,
        "minerals": 0.3,
        "grain": 0.2,
        "water": 0.15,
        "luxury_goods": 0.1,
    },
    "High-Tech": {
        "computers": -0.4,
        "robots": -0.3,
        "medicines": -0.2,
        "industrial_parts": -0.15,
        "luxury_goods": -0.1,
        "minerals": 0.25,
        "precious_metals": 0.15,
        "grain": 0.2,
        "military_fuel": -0.1,
    },
    "Extraction": {
        "minerals": -0.45,
        "precious_metals": -0.3,
        "gem_stones": -0.25,
        "metal_alloys": -0.2,
        "radioactives": -0.15,
        "farm_machinery": 0.2,
        "air_processors": 0.25,
        "liquid_oxygen": 0.2,
        "water": 0.15,
        "grain": 0.3,
    },
    "Tourist": {
        "luxury_goods": -0.25,
        "liquor": -0.2,
        "animal_skins": -0.15,
        "medicines": 0.1,
        "computers": 0.15,
        "narcotics": 0.1,
    },
    "Colony": {
        "grain": -0.15,
        "water": -0.2,
        "liquid_oxygen": -0.15,
        "synthetic_meat": -0.1,
        "farm_machinery": 0.3,
        "industrial_parts": 0.25,
        "medicines": 0.2,
        "air_processors": 0.15,
        "computers": 0.2,
    },
}

# Governments that ban illegal / restricted goods more strictly
_STRICT = {"Democracy", "Corporate", "Communist"}
_PERMISSIVE = {"Anarchy", "Feudal", "Dictatorship"}


class Market:
    def __init__(self) -> None:
        self.goods = load_goods()
        self.goods_by_id = {g.id: g for g in self.goods}

    def listings(self, system: StarSystem, day: int, location_id: str = "") -> list[MarketListing]:
        seed = (system.seed ^ (day * 2654435761) ^ (hash(location_id) & 0xFFFFFFFF)) & 0xFFFFFFFF
        rng = random.Random(seed)
        bias = _ECONOMY_BIAS.get(system.economy, {})
        out: list[MarketListing] = []
        for g in self.goods:
            faction = system_faction(system)
            # Hard ban from market when listed in illegal_in for this faction
            if faction in g.illegal_in:
                continue
            if not g.legal:
                if system.government in _STRICT and rng.random() < 0.75:
                    continue
                if system.government not in _PERMISSIVE and rng.random() < 0.45:
                    continue
            if g.restricted and system.government in _STRICT and rng.random() < 0.35:
                continue
            if g.id == "military_fuel" and system.tech_level < 6:
                continue
            if g.id == "robots" and system.government == "Feudal" and rng.random() < 0.7:
                continue

            mod = bias.get(g.id, 0.0)
            daily = 1.0 + (rng.random() - 0.5) * 0.35
            tech = 1.0 + (system.tech_level - 5) * 0.02
            raw = g.base_price * (1.0 + mod) * daily * tech
            if g.disposal:
                # Negative base: you pay to dispose; stock is "capacity to take"
                price = min(-1, int(raw))
                stock = int(rng.randint(5, 40 + system.tech_level * 3))
            else:
                price = max(1, int(raw))
                stock = int(rng.randint(0, 35 + system.tech_level * 4))
                if mod < -0.2:
                    stock += rng.randint(10, 40)  # export surplus

            if g.is_drive_fuel or g.id == "hydrogen_fuel":
                stock = max(stock, 25)
                price = max(8, int(abs(g.base_price) * daily))
            if g.id == "water" and system.economy == "Colony":
                stock = max(stock, 15)

            out.append(MarketListing(good=g, price=price, stock=stock))
        return out

    def trade_board(self, system: StarSystem) -> dict[str, list[str]]:
        """Import and export columns from this economy's price bias."""
        economy = "Extraction" if system.economy == "Mining" else system.economy
        bias = _ECONOMY_BIAS.get(economy, {})
        major_imp: list[str] = []
        minor_imp: list[str] = []
        major_exp: list[str] = []
        minor_exp: list[str] = []
        for gid, val in sorted(bias.items(), key=lambda kv: -abs(kv[1])):
            good = self.goods_by_id.get(gid)
            name = good.name if good else gid
            if val >= 0.25:
                major_imp.append(name)
            elif val > 0:
                minor_imp.append(name)
            elif val <= -0.25:
                major_exp.append(name)
            elif val < 0:
                minor_exp.append(name)
        illegal = [g.name for g in self.goods if not g.legal]
        return {
            "Major Imports": major_imp,
            "Minor Imports": minor_imp,
            "Major Exports": major_exp,
            "Minor Exports": minor_exp,
            "Illegal Goods": illegal,
        }

    def price_map(self, system: StarSystem, day: int, location_id: str = "") -> dict[str, int]:
        return {L.good.id: L.price for L in self.listings(system, day, location_id)}

    def can_carry(self, good_id: str, has_life_support: bool) -> tuple[bool, str]:
        g = self.goods_by_id.get(good_id)
        if not g:
            return False, "Unknown good"
        if g.needs_life_support and not has_life_support:
            return False, "Requires cargo bay life support"
        return True, ""
