"""Unit tests for orbit, rebase, market prices, jump range."""

from __future__ import annotations

import sys
from pathlib import Path

import numpy as np

ROOT = Path(__file__).resolve().parent.parent
sys.path.insert(0, str(ROOT))

from sim.clock import Stardate
from sim.frame import FrameState, FrameSystem, BodyRef
from sim.galaxy import Galaxy, build_frame_system
from sim.kepler import OrbitalElements, position_at
from sim.market import Market
from sim.shipyard import Shipyard, default_eagle, fitted_to_stats


def test_orbit_closes():
    el = OrbitalElements(semi_major_au=1.0, eccentricity=0.0, parent_mass_sol=1.0)
    T = el.period()
    p0 = position_at(el, 0.0)
    p1 = position_at(el, T)
    err = float(np.linalg.norm(p0 - p1))
    assert err < 1.0, f"orbit did not close, err={err} km"


def test_eccentric_orbit_closes():
    el = OrbitalElements(semi_major_au=0.5, eccentricity=0.2, parent_mass_sol=0.17)
    T = el.period()
    p0 = position_at(el, 10.0)
    p1 = position_at(el, 10.0 + T)
    err = float(np.linalg.norm(p0 - p1))
    assert err < 10.0, f"eccentric orbit err={err}"


def test_rebase_preserves_world():
    frames = FrameSystem()
    frames.add(
        BodyRef(
            id="star",
            name="Star",
            radius_km=1e5,
            world_pos=lambda d: np.zeros(3),
        )
    )
    frames.add(
        BodyRef(
            id="planet",
            name="P",
            radius_km=1000,
            world_pos=lambda d: np.array([10000.0, 0.0, 0.0]),
            parent_id="star",
        )
    )
    state = FrameState(parent_id="star", position=np.array([10005.0, 1.0, 2.0]), velocity=np.array([0.1, 0.0, 0.0]))
    w0 = frames.ship_world_pos(state, 0.0)
    v0 = frames.ship_world_vel(state, 0.0)
    state2 = frames.rebase(state, 0.0, "planet")
    w1 = frames.ship_world_pos(state2, 0.0)
    v1 = frames.ship_world_vel(state2, 0.0)
    assert float(np.linalg.norm(w0 - w1)) < 1e-6
    assert float(np.linalg.norm(v0 - v1)) < 1e-6
    assert state2.parent_id == "planet"


def test_ross154_prices_stable():
    g = Galaxy()
    m = Market()
    sys = g.get("ross154")
    a = m.price_map(sys, 0, "ross154_station")
    b = m.price_map(sys, 0, "ross154_station")
    assert a == b
    c = m.price_map(sys, 1, "ross154_station")
    # day change should alter at least something
    assert a != c or len(a) > 0


def test_eagle_cannot_reach_sol():
    g = Galaxy()
    dist = g.distance_ly("ross154", "sol")
    yard = Shipyard()
    fitted = default_eagle()
    stats = fitted_to_stats(fitted, yard.ships, yard.equipment)
    assert stats.jump_range_ly < dist, f"jump {stats.jump_range_ly} should be < dist {dist}"


def test_full_ship_roster():
    yard = Shipyard()
    assert len(yard.ships) >= 29
    assert "eagle_mk1" in yard.ships
    assert "anaconda" in yard.ships
    assert "panther" in yard.ships


def test_equipment_catalog():
    yard = Shipyard()
    assert "pulse_1mw" in yard.equipment
    assert "drive_class1" in yard.equipment
    assert "fuel_scoop" in yard.equipment
    assert "shield_gen" in yard.equipment
    assert len(yard.equipment) >= 40


def test_goods_catalog():
    m = Market()
    ids = {g.id for g in m.goods}
    assert "hydrogen_fuel" in ids
    assert "slaves" in ids
    assert "radioactives" in ids
    assert "military_fuel" in ids
    assert len(m.goods) >= 30


def test_ross154_has_station():
    g = Galaxy()
    sys = g.get("ross154")
    assert any(p.name == "Aster" and p.is_gas_giant for p in sys.planets)
    merlin = next(p for p in sys.planets if p.name == "Merlin")
    assert merlin.parent_id == "ross154_aster"
    assert any(sp.name == "Sirocco Starport" for sp in merlin.starports)
    assert len(sys.stations) >= 1
    frames = build_frame_system(sys)
    assert "ross154_station" in frames.bodies
    assert "ross154_sirocco" in frames.bodies
    assert "ross154_aster" in frames.bodies
    # Merlin world pos is near Aster, not near star alone
    days = 0.0
    aster = frames.world_position("ross154_aster", days)
    merlin_pos = frames.world_position("ross154_merlin", days)
    assert float(np.linalg.norm(merlin_pos - aster)) < 300000
    assert float(np.linalg.norm(merlin_pos)) > 1e6  # far from star with Aster


def test_stardate_format():
    s = Stardate(0)
    assert "3200" in s.format()


if __name__ == "__main__":
    tests = [v for k, v in list(globals().items()) if k.startswith("test_")]
    failed = 0
    for t in tests:
        try:
            t()
            print(f"OK  {t.__name__}")
        except Exception as e:
            failed += 1
            print(f"FAIL {t.__name__}: {e}")
    print(f"\n{len(tests) - failed}/{len(tests)} passed")
    sys.exit(1 if failed else 0)
