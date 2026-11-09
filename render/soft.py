"""Painter's-algorithm flat shader (1993 style)."""

from __future__ import annotations

import pygame
import numpy as np

from render.meshes import Mesh


def shade_color(
    base: tuple[int, int, int],
    normal: np.ndarray,
    light_dir: np.ndarray,
    ambient: float = 0.18,
) -> tuple[int, int, int]:
    n = normal / max(float(np.linalg.norm(normal)), 1e-9)
    l = light_dir / max(float(np.linalg.norm(light_dir)), 1e-9)
    diff = max(0.0, float(np.dot(n, -l)))  # light_dir points from surface to light? use toward light
    # light_dir should be from vertex toward sun; we pass sun - pos so use +dot
    diff = max(0.0, float(np.dot(n, l)))
    f = ambient + (1.0 - ambient) * diff
    return (
        int(min(255, base[0] * f)),
        int(min(255, base[1] * f)),
        int(min(255, base[2] * f)),
    )


def transform_mesh(
    mesh: Mesh,
    model: np.ndarray,
    camera_pos: np.ndarray,
    camera_rot: np.ndarray,
) -> tuple[np.ndarray, list]:
    """Return camera-space vertices and face records."""
    # model: 3x3 orientation, applied to verts then + world offset baked into camera
    world = (mesh.vertices @ model.T)
    cam = (world - camera_pos) @ camera_rot
    return cam, mesh.faces


def project(v: np.ndarray, fov: float, w: int, h: int) -> tuple[float, float] | None:
    if v[2] <= 0.05:
        return None
    f = fov / v[2]
    x = w * 0.5 + v[0] * f
    y = h * 0.5 - v[1] * f
    return x, y


def draw_mesh(
    surface: pygame.Surface,
    mesh: Mesh,
    orientation: np.ndarray,
    world_pos: np.ndarray,
    camera_pos: np.ndarray,
    camera_rot: np.ndarray,
    light_dir: np.ndarray,
    scale: float = 1.0,
    fov: float = 400.0,
    interior: bool = False,
) -> None:
    w, h = surface.get_size()
    verts = mesh.vertices * scale
    world = (verts @ orientation.T) + world_pos
    cam = (world - camera_pos) @ camera_rot

    faces_draw = []
    for fi, face in enumerate(mesh.faces):
        pts = cam[list(face)]
        if np.any(pts[:, 2] <= 0.08):
            continue
        # Normal in camera space
        e1 = pts[1] - pts[0]
        e2 = pts[2] - pts[0]
        n = np.cross(e1, e2)
        # Backface: if normal points away from camera (z component of facing)
        # Camera looks +Z; face visible if normal · view < 0 where view is toward cam from face
        view = -pts[0]  # toward camera
        facing = float(np.dot(n, view))
        if interior:
            facing = -facing
        if facing <= 0:
            continue
        depth = float(np.mean(pts[:, 2]))
        # Light in world: transform normal back approx using orientation
        nw = orientation @ np.cross(
            (verts[face[1]] - verts[face[0]]),
            (verts[face[2]] - verts[face[0]]),
        )
        col = shade_color(mesh.face_colors[fi % len(mesh.face_colors)], nw, light_dir)
        proj = []
        ok = True
        for p in pts:
            pr = project(p, fov, w, h)
            if pr is None:
                ok = False
                break
            proj.append(pr)
        if ok and len(proj) >= 3:
            faces_draw.append((depth, proj, col))

    faces_draw.sort(key=lambda x: -x[0])  # far to near
    for _, proj, col in faces_draw:
        pygame.draw.polygon(surface, col, proj)
        pygame.draw.polygon(surface, (max(0, col[0] - 30), max(0, col[1] - 30), max(0, col[2] - 30)), proj, 1)


def draw_sphere_approx(
    surface: pygame.Surface,
    center_world: np.ndarray,
    radius_km: float,
    camera_pos: np.ndarray,
    camera_rot: np.ndarray,
    color: tuple[int, int, int],
    light_dir: np.ndarray,
    fov: float = 400.0,
    rings: bool = False,
    atmosphere: bool = False,
    gas: bool = False,
    spin: float = 0.0,
) -> None:
    """Draw planet as shaded disc (fast, period look). Optional Saturn-style rings."""
    w, h = surface.get_size()
    cam = (center_world - camera_pos) @ camera_rot
    if cam[2] <= radius_km * 0.5:
        return
    pr = project(cam, fov, w, h)
    if pr is None:
        return
    r_px = max(2.0, fov * radius_km / cam[2])
    if r_px > max(w, h) * 2:
        r_px = max(w, h) * 2
    ld = light_dir / max(float(np.linalg.norm(light_dir)), 1e-9)
    lit = shade_color(color, np.array([0.0, 0.0, 1.0]), ld, ambient=0.22)
    dark = shade_color(color, np.array([0.0, 0.0, -1.0]), ld, ambient=0.22)

    # Far half of rings first (painter)
    if rings and r_px > 4:
        rw, rh = int(r_px * 2.4), int(r_px * 0.42)
        rect = pygame.Rect(0, 0, rw, rh)
        rect.center = (int(pr[0]), int(pr[1]))
        pygame.draw.ellipse(surface, (160, 140, 90), rect, max(1, int(r_px * 0.04)))
        inner = pygame.Rect(0, 0, int(rw * 0.72), int(rh * 0.72))
        inner.center = rect.center
        pygame.draw.ellipse(surface, (90, 75, 45), inner, max(1, int(r_px * 0.03)))

    pygame.draw.circle(surface, dark, (int(pr[0]), int(pr[1])), int(r_px))
    ox = int(pr[0] - ld[0] * r_px * 0.25)
    oy = int(pr[1] + ld[1] * r_px * 0.25)
    pygame.draw.circle(surface, lit, (ox, oy), int(r_px * 0.85))
    if atmosphere and r_px > 3:
        rim = (min(255, color[0] + 40), min(255, color[1] + 50), min(255, color[2] + 70))
        pygame.draw.circle(surface, rim, (int(pr[0]), int(pr[1])), int(r_px * 1.06), max(1, int(r_px * 0.04)))
    if gas and r_px > 6:
        import math as _math
        for i, shift in enumerate((-0.35, -0.1, 0.15, 0.38)):
            band = (
                min(255, color[0] + (i - 1) * 18),
                min(255, color[1] + (1 - i) * 12),
                max(0, color[2] - i * 8),
            )
            by = int(pr[1] + r_px * (shift + 0.04 * _math.sin(spin * 6.28 + i)))
            pygame.draw.line(surface, band, (int(pr[0] - r_px * 0.7), by), (int(pr[0] + r_px * 0.7), by), max(1, int(r_px * 0.06)))
    elif r_px > 8:
        import math as _math
        mark = (max(0, color[0] - 30), max(0, color[1] - 20), max(0, color[2] - 10))
        mx = int(pr[0] + _math.sin(spin * 6.28) * r_px * 0.35)
        my = int(pr[1] + _math.cos(spin * 6.28) * r_px * 0.2)
        pygame.draw.circle(surface, mark, (mx, my), max(1, int(r_px * 0.08)))

    if rings and r_px > 4:
        # Near arc of rings (clip by drawing bottom half thicker)
        rw, rh = int(r_px * 2.4), int(r_px * 0.42)
        rect = pygame.Rect(0, 0, rw, rh)
        rect.center = (int(pr[0]), int(pr[1] + r_px * 0.05))
        # Only draw lower half approximately with arcs
        pygame.draw.arc(surface, (200, 180, 110), rect, 3.4, 6.1, max(1, int(r_px * 0.05)))


def draw_sun(
    surface: pygame.Surface,
    sun_world: np.ndarray,
    camera_pos: np.ndarray,
    camera_rot: np.ndarray,
    color: tuple[int, int, int],
    fov: float = 400.0,
) -> None:
    w, h = surface.get_size()
    cam = (sun_world - camera_pos) @ camera_rot
    if cam[2] <= 1:
        return
    pr = project(cam, fov, w, h)
    if pr is None:
        return
    r = max(4, min(80, fov * 80000 / cam[2]))
    pygame.draw.circle(surface, color, (int(pr[0]), int(pr[1])), int(r))
    glow = tuple(min(255, c + 40) for c in color)
    pygame.draw.circle(surface, glow, (int(pr[0]), int(pr[1])), int(r * 0.5))
