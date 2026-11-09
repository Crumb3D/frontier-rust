"""Low-poly original meshes in the flat-shaded FE2 style."""

from __future__ import annotations

import math
from dataclasses import dataclass

import numpy as np


@dataclass
class Mesh:
    vertices: np.ndarray  # (N, 3)
    faces: list[tuple[int, ...]]  # indices
    face_colors: list[tuple[int, int, int]]


def _box(sx: float, sy: float, sz: float, color: tuple[int, int, int]) -> Mesh:
    v = np.array(
        [
            [-sx, -sy, -sz],
            [sx, -sy, -sz],
            [sx, sy, -sz],
            [-sx, sy, -sz],
            [-sx, -sy, sz],
            [sx, -sy, sz],
            [sx, sy, sz],
            [-sx, sy, sz],
        ],
        dtype=np.float64,
    )
    faces = [
        (0, 1, 2, 3),
        (4, 7, 6, 5),
        (0, 4, 5, 1),
        (2, 6, 7, 3),
        (0, 3, 7, 4),
        (1, 5, 6, 2),
    ]
    return Mesh(v, faces, [color] * len(faces))


def eagle() -> Mesh:
    """Small interceptor wedge."""
    v = np.array(
        [
            [0.0, 0.0, 1.2],
            [-0.7, 0.0, -0.6],
            [0.7, 0.0, -0.6],
            [0.0, 0.25, -0.2],
            [0.0, -0.15, -0.2],
            [-0.9, 0.0, -0.9],
            [0.9, 0.0, -0.9],
        ],
        dtype=np.float64,
    )
    faces = [
        (0, 1, 3),
        (0, 3, 2),
        (0, 2, 4),
        (0, 4, 1),
        (1, 5, 3),
        (2, 3, 6),
        (1, 4, 5),
        (2, 6, 4),
        (3, 5, 6),
        (4, 6, 5),
    ]
    colors = [
        (180, 180, 200),
        (160, 160, 190),
        (140, 140, 160),
        (120, 120, 140),
        (100, 140, 180),
        (100, 140, 180),
        (80, 80, 100),
        (80, 80, 100),
        (60, 60, 80),
        (200, 80, 40),
    ]
    return Mesh(v, faces, colors)


def adder() -> Mesh:
    v = np.array(
        [
            [0, 0.2, 1.4],
            [-0.8, 0, 0.2],
            [0.8, 0, 0.2],
            [-0.6, 0.3, -1.0],
            [0.6, 0.3, -1.0],
            [-0.6, -0.2, -1.0],
            [0.6, -0.2, -1.0],
            [0, 0.4, 0.0],
        ],
        dtype=np.float64,
    )
    faces = [
        (0, 1, 7),
        (0, 7, 2),
        (0, 2, 6, 5, 1),
        (7, 1, 3),
        (7, 4, 2),
        (7, 3, 4),
        (1, 5, 3),
        (2, 4, 6),
        (3, 5, 6, 4),
    ]
    c = (120, 150, 100)
    return Mesh(v, faces, [c] * len(faces))


def viper() -> Mesh:
    v = np.array(
        [
            [0, 0, 1.5],
            [-0.5, 0.15, 0.3],
            [0.5, 0.15, 0.3],
            [-1.0, 0, -0.8],
            [1.0, 0, -0.8],
            [0, 0.35, -0.3],
            [0, -0.2, -0.5],
        ],
        dtype=np.float64,
    )
    faces = [(0, 1, 5), (0, 5, 2), (0, 2, 6, 1), (5, 1, 3), (5, 4, 2), (5, 3, 4), (1, 6, 3), (2, 4, 6), (3, 6, 4)]
    return Mesh(v, faces, [(200, 200, 210)] * 9)


def cobra() -> Mesh:
    v = np.array(
        [
            [0, 0.1, 1.6],
            [-1.2, 0, 0.2],
            [1.2, 0, 0.2],
            [-0.8, 0.25, -1.2],
            [0.8, 0.25, -1.2],
            [-0.8, -0.15, -1.2],
            [0.8, -0.15, -1.2],
        ],
        dtype=np.float64,
    )
    faces = [
        (0, 1, 3, 4, 2),
        (0, 2, 6, 5, 1),
        (1, 5, 3),
        (2, 4, 6),
        (3, 5, 6, 4),
    ]
    return Mesh(v, faces, [(90, 110, 140)] * 5)


def python_ship() -> Mesh:
    return _box(0.6, 0.35, 1.8, (100, 90, 70))


def anaconda() -> Mesh:
    return _box(0.9, 0.45, 2.4, (80, 100, 90))


def transporter() -> Mesh:
    return _box(0.5, 0.4, 1.5, (140, 120, 80))


def asp() -> Mesh:
    v = np.array(
        [
            [0, 0, 1.8],
            [-0.9, 0.2, 0],
            [0.9, 0.2, 0],
            [-0.5, 0.3, -1.4],
            [0.5, 0.3, -1.4],
            [0, -0.3, -0.5],
            [-1.1, 0, -0.8],
            [1.1, 0, -0.8],
        ],
        dtype=np.float64,
    )
    faces = [
        (0, 1, 2),
        (0, 2, 5, 1),
        (1, 6, 3),
        (2, 4, 7),
        (1, 3, 4, 2),
        (5, 6, 1),
        (5, 2, 7),
        (3, 6, 5, 7, 4),
    ]
    return Mesh(v, faces, [(160, 170, 190)] * len(faces))


def slot_station(radius: float = 2.0) -> Mesh:
    """Rotating slot / torus-ish station approximated as octagon ring + docking slot."""
    verts = []
    faces = []
    colors = []
    n = 12
    r_out = radius
    r_in = radius * 0.65
    thick = radius * 0.25
    for i in range(n):
        a0 = 2 * math.pi * i / n
        a1 = 2 * math.pi * (i + 1) / n
        for a in (a0,):
            c, s = math.cos(a), math.sin(a)
            verts.append([c * r_out, -thick, s * r_out])
            verts.append([c * r_out, thick, s * r_out])
            verts.append([c * r_in, thick, s * r_in])
            verts.append([c * r_in, -thick, s * r_in])
    v = np.array(verts, dtype=np.float64)
    for i in range(n):
        b = i * 4
        nxt = ((i + 1) % n) * 4
        # outer
        faces.append((b, nxt, nxt + 1, b + 1))
        colors.append((140, 140, 150))
        # inner
        faces.append((b + 3, b + 2, nxt + 2, nxt + 3))
        colors.append((100, 100, 120))
        # top
        faces.append((b + 1, nxt + 1, nxt + 2, b + 2))
        colors.append((160, 160, 170))
        # bottom
        faces.append((b, b + 3, nxt + 3, nxt))
        colors.append((80, 80, 90))
    # Docking slot markers (bright)
    slot_v_start = len(v)
    extra = np.array(
        [
            [-0.15, -thick * 1.2, r_in * 0.9],
            [0.15, -thick * 1.2, r_in * 0.9],
            [0.15, thick * 1.2, r_in * 0.9],
            [-0.15, thick * 1.2, r_in * 0.9],
            [-0.15, -thick * 1.2, r_in * 0.5],
            [0.15, -thick * 1.2, r_in * 0.5],
            [0.15, thick * 1.2, r_in * 0.5],
            [-0.15, thick * 1.2, r_in * 0.5],
        ],
        dtype=np.float64,
    )
    v = np.vstack([v, extra])
    s = slot_v_start
    faces.append((s, s + 1, s + 2, s + 3))
    colors.append((255, 200, 40))
    faces.append((s + 4, s + 7, s + 6, s + 5))
    colors.append((255, 180, 20))
    return Mesh(v, faces, colors)


def sphere(subdiv: int = 2, radius: float = 1.0) -> Mesh:
    """Coarse icosphere for planets."""
    t = (1.0 + math.sqrt(5.0)) / 2.0
    verts = [
        [-1, t, 0],
        [1, t, 0],
        [-1, -t, 0],
        [1, -t, 0],
        [0, -1, t],
        [0, 1, t],
        [0, -1, -t],
        [0, 1, -t],
        [t, 0, -1],
        [t, 0, 1],
        [-t, 0, -1],
        [-t, 0, 1],
    ]
    faces_i = [
        (0, 11, 5),
        (0, 5, 1),
        (0, 1, 7),
        (0, 7, 10),
        (0, 10, 11),
        (1, 5, 9),
        (5, 11, 4),
        (11, 10, 2),
        (10, 7, 6),
        (7, 1, 8),
        (3, 9, 4),
        (3, 4, 2),
        (3, 2, 6),
        (3, 6, 8),
        (3, 8, 9),
        (4, 9, 5),
        (2, 4, 11),
        (6, 2, 10),
        (8, 6, 7),
        (9, 8, 1),
    ]
    # Normalize
    v = np.array(verts, dtype=np.float64)
    v = v / np.linalg.norm(v, axis=1, keepdims=True) * radius
    colors = [(128, 128, 128)] * len(faces_i)
    return Mesh(v, faces_i, colors)


SHIP_MESHES = {
    "eagle_mk1": eagle,
    "eagle_mk2": eagle,
    "eagle_mk3": eagle,
    "mamba": eagle,
    "sidewinder": eagle,
    "krait": eagle,
    "osprey": eagle,
    "falcon": eagle,
    "hawk": eagle,
    "kestrel": eagle,
    "gecko": adder,
    "adder": adder,
    "viper": viper,
    "cobra_mk1": cobra,
    "cobra_mk3": cobra,
    "moray": cobra,
    "constrictor": asp,
    "asp": asp,
    "transporter": transporter,
    "lion": transporter,
    "tiger": python_ship,
    "python": python_ship,
    "imperial_courier": asp,
    "imperial_trader": python_ship,
    "imperial_explorer": anaconda,
    "anaconda": anaconda,
    "puma": anaconda,
    "boa": anaconda,
    "panther": anaconda,
    "shuttle": lambda: _box(0.3, 0.2, 0.5, (180, 180, 100)),
    "lifter": lambda: _box(0.35, 0.25, 0.55, (160, 140, 100)),
}


def mesh_for_ship(ship_id: str) -> Mesh:
    """Bare hull for traffic and previews. Player fittings use build_ship_mesh."""
    from render.ship_models import build_ship_mesh

    mesh, _scale = build_ship_mesh(ship_id)
    return mesh
