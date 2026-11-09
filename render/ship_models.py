"""Ship hulls from the published Elite blueprints (Ian Bell / David Braben).

Source: the 6502 ship meshes clawed out of the original data
(data/elite_hulls.txt). Nose is +Z. Each Frontier ship id picks that hull,
then laden mass sets the size. Fittings that sit on the outside are added
on top of the same mesh.
"""

from __future__ import annotations

import re
from pathlib import Path

import numpy as np

from render.meshes import Mesh

HULL = (176, 182, 196)
ENGINE = (36, 88, 160)
GUN = (200, 190, 150)
GEAR = (90, 90, 96)
_TOKEN = re.compile(r"-?&[0-9A-Fa-f]+|-?\d+")

# Frontier id -> blueprint name in elite_hulls.txt.
# Names that exist in both games use that ship's own vertices.
_HULL = {
    "sidewinder": "SIDEWINDER",
    "krait": "KRAIT",
    "gecko": "GECKO",
    "adder": "ADDER",
    "viper": "VIPER",
    "cobra_mk1": "COBRAMK1",
    "cobra_mk3": "COBRA",
    "moray": "MORAY",
    "constrictor": "CONSTRICTOR",
    "asp": "ASP",
    "transporter": "TRANSPORTER",
    "python": "PYTHON",
    "anaconda": "ANACONDA",
    "boa": "BOA",
    "mamba": "MAMBA",
    "shuttle": "SHUTTLE",
}

_LADEN = {
    "eagle_mk1": 25, "eagle_mk2": 28, "eagle_mk3": 30, "mamba": 30,
    "sidewinder": 33, "krait": 35, "cobra_mk1": 75, "cobra_mk3": 100, "moray": 87,
    "viper": 65, "osprey": 15, "falcon": 16, "hawk": 18, "kestrel": 20,
    "adder": 55, "gecko": 45, "asp": 150, "constrictor": 120, "imperial_courier": 480,
    "python": 500, "imperial_trader": 700, "tiger": 400,
    "anaconda": 800, "boa": 1500, "puma": 1000, "imperial_explorer": 1600,
    "panther": 2000, "transporter": 200, "lion": 300, "shuttle": 8, "lifter": 10,
}

_cache: dict[str, Mesh] | None = None


def _num(token: str) -> int:
    token = token.strip()
    if token.startswith("-&"):
        return -int(token[2:], 16)
    if token.startswith("&"):
        return int(token[1:], 16)
    return int(token)


def _parse_hulls() -> dict[str, Mesh]:
    path = Path(__file__).resolve().parents[1] / "data" / "elite_hulls.txt"
    text = path.read_text(encoding="utf-8", errors="replace")
    ships: dict[str, Mesh] = {}
    name = None
    mode = None
    verts: list[tuple[int, int, int]] = []
    faces: list[tuple[int, ...]] = []
    normals: list[tuple[int, int, int]] = []

    def finish() -> None:
        if name and verts and faces:
            ships[name] = _mesh_from(verts, faces, normals)

    for raw in text.splitlines():
        line = raw.strip()
        if not line or line.startswith("REM"):
            continue
        if line == ":":
            finish()
            name = None
            mode = None
            verts = []
            faces = []
            normals = []
            continue
        if name is None:
            name = line.split()[0]
            continue
        if line == "vertices":
            mode = "v"
            continue
        if line == "faces":
            mode = "f"
            continue
        if line == "hue-lig-sat":
            mode = None
            continue
        tokens = _TOKEN.findall(line)
        if mode == "v" and len(tokens) >= 3:
            verts.append((_num(tokens[0]), _num(tokens[1]), _num(tokens[2])))
        elif mode == "f" and len(tokens) >= 6:
            count = _num(tokens[4])
            idx = [_num(t) for t in tokens[5:5 + count]]
            if len(idx) >= 3:
                faces.append(tuple(idx))
                normals.append((_num(tokens[1]), _num(tokens[2]), _num(tokens[3])))
    finish()
    return ships


def _mesh_from(verts, faces, normals) -> Mesh:
    v = np.array(verts, dtype=np.float64)
    # Published data is +Z nose, +Y up. Fit the longest axis to 1.
    v -= v.mean(axis=0)
    extent = float(np.max(np.ptp(v, axis=0))) or 1.0
    v /= extent
    wound = []
    for face, normal in zip(faces, normals):
        pts = v[list(face[:3])]
        got = np.cross(pts[1] - pts[0], pts[2] - pts[0])
        if float(np.dot(got, np.array(normal, dtype=np.float64))) < 0:
            face = tuple(reversed(face))
        wound.append(face)
    colors = []
    for normal in normals:
        colors.append(ENGINE if normal[2] < -40 else HULL)
    return Mesh(v, wound, colors)


def _hulls() -> dict[str, Mesh]:
    global _cache
    if _cache is None:
        _cache = _parse_hulls()
    return _cache


def _merge(parts: list[Mesh]) -> Mesh:
    verts = []
    faces: list[tuple[int, ...]] = []
    colors: list[tuple[int, int, int]] = []
    offset = 0
    for mesh in parts:
        verts.append(mesh.vertices)
        for face in mesh.faces:
            faces.append(tuple(i + offset for i in face))
        colors.extend(mesh.face_colors[: len(mesh.faces)])
        while len(colors) < len(faces):
            colors.append(HULL)
        offset += len(mesh.vertices)
    return Mesh(np.vstack(verts), faces, colors)


def _box_at(cx, cy, cz, sx, sy, sz, color) -> Mesh:
    x0, x1 = cx - sx, cx + sx
    y0, y1 = cy - sy, cy + sy
    z0, z1 = cz - sz, cz + sz
    verts = np.array(
        [
            (x0, y0, z0), (x1, y0, z0), (x1, y1, z0), (x0, y1, z0),
            (x0, y0, z1), (x1, y0, z1), (x1, y1, z1), (x0, y1, z1),
        ],
        dtype=np.float64,
    )
    faces = [
        (0, 3, 2, 1), (4, 5, 6, 7), (0, 1, 5, 4),
        (2, 3, 7, 6), (0, 4, 7, 3), (1, 2, 6, 5),
    ]
    return Mesh(verts, faces, [color] * len(faces))


def ship_scale(ship_id: str) -> float:
    laden = _LADEN.get(ship_id, 40)
    return 0.85 * (laden / 25.0) ** 0.33


def hull_mesh(ship_id: str) -> Mesh:
    library = _hulls()
    key = _HULL.get(ship_id)
    mesh = library.get(key) if key else None
    if mesh is None:
        mesh = library["SHUTTLE"]
    return Mesh(mesh.vertices.copy(), list(mesh.faces), list(mesh.face_colors))


def build_ship_mesh(
    ship_id: str,
    *,
    guns: dict[str, str] | None = None,
    missile_count: int = 0,
    equipment: list[str] | None = None,
    gear_down: bool = False,
) -> tuple[Mesh, float]:
    guns = guns or {}
    equipment = equipment or []
    scale = ship_scale(ship_id)
    parts = [hull_mesh(ship_id)]
    nose = float(parts[0].vertices[:, 2].max())
    tail = float(parts[0].vertices[:, 2].min())
    span = float(np.max(np.abs(parts[0].vertices[:, 0])))
    if "front" in guns:
        parts.append(_box_at(0, 0, nose + 0.06, 0.02, 0.02, 0.06, GUN))
    if "rear" in guns:
        parts.append(_box_at(0, 0.04, tail - 0.04, 0.02, 0.02, 0.05, GUN))
    if "top" in guns:
        parts.append(_box_at(0, 0.16, 0.05, 0.02, 0.05, 0.02, GUN))
    if "bottom" in guns:
        parts.append(_box_at(0, -0.16, 0.0, 0.02, 0.05, 0.02, GUN))
    pylons = missile_count + sum(1 for e in equipment if str(e).startswith("mine_"))
    for i in range(min(pylons, 4)):
        side = -1 if i % 2 == 0 else 1
        parts.append(_box_at(side * span * 0.55, -0.08, -0.05, 0.03, 0.03, 0.06, (180, 70, 40)))
    if "fuel_scoop" in equipment:
        parts.append(_box_at(0, -0.1, 0.0, span * 0.15, 0.02, 0.12, (60, 90, 130)))
    if "mining_machine" in equipment:
        parts.append(_box_at(0, -0.12, tail * 0.4, 0.08, 0.04, 0.08, (150, 130, 70)))
    if any(e in equipment for e in ("ecm", "ecm_naval")):
        parts.append(_box_at(0, 0.14, -0.05, 0.03, 0.03, 0.03, (70, 190, 110)))
    if gear_down:
        parts.append(_box_at(-span * 0.35, -0.12, 0.05, 0.015, 0.06, 0.015, GEAR))
        parts.append(_box_at(span * 0.35, -0.12, 0.05, 0.015, 0.06, 0.015, GEAR))
    mesh = _merge(parts)
    mesh.vertices = mesh.vertices * scale
    return mesh, scale


def blit_ship_preview(surface, rect, ship_id: str, **fit) -> None:
    import pygame

    from render.soft import draw_mesh

    card = pygame.Surface((rect.w, rect.h))
    card.fill((6, 4, 16))
    pygame.draw.rect(card, (90, 40, 140), card.get_rect(), 1)
    mesh, scale = build_ship_mesh(ship_id, **fit)
    dist = max(scale * 3.2, 1.4)
    yaw, pitch = 0.7, 0.28
    cam = np.array(
        [
            dist * np.cos(pitch) * np.sin(yaw),
            dist * np.sin(pitch),
            -dist * np.cos(pitch) * np.cos(yaw),
        ]
    )
    forward = -cam / np.linalg.norm(cam)
    right = np.cross(forward, np.array([0.0, 1.0, 0.0]))
    right = right / max(float(np.linalg.norm(right)), 1e-6)
    up = np.cross(right, forward)
    draw_mesh(
        card, mesh, np.eye(3), np.zeros(3), cam,
        np.column_stack((right, up, forward)), np.array([0.4, 0.55, 0.75]),
        scale=1.0, fov=rect.h * 1.15,
    )
    surface.blit(card, rect.topleft)
