"""Galaxy catalog, seeded systems, hand-tuned Ross 154 and Sol."""

from __future__ import annotations

import json
import math
import random
from dataclasses import dataclass, field
from pathlib import Path
from typing import Callable

import numpy as np

from sim.frame import BodyRef, FrameSystem
from sim.kepler import OrbitalElements, position_at, AU_KM

DATA = Path(__file__).resolve().parent.parent / "data"

ECONOMIES = ("Agricultural", "Industrial", "High-Tech", "Extraction", "Tourist", "Colony")
GOVERNMENTS = ("Democracy", "Corporate", "Dictatorship", "Anarchy", "Feudal", "Communist")

PLANET_NAMES = (
    "Merlin", "Avalon", "Tintagel", "Camelot", "Lyonesse", "Galahad",
    "Earth", "Mars", "Venus", "Mercury", "Jupiter", "Saturn",
    "New Caledonia", "Port Royal", "Stark", "Iota", "Helios", "Nova",
    "Eden", "Athena", "Orion", "Vega", "Rhea", "Titan",
)


@dataclass
class Starport:
    """Surface landing site with multiple pads (FE2 city/starport)."""

    id: str
    name: str
    planet_id: str
    lat_deg: float = 15.0
    lon_deg: float = 40.0
    pads: list[str] = field(
        default_factory=lambda: ["Pad A", "Pad B", "Pad C", "Pad D"]
    )


@dataclass
class Planet:
    id: str
    name: str
    elements: OrbitalElements
    radius_km: float
    has_atmosphere: bool = False
    atmosphere_km: float = 0.0
    has_starport: bool = False
    color: tuple[int, int, int] = (80, 120, 180)
    inhabited: bool = False
    parent_id: str | None = None  # None = orbits the star; else moon of that body
    is_gas_giant: bool = False
    has_rings: bool = False
    rotation_days: float = 1.0
    # Moon circular orbit (used when parent_id set; elements ignored for position)
    moon_orbit_km: float = 0.0
    moon_period_days: float = 1.0
    moon_phase0: float = 0.0
    starports: list[Starport] = field(default_factory=list)


@dataclass
class Station:
    id: str
    name: str
    parent_planet_id: str
    orbit_radius_km: float
    orbit_period_days: float
    radius_km: float = 0.8
    phase0: float = 0.0


@dataclass
class StarSystem:
    id: str
    name: str
    position_ly: np.ndarray
    spectral: str
    mass: float
    seed: int
    economy: str
    government: str
    tech_level: int
    allegiance: str = "Independent"
    planets: list[Planet] = field(default_factory=list)
    stations: list[Station] = field(default_factory=list)

    def planet_by_id(self, pid: str) -> Planet | None:
        for p in self.planets:
            if p.id == pid:
                return p
        return None

    def starport_by_id(self, sid: str) -> Starport | None:
        for p in self.planets:
            for sp in p.starports:
                if sp.id == sid:
                    return sp
        return None

    def star_color(self) -> tuple[int, int, int]:
        s = self.spectral[0].upper() if self.spectral else "G"
        return {
            "O": (155, 176, 255),
            "B": (170, 191, 255),
            "A": (202, 215, 255),
            "F": (248, 247, 255),
            "G": (255, 244, 234),
            "K": (255, 210, 161),
            "M": (255, 160, 100),
        }.get(s, (255, 244, 234))


def _hash_seed(name: str) -> int:
    h = 2166136261
    for ch in name:
        h ^= ord(ch)
        h = (h * 16777619) & 0xFFFFFFFF
    return h


def surface_offset(lat_deg: float, lon_deg: float, radius_km: float) -> np.ndarray:
    """Lat/lon to cartesian offset from planet center (km)."""
    lat = math.radians(lat_deg)
    lon = math.radians(lon_deg)
    r = radius_km + 0.15  # sit slightly above surface
    x = r * math.cos(lat) * math.cos(lon)
    y = r * math.sin(lat)
    z = r * math.cos(lat) * math.sin(lon)
    return np.array([x, y, z], dtype=np.float64)


def _hand_ross154() -> StarSystem:
    """FE2 start: Merlin moon of Aster (gas giant) around Ross 154; Sirocco Starport."""
    aster = Planet(
        id="ross154_aster",
        name="Aster",
        elements=OrbitalElements(
            semi_major_au=0.18,
            eccentricity=0.03,
            inclination_deg=1.5,
            mean_anomaly0_deg=55.0,
            parent_mass_sol=0.17,
        ),
        radius_km=52000.0,
        has_atmosphere=True,
        atmosphere_km=800.0,
        color=(210, 170, 110),
        inhabited=False,
        is_gas_giant=True,
    )
    # Merlin: habitable moon of Aster (manual intro location)
    merlin = Planet(
        id="ross154_merlin",
        name="Merlin",
        elements=OrbitalElements(semi_major_au=0.001, parent_mass_sol=0.001),  # unused for moon
        radius_km=2800.0,
        has_atmosphere=True,
        atmosphere_km=55.0,
        has_starport=True,
        color=(70, 130, 95),
        inhabited=True,
        parent_id="ross154_aster",
        moon_orbit_km=185000.0,
        moon_period_days=2.8,
        moon_phase0=0.9,
        starports=[
            Starport(
                id="ross154_sirocco",
                name="Sirocco Starport",
                planet_id="ross154_merlin",
                lat_deg=18.0,
                lon_deg=42.0,
                pads=["Pad 1", "Pad 2", "Pad 3", "Pad 4", "Pad 5", "Hangar North"],
            )
        ],
    )
    avalon = Planet(
        id="ross154_avalon",
        name="Avalon",
        elements=OrbitalElements(
            semi_major_au=0.55,
            eccentricity=0.05,
            inclination_deg=5.0,
            mean_anomaly0_deg=120.0,
            parent_mass_sol=0.17,
        ),
        radius_km=6000.0,
        has_atmosphere=False,
        color=(160, 120, 80),
        inhabited=False,
    )
    station = Station(
        id="ross154_station",
        name="Merlin Orbital",
        parent_planet_id="ross154_merlin",
        orbit_radius_km=6500.0,
        orbit_period_days=0.35,
        phase0=0.3,
    )
    return StarSystem(
        id="ross154",
        name="Ross 154",
        position_ly=np.array([2.94, -7.21, -5.65], dtype=np.float64),
        spectral="M3.5V",
        mass=0.17,
        seed=_hash_seed("Ross 154"),
        economy="Colony",
        government="Democracy",
        tech_level=5,
        planets=[aster, merlin, avalon],
        stations=[station],
    )


def _hand_sol() -> StarSystem:
    earth = Planet(
        id="sol_earth",
        name="Earth",
        elements=OrbitalElements(
            semi_major_au=1.0,
            eccentricity=0.0167,
            inclination_deg=0.0,
            mean_anomaly0_deg=0.0,
            parent_mass_sol=1.0,
        ),
        radius_km=6371.0,
        has_atmosphere=True,
        atmosphere_km=100.0,
        has_starport=True,
        color=(40, 90, 180),
        inhabited=True,
        starports=[
            Starport(
                id="sol_london",
                name="London Starport",
                planet_id="sol_earth",
                lat_deg=51.5,
                lon_deg=-0.1,
                pads=["Pad 1", "Pad 2", "Pad 3", "Pad 4", "Pad 5", "Pad 6", "Military Apron"],
            )
        ],
    )
    mars = Planet(
        id="sol_mars",
        name="Mars",
        elements=OrbitalElements(
            semi_major_au=1.52,
            eccentricity=0.09,
            inclination_deg=1.8,
            mean_anomaly0_deg=80.0,
            parent_mass_sol=1.0,
        ),
        radius_km=3390.0,
        has_atmosphere=True,
        atmosphere_km=30.0,
        has_starport=True,
        color=(180, 90, 50),
        inhabited=True,
        starports=[
            Starport(
                id="sol_olympus",
                name="Olympus Starport",
                planet_id="sol_mars",
                lat_deg=18.0,
                lon_deg=-133.0,
                pads=["Pad A", "Pad B", "Pad C", "Pad D"],
            )
        ],
    )
    jupiter = Planet(
        id="sol_jupiter",
        name="Jupiter",
        elements=OrbitalElements(
            semi_major_au=5.2,
            eccentricity=0.05,
            inclination_deg=1.3,
            mean_anomaly0_deg=200.0,
            parent_mass_sol=1.0,
        ),
        radius_km=69911.0,
        color=(200, 160, 100),
        inhabited=False,
        is_gas_giant=True,
        has_atmosphere=True,
        atmosphere_km=2000.0,
    )
    station = Station(
        id="sol_earth_orbital",
        name="Gateway",
        parent_planet_id="sol_earth",
        orbit_radius_km=12000.0,
        orbit_period_days=0.5,
        phase0=1.0,
    )
    return StarSystem(
        id="sol",
        name="Sol",
        position_ly=np.zeros(3, dtype=np.float64),
        spectral="G2V",
        mass=1.0,
        seed=_hash_seed("Sol"),
        economy="High-Tech",
        government="Democracy",
        tech_level=10,
        planets=[earth, mars, jupiter],
        stations=[station],
    )


def _seeded_system(star: dict) -> StarSystem:
    seed = _hash_seed(star["name"])
    rng = random.Random(seed)
    n_planets = rng.randint(1, 5)
    planets: list[Planet] = []
    stations: list[Station] = []
    for i in range(n_planets):
        a = 0.08 + i * 0.25 + rng.random() * 0.1
        name = PLANET_NAMES[(seed + i * 17) % len(PLANET_NAMES)]
        pid = f"{star['id']}_p{i}"
        inhabited = rng.random() < 0.45
        has_atm = inhabited or rng.random() < 0.3
        gas = (not inhabited) and rng.random() < 0.25 and a > 0.3
        p = Planet(
            id=pid,
            name=name,
            elements=OrbitalElements(
                semi_major_au=a,
                eccentricity=rng.random() * 0.08,
                inclination_deg=rng.random() * 8.0,
                mean_anomaly0_deg=rng.random() * 360.0,
                long_asc_node_deg=rng.random() * 360.0,
                parent_mass_sol=star["mass"],
            ),
            radius_km=(25000 + rng.random() * 40000) if gas else (2000 + rng.random() * 8000),
            has_atmosphere=has_atm or gas,
            atmosphere_km=(200 + rng.random() * 600) if gas else (50 + rng.random() * 80 if has_atm else 0),
            has_starport=inhabited,
            color=(
                (200, 160, 100) if gas else (
                    int(60 + rng.random() * 160),
                    int(60 + rng.random() * 160),
                    int(60 + rng.random() * 160),
                )
            ),
            inhabited=inhabited,
            is_gas_giant=gas,
            rotation_days=0.4 + rng.random() * 1.6,
        )
        if inhabited:
            p.starports.append(
                Starport(
                    id=f"{pid}_port",
                    name=f"{name} Starport",
                    planet_id=pid,
                    lat_deg=rng.uniform(-40, 40),
                    lon_deg=rng.uniform(-180, 180),
                    pads=[f"Pad {n}" for n in range(1, 1 + rng.randint(3, 6))],
                )
            )
        planets.append(p)
        if inhabited and rng.random() < 0.7:
            stations.append(
                Station(
                    id=f"{pid}_stn",
                    name=f"{name} Orbital",
                    parent_planet_id=pid,
                    orbit_radius_km=p.radius_km * 1.8 + 2000,
                    orbit_period_days=0.3 + rng.random() * 0.5,
                    phase0=rng.random() * math.pi * 2,
                )
            )
    return StarSystem(
        id=star["id"],
        name=star["name"],
        position_ly=np.array([star["x"], star["y"], star["z"]], dtype=np.float64),
        spectral=star["spectral"],
        mass=star["mass"],
        seed=seed,
        economy=ECONOMIES[seed % len(ECONOMIES)],
        government=GOVERNMENTS[(seed // 7) % len(GOVERNMENTS)],
        tech_level=1 + (seed % 10),
        planets=planets,
        stations=stations,
    )


class Galaxy:
    def __init__(self) -> None:
        with open(DATA / "stars.json", encoding="utf-8") as f:
            stars = json.load(f)
        self.systems: dict[str, StarSystem] = {}
        hand = {"ross154": _hand_ross154(), "sol": _hand_sol()}
        for s in stars:
            if s["id"] in hand:
                sys = hand[s["id"]]
                sys.position_ly = np.array([s["x"], s["y"], s["z"]], dtype=np.float64)
                if "economy" in s:
                    sys.economy = s["economy"]
                if "government" in s:
                    sys.government = s["government"]
                if "tech_level" in s:
                    sys.tech_level = int(s["tech_level"])
                if "allegiance" in s:
                    sys.allegiance = s["allegiance"]
                self.systems[s["id"]] = sys
            else:
                sys = _seeded_system(s)
                if "economy" in s:
                    sys.economy = s["economy"]
                if "government" in s:
                    sys.government = s["government"]
                if "tech_level" in s:
                    sys.tech_level = int(s["tech_level"])
                if "allegiance" in s:
                    sys.allegiance = s["allegiance"]
                self.systems[s["id"]] = sys

    def get(self, system_id: str) -> StarSystem:
        return self.systems[system_id]

    def distance_ly(self, a: str, b: str) -> float:
        pa = self.systems[a].position_ly
        pb = self.systems[b].position_ly
        return float(np.linalg.norm(pa - pb))

    def neighbors(self, system_id: str, max_ly: float) -> list[tuple[str, float]]:
        out = []
        for sid, sys in self.systems.items():
            if sid == system_id:
                continue
            d = self.distance_ly(system_id, sid)
            if d <= max_ly:
                out.append((sid, d))
        out.sort(key=lambda x: x[1])
        return out


def build_frame_system(system: StarSystem) -> FrameSystem:
    """Build hierarchical bodies: star → planets → moons → starports / stations."""
    frames = FrameSystem()

    def star_pos(_days: float) -> np.ndarray:
        return np.zeros(3, dtype=np.float64)

    frames.add(
        BodyRef(
            id="star",
            name=system.name,
            radius_km=max(100000.0 * system.mass, 50000.0),
            world_pos=star_pos,
        )
    )

    planet_pos: dict[str, Callable[[float], np.ndarray]] = {}

    # Primary planets (orbit star)
    for p in system.planets:
        if p.parent_id is not None:
            continue
        elems = p.elements

        def make_pos(el: OrbitalElements) -> Callable[[float], np.ndarray]:
            return lambda days, e=el: position_at(e, days)

        pos_fn = make_pos(elems)
        planet_pos[p.id] = pos_fn
        frames.add(
            BodyRef(
                id=p.id,
                name=p.name,
                radius_km=p.radius_km,
                world_pos=pos_fn,
                parent_id="star",
                is_starport=False,
                atmosphere_km=p.atmosphere_km,
                spin_period_days=p.rotation_days,
            )
        )

    # Moons (orbit a parent planet)
    for p in system.planets:
        if p.parent_id is None:
            continue
        parent_fn = planet_pos[p.parent_id]

        def make_moon(
            pfn: Callable[[float], np.ndarray],
            radius: float,
            period: float,
            phase: float,
        ) -> Callable[[float], np.ndarray]:
            def moon_pos(days: float) -> np.ndarray:
                ang = phase + 2.0 * math.pi * days / max(period, 1e-6)
                offset = np.array(
                    [math.cos(ang) * radius, 0.0, math.sin(ang) * radius],
                    dtype=np.float64,
                )
                return pfn(days) + offset

            return moon_pos

        pos_fn = make_moon(parent_fn, p.moon_orbit_km, p.moon_period_days, p.moon_phase0)
        planet_pos[p.id] = pos_fn
        frames.add(
            BodyRef(
                id=p.id,
                name=p.name,
                radius_km=p.radius_km,
                world_pos=pos_fn,
                parent_id=p.parent_id,
                is_starport=False,
                atmosphere_km=p.atmosphere_km,
                spin_period_days=p.rotation_days,
            )
        )

    # Surface starports (fixed on planet surface)
    for p in system.planets:
        pfn = planet_pos[p.id]
        for sp in p.starports:
            off = surface_offset(sp.lat_deg, sp.lon_deg, p.radius_km)
            spin = p.rotation_days

            def make_port(
                parent_fn: Callable[[float], np.ndarray],
                offset: np.ndarray,
                period: float,
            ) -> Callable[[float], np.ndarray]:
                def port_pos(days: float, pfn=parent_fn, o=offset, period=period) -> np.ndarray:
                    from render.structures import spin_about_y

                    return pfn(days) + spin_about_y(days, period) @ o

                return port_pos

            frames.add(
                BodyRef(
                    id=sp.id,
                    name=sp.name,
                    radius_km=2.0,
                    world_pos=make_port(pfn, off, spin),
                    parent_id=p.id,
                    is_starport=True,
                    atmosphere_km=0.0,
                )
            )

    for st in system.stations:
        parent_fn = planet_pos[st.parent_planet_id]

        def make_stn(
            pfn: Callable[[float], np.ndarray],
            radius: float,
            period: float,
            phase: float,
        ) -> Callable[[float], np.ndarray]:
            def stn_pos(days: float) -> np.ndarray:
                ang = phase + 2.0 * math.pi * days / period
                offset = np.array(
                    [math.cos(ang) * radius, 0.0, math.sin(ang) * radius],
                    dtype=np.float64,
                )
                return pfn(days) + offset

            return stn_pos

        frames.add(
            BodyRef(
                id=st.id,
                name=st.name,
                radius_km=st.radius_km,
                world_pos=make_stn(parent_fn, st.orbit_radius_km, st.orbit_period_days, st.phase0),
                parent_id=st.parent_planet_id,
                is_station=True,
            )
        )

    return frames
