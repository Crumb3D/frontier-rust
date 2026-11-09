"""Interactive FE2-style galactic chart camera + picker."""

from __future__ import annotations

import math
from dataclasses import dataclass, field

import numpy as np
import pygame

from sim.galaxy import Galaxy, StarSystem

AMBER = (255, 180, 40)
GREEN = (40, 220, 80)
WHITE = (220, 220, 220)
CYAN = (80, 200, 220)
GRID = (20, 90, 40)
STEM = (140, 60, 40)
NAVY = (8, 16, 48)
FED_BLUE = (51, 136, 255)
INDIE_ORANGE = (255, 170, 0)


@dataclass
class ChartCamera:
    """Orbit camera for the galactic map — drag rotate, wheel zoom, click centre."""

    center: np.ndarray = field(default_factory=lambda: np.zeros(3, dtype=np.float64))
    rot_y: float = 0.35  # yaw
    rot_x: float = 0.55  # pitch (tilt)
    scale: float = 18.0  # pixels per light-year
    dragging: bool = False
    last_mouse: tuple[int, int] = (0, 0)
    # Cached pick list from last draw: (sid, tip_x, tip_y)
    picks: list[tuple[str, int, int]] = field(default_factory=list)

    def look_at(self, pos: np.ndarray) -> None:
        self.center = np.array(pos, dtype=np.float64).copy()

    def zoom(self, factor: float) -> None:
        self.scale = float(np.clip(self.scale * factor, 4.0, 80.0))

    def begin_drag(self, pos: tuple[int, int]) -> None:
        self.dragging = True
        self.last_mouse = pos

    def end_drag(self) -> None:
        self.dragging = False

    def drag(self, pos: tuple[int, int]) -> None:
        if not self.dragging:
            return
        dx = pos[0] - self.last_mouse[0]
        dy = pos[1] - self.last_mouse[1]
        self.last_mouse = pos
        self.rot_y += dx * 0.01
        self.rot_x = float(np.clip(self.rot_x + dy * 0.01, -1.2, 1.2))

    def project(
        self, world: np.ndarray, cx: int, cy: int
    ) -> tuple[int, int, int, float, float]:
        """Return (px, plane_y, tip_y, height_ly, depth)."""
        p = world - self.center
        x, y, z = float(p[0]), float(p[1]), float(p[2])
        cy_r, sy = math.cos(self.rot_y), math.sin(self.rot_y)
        cx_r, sx = math.cos(self.rot_x), math.sin(self.rot_x)
        # Yaw around Y, then pitch around X
        x, z = x * cy_r + z * sy, -x * sy + z * cy_r
        y, z = y * cx_r - z * sx, y * sx + z * cx_r
        # Perspective foreshortening on depth
        persp = 1.0 / (1.0 + max(-0.4, z) * 0.02)
        sc = self.scale * persp
        px = int(cx + x * sc)
        plane_y = int(cy + z * sc * 0.55)
        tip_y = int(plane_y - y * sc * 0.85)
        return px, plane_y, tip_y, y, z

    def pick(self, mx: int, my: int, radius: float = 22.0) -> str | None:
        best = None
        best_d = radius
        for sid, tx, ty in self.picks:
            d = math.hypot(tx - mx, ty - my)
            if d < best_d:
                best_d = d
                best = sid
        return best


def _allegiance_color(sys: StarSystem) -> tuple[int, int, int]:
    alleg = getattr(sys, "allegiance", "") or ""
    if "Fed" in alleg:
        return FED_BLUE
    if alleg:
        return INDIE_ORANGE
    return sys.star_color()


def draw_galactic_chart(
    surface: pygame.Surface,
    font: pygame.font.Font,
    small: pygame.font.Font,
    galaxy: Galaxy,
    current_id: str,
    selected_id: str | None,
    jump_range: float,
    camera: ChartCamera,
    *,
    stardate: str = "",
    fuel: float = 0.0,
    compression_index: int = 0,
    paused: bool = False,
    docked: bool = False,
    view_mode: str = "front",
    info_page: int = 0,
    engines_off: bool = False,
    gear_down: bool = True,
    hud_labels: bool = True,
    hyperspace_ready: bool = False,
    autopilot: bool = False,
    map_tools: bool = False,
):
    """FE2 galactic map with interactive 3D orbit camera."""
    from render.console_bar import console_bar_height, draw_docked_console

    w, h = surface.get_size()
    ch = console_bar_height(h)
    map_h = h - ch
    surface.fill(NAVY)

    # Starfield dust
    rng_seed = int(abs(camera.center[0] * 100 + camera.center[2] * 10)) & 0xFFFF
    import random
    rng = random.Random(rng_seed)
    for _ in range(80):
        surface.set_at((rng.randrange(w), rng.randrange(max(1, map_h))), (40, 50, 80))

    cx, cy = w // 2, int(map_h * 0.52)
    sc = camera.scale

    # Rotated green grid on galactic plane (y = 0). F8 on this screen toggles it.
    grid_span = max(8, int(220 / max(sc, 1)))
    if getattr(camera, "show_grid", True):
        step = 1.0
        for gx in range(-grid_span, grid_span + 1):
            line = []
            for gz in range(-grid_span, grid_span + 1):
                world = np.array([camera.center[0] + gx * step, 0.0, camera.center[2] + gz * step], dtype=np.float64)
                px, plane_y, _, _, _ = camera.project(world, cx, cy)
                line.append((px, plane_y))
            if len(line) >= 2:
                pygame.draw.lines(surface, GRID, False, line, 1)
        for gz in range(-grid_span, grid_span + 1):
            line = []
            for gx in range(-grid_span, grid_span + 1):
                world = np.array([camera.center[0] + gx * step, 0.0, camera.center[2] + gz * step], dtype=np.float64)
                px, plane_y, _, _, _ = camera.project(world, cx, cy)
                line.append((px, plane_y))
            if len(line) >= 2:
                pygame.draw.lines(surface, GRID, False, line, 1)

    # Jump range circle on plane around current system
    cur = galaxy.get(current_id)
    ring = []
    for i in range(48):
        ang = i * math.tau / 48
        world = cur.position_ly + np.array(
            [math.cos(ang) * jump_range, -cur.position_ly[1], math.sin(ang) * jump_range],
            dtype=np.float64,
        )
        # on galactic plane y=0, offset in xz from current
        world = np.array(
            [
                cur.position_ly[0] + math.cos(ang) * jump_range,
                0.0,
                cur.position_ly[2] + math.sin(ang) * jump_range,
            ],
            dtype=np.float64,
        )
        px, plane_y, _, _, _ = camera.project(world, cx, cy)
        ring.append((px, plane_y))
    if len(ring) >= 2:
        pygame.draw.lines(surface, (40, 120, 60), True, ring, 1)

    # CORE watermark
    core = font.render("CORE", True, (80, 20, 30))
    surface.blit(core, (w - core.get_width() - 24, 28))

    sel = selected_id or current_id
    camera.picks = []
    items = []
    for sid, sys in galaxy.systems.items():
        px, plane_y, tip_y, height, depth = camera.project(sys.position_ly, cx, cy)
        dist = galaxy.distance_ly(current_id, sid)
        items.append((depth, sid, sys, px, plane_y, tip_y, height, dist))
    items.sort(key=lambda t: t[0])

    for _, sid, sys, px, plane_y, tip_y, height, dist in items:
        if not (-20 <= px < w + 20 and -20 <= tip_y < map_h + 20):
            continue
        camera.picks.append((sid, px, tip_y))
        col = _allegiance_color(sys)
        if sid == current_id:
            col = GREEN
        elif sid == sel:
            col = AMBER
        # Stem from galactic plane foot to star
        pygame.draw.line(surface, STEM, (px, plane_y), (px, tip_y), 1)
        r = 5 if sid == current_id else (4 if sid == sel else max(2, int(2 + sys.mass * 2)))
        pygame.draw.circle(surface, col, (px, tip_y), r)
        if sid == sel:
            pygame.draw.circle(surface, WHITE, (px, tip_y), r + 2, 1)
        show_label = dist < 22 / (sc / 18) or sid in (current_id, sel)
        if show_label:
            surface.blit(small.render(sys.name, True, WHITE), (px + 6, tip_y - 5))

    # HUD hints
    surface.blit(small.render("Click destination star  H/Enter/F8 = jump  Drag/Wheel navigate", True, CYAN), (8, 8))
    sector_x = int(round(camera.center[0] / 8))
    sector_z = int(round(camera.center[2] / 8))
    surface.blit(small.render(f"Sector: {sector_x},{sector_z}", True, CYAN), (8, map_h - 24))
    surface.blit(small.render(f"Zoom {sc:.0f}", True, CYAN), (w - 70, 8))

    if sel:
        need = galaxy.distance_ly(current_id, sel) * 0.15
        if need > fuel + 1e-6 and sel != current_id:
            surface.blit(
                small.render(f"Insufficient Fuel [{need:.1f}t required]", True, WHITE),
                (8, map_h - 36),
            )

    s = galaxy.get(sel)
    d = galaxy.distance_ly(current_id, sel)
    alleg = getattr(s, "allegiance", "") or ""
    msg = f"Type {s.spectral} / {s.economy}. {s.government}."
    if alleg:
        msg = f"Type {s.spectral} / {alleg}. {s.economy}."
    hits = draw_docked_console(
        surface, small, stardate=stardate or "—", message=msg,
        docked=docked, comms=docked, compression_index=compression_index,
        paused=paused, view_mode=view_mode, map_mode="galaxy", screen="galaxy", map_tools=map_tools,
        recess_label="Galactic Map", info_page=info_page,
        engines_off=engines_off, gear_down=gear_down, hud_labels=hud_labels,
        hyperspace_ready=hyperspace_ready, autopilot=autopilot,
    )
    surface.blit(small.render(s.name[:10], False, GREEN), (w // 2 - 130, h - ch + 4))
    surface.blit(small.render(f"Dist. {d:.2f} light years", True, AMBER), (w // 2 - 55, h - ch + 4))
    return hits
