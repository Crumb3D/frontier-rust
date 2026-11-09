"""Frontier-style orbital stations and surface starports.

Geometry is in a local frame: +Z is the docking mouth / approach, +Y is up.
Positions and spin come from the simulation clock, not from the player's ship.
"""

from __future__ import annotations

import math

import numpy as np

from render.meshes import Mesh

_STATION: dict[tuple[bool, bool], Mesh] = {}
_PORT: dict[tuple[bool, bool], Mesh] = {}


def spin_about_y(days: float, period_days: float, phase: float = 0.0) -> np.ndarray:
    if period_days <= 1e-6:
        return np.eye(3, dtype=np.float64)
    a = phase + 2.0 * math.pi * days / period_days
    c, s = math.cos(a), math.sin(a)
    return np.array([[c, 0.0, s], [0.0, 1.0, 0.0], [-s, 0.0, c]], dtype=np.float64)


def station_orientation(days: float, station_id: str) -> np.ndarray:
    """Visible spin of an orbital station. Period is a few minutes of game time."""
    phase = (hash(station_id) % 997) / 997.0 * math.pi * 2
    return spin_about_y(days, 0.006, phase)


def dock_mouth(center: np.ndarray, orientation: np.ndarray, radius_km: float) -> np.ndarray:
    """World point just outside the docking slot. +Z in the station frame."""
    return center + orientation[:, 2] * (radius_km * 2.4)


def surface_frame(normal: np.ndarray) -> np.ndarray:
    """Columns are right, up, forward. Up follows the surface normal."""
    up = normal / max(float(np.linalg.norm(normal)), 1e-9)
    helper = np.array([0.0, 0.0, 1.0]) if abs(up[1]) > 0.85 else np.array([0.0, 1.0, 0.0])
    right = np.cross(helper, up)
    right = right / max(float(np.linalg.norm(right)), 1e-9)
    forward = np.cross(up, right)
    return np.column_stack((right, up, forward))


def _box(cx, cy, cz, sx, sy, sz, color) -> tuple[list, list, list]:
    x0, x1 = cx - sx, cx + sx
    y0, y1 = cy - sy, cy + sy
    z0, z1 = cz - sz, cz + sz
    return (
        [
            (x0, y0, z0), (x1, y0, z0), (x1, y1, z0), (x0, y1, z0),
            (x0, y0, z1), (x1, y0, z1), (x1, y1, z1), (x0, y1, z1),
        ],
        [
            (0, 3, 2, 1), (4, 5, 6, 7), (0, 1, 5, 4),
            (3, 7, 6, 2), (0, 4, 7, 3), (1, 2, 6, 5),
        ],
        [color] * 6,
    )


def _merge(parts: list[tuple[list, list, list]]) -> Mesh:
    verts: list = []
    faces: list = []
    colors: list = []
    offset = 0
    for vs, fs, cs in parts:
        verts.extend(vs)
        faces.extend(tuple(i + offset for i in f) for f in fs)
        colors.extend(cs)
        offset += len(vs)
    return Mesh(np.array(verts, dtype=np.float64), faces, colors)


def orbital_station_mesh(cleared: bool, flash_on: bool) -> Mesh:
    """Docking bay from the manual: walls, a back wall, and the entrance lights.

    The red light flashes until clearance. It then stays white, and the orange
    homing lights come on. Fig 43 is the bay, not a Coriolis cube.
    """
    key = (cleared, flash_on)
    cached = _STATION.get(key)
    if cached is not None:
        return cached
    wall = (138, 142, 150)
    back = (88, 92, 102)
    if cleared:
        beacon = (230, 230, 235)
        guide = (255, 150, 30)
    elif flash_on:
        beacon = (220, 30, 28)
        guide = (70, 28, 24)
    else:
        beacon = (90, 16, 16)
        guide = (50, 22, 20)
    parts = [
        _box(0, 0, -0.55, 0.7, 0.45, 0.08, back),
        _box(-0.62, 0, 0.15, 0.08, 0.45, 0.7, wall),
        _box(0.62, 0, 0.15, 0.08, 0.45, 0.7, wall),
        _box(0, 0.48, 0.15, 0.55, 0.06, 0.7, wall),
        _box(0, -0.48, 0.15, 0.55, 0.06, 0.7, wall),
        _box(0, 0.58, 0.8, 0.08, 0.06, 0.08, beacon),
        _box(-0.35, 0.2, 0.82, 0.05, 0.04, 0.04, guide),
        _box(0.35, 0.2, 0.82, 0.05, 0.04, 0.04, guide),
        _box(-0.35, -0.2, 0.82, 0.05, 0.04, 0.04, guide),
        _box(0.35, -0.2, 0.82, 0.05, 0.04, 0.04, guide),
    ]
    mesh = _merge(parts)
    _STATION[key] = mesh
    return mesh


def starport_mesh(sealed: bool, lock_open: bool) -> Mesh:
    """City from the Amiga FAQ, or the manual's hexagonal airlock.

    Breathable cities are concrete blocks, a green patch and a pad.
    A world with no breathable atmosphere keeps a hexagonal door over the
    pad until clearance opens it.
    """
    key = (sealed, lock_open)
    cached = _PORT.get(key)
    if cached is not None:
        return cached
    pad = (170, 170, 176)
    concrete = (120, 120, 124)
    brick = (140, 90, 70)
    green = (60, 110, 55)
    parts = [
        _box(0, 0.02, 0.1, 0.7, 0.02, 0.45, pad),
        _box(-0.85, 0.18, -0.35, 0.22, 0.16, 0.2, concrete),
        _box(0.7, 0.14, -0.5, 0.28, 0.12, 0.16, concrete),
        _box(0.15, 0.1, -0.7, 0.1, 0.08, 0.1, brick),
        _box(-0.2, 0.025, -0.15, 0.25, 0.01, 0.2, green),
    ]
    if sealed and not lock_open:
        parts.append(_box(0, 0.08, 0.1, 0.55, 0.04, 0.4, (70, 110, 120)))
    elif not sealed:
        parts.append(_box(0, 0.22, -0.85, 0.06, 0.2, 0.06, (150, 148, 140)))
    mesh = _merge(parts)
    _PORT[key] = mesh
    return mesh
