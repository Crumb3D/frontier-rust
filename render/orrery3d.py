"""Interactive FE2-style 3D system orrery (pygame software raster)."""

from __future__ import annotations

import math
from dataclasses import dataclass, field

import numpy as np
import pygame

from sim.galaxy import Planet, StarSystem, Station
from sim.kepler import AU_KM, position_at

AMBER = (255, 180, 40)
GREEN = (40, 220, 80)
DIM = (50, 70, 90)
WHITE = (230, 230, 235)
CYAN = (80, 210, 230)
STEM = (160, 70, 50)
ORBIT = (40, 100, 70)
ORBIT_DIM = (30, 55, 45)
NAVY = (4, 8, 24)
PANEL_BG = (12, 10, 28)
YELLOW = (255, 210, 60)


@dataclass
class OrreryCamera:
    """Orbit / pan / zoom camera for the system map (units: km from star)."""

    center: np.ndarray = field(default_factory=lambda: np.zeros(3, dtype=np.float64))
    rot_y: float = 0.45
    rot_x: float = 0.70
    scale: float = 140.0  # pixels per AU
    dragging: bool = False
    drag_mode: str = "rotate"  # rotate | pan
    last_mouse: tuple[int, int] = (0, 0)
    show_stems: bool = False
    show_names: bool = False
    show_orbits: bool = False
    show_grid: bool = False
    picks: list[tuple[str, int, int]] = field(default_factory=list)

    def look_at(self, pos: np.ndarray) -> None:
        self.center = np.array(pos, dtype=np.float64).copy()

    def zoom(self, factor: float) -> None:
        self.scale = float(np.clip(self.scale * factor, 8.0, 12000.0))

    def begin_drag(self, pos: tuple[int, int], mode: str = "rotate") -> None:
        self.dragging = True
        self.drag_mode = mode
        self.last_mouse = pos

    def end_drag(self) -> None:
        self.dragging = False

    def drag(self, pos: tuple[int, int]) -> None:
        if not self.dragging:
            return
        dx = pos[0] - self.last_mouse[0]
        dy = pos[1] - self.last_mouse[1]
        self.last_mouse = pos
        if self.drag_mode == "pan":
            # Pan in camera-facing plane
            cy_r, sy = math.cos(self.rot_y), math.sin(self.rot_y)
            # Right vector in XZ
            right = np.array([cy_r, 0.0, sy], dtype=np.float64)
            up = np.array([0.0, 1.0, 0.0], dtype=np.float64)
            km_per_px = AU_KM / max(self.scale, 1e-6)
            self.center = self.center - right * dx * km_per_px * 0.8 + up * dy * km_per_px * 0.8
        else:
            self.rot_y += dx * 0.01
            self.rot_x = float(np.clip(self.rot_x + dy * 0.01, 0.08, 1.45))

    def project(self, world_km: np.ndarray, cx: int, cy: int) -> tuple[int, int, int, float, float]:
        """Return (px, plane_y, tip_y, height_km, depth)."""
        p = (world_km - self.center) / AU_KM  # work in AU for stable scale
        x, y, z = float(p[0]), float(p[1]), float(p[2])
        cy_r, sy = math.cos(self.rot_y), math.sin(self.rot_y)
        cx_r, sx = math.cos(self.rot_x), math.sin(self.rot_x)
        x, z = x * cy_r + z * sy, -x * sy + z * cy_r
        y, z = y * cx_r - z * sx, y * sx + z * cx_r
        persp = 1.0 / (1.0 + max(-0.5, z) * 0.15)
        sc = self.scale * persp
        px = int(cx + x * sc)
        plane_y = int(cy + z * sc * 0.55)
        tip_y = int(plane_y - y * sc * 0.9)
        return px, plane_y, tip_y, float((world_km - self.center)[1]), z

    def pick(self, mx: int, my: int, radius: float = 22.0) -> str | None:
        best = None
        best_d = radius
        for sid, tx, ty in self.picks:
            d = math.hypot(tx - mx, ty - my)
            if d < best_d:
                best_d = d
                best = sid
        return best


def _moon_visual_radius_km(moon: Planet, parent: Planet, scale_px_per_au: float) -> float:
    """Exaggerate moon orbits so they're readable at system scale (FE2-style)."""
    base = max(moon.moon_orbit_km, parent.radius_km * 2.5)
    # At low zoom, boost moons; at high zoom approach true size
    boost = float(np.clip(28.0 - math.log10(max(scale_px_per_au, 10.0)) * 6.0, 4.0, 22.0))
    return base * boost


def body_world_km(system: StarSystem, planet_id: str, days: float, scale_px: float) -> np.ndarray:
    """Star-centric km position with FE2-exaggerated moon orbits."""
    p = system.planet_by_id(planet_id)
    if p is None:
        return np.zeros(3)
    if p.parent_id is None:
        return position_at(p.elements, days)
    parent = system.planet_by_id(p.parent_id)
    if parent is None:
        return position_at(p.elements, days)
    parent_pos = body_world_km(system, parent.id, days, scale_px)
    r = _moon_visual_radius_km(p, parent, scale_px)
    ang = p.moon_phase0 + 2.0 * math.pi * days / max(p.moon_period_days, 1e-6)
    # Incline moon orbit slightly for 3D depth
    inc = 0.25
    offset = np.array(
        [
            math.cos(ang) * r,
            math.sin(ang) * r * math.sin(inc),
            math.sin(ang) * r * math.cos(inc),
        ],
        dtype=np.float64,
    )
    return parent_pos + offset


def station_world_km(system: StarSystem, st: Station, days: float, scale_px: float) -> np.ndarray:
    parent = body_world_km(system, st.parent_planet_id, days, scale_px)
    p = system.planet_by_id(st.parent_planet_id)
    r = st.orbit_radius_km
    if p and scale_px < 400:
        r = max(r, p.radius_km * 1.8) * 3.0
    ang = st.phase0 + 2.0 * math.pi * days / max(st.orbit_period_days, 1e-6)
    return parent + np.array([math.cos(ang) * r, 0.0, math.sin(ang) * r], dtype=np.float64)


def _draw_orbit_ellipse(
    surface: pygame.Surface,
    cam: OrreryCamera,
    cx: int,
    cy: int,
    center_km: np.ndarray,
    radius_km: float,
    inclination_deg: float,
    color: tuple[int, int, int],
    map_h: int,
    segments: int = 64,
) -> None:
    """Wireframe circular/elliptical orbit in 3D, projected."""
    inc = math.radians(inclination_deg)
    pts: list[tuple[int, int]] = []
    depths: list[float] = []
    for i in range(segments + 1):
        ang = i * math.tau / segments
        # Orbit in parent's local frame: XZ with Y from inclination
        local = np.array(
            [
                math.cos(ang) * radius_km,
                math.sin(ang) * radius_km * math.sin(inc),
                math.sin(ang) * radius_km * math.cos(inc),
            ],
            dtype=np.float64,
        )
        px, _plane, tip, _h, depth = cam.project(center_km + local, cx, cy)
        # Use tip as the orbit point (true 3D position)
        world = center_km + local
        px, plane_y, tip_y, _, depth = cam.project(world, cx, cy)
        pts.append((px, tip_y))
        depths.append(depth)
    # Draw only on-screen segments
    for i in range(len(pts) - 1):
        a, b = pts[i], pts[i + 1]
        if (0 <= a[0] < surface.get_width() or 0 <= b[0] < surface.get_width()) and (
            -40 <= a[1] < map_h + 40 or -40 <= b[1] < map_h + 40
        ):
            pygame.draw.line(surface, color, a, b, 1)


def _body_screen_radius(p: Planet, scale: float) -> int:
    # Visual size from log radius; grow slightly when zoomed
    base = math.log10(p.radius_km + 10) * (2.2 if p.is_gas_giant else 1.6)
    zoom_boost = float(np.clip(math.log10(scale / 50.0 + 1) * 1.5, 0, 6))
    return max(3, int(base + zoom_boost))


def draw_orrery(
    surface: pygame.Surface,
    font: pygame.font.Font,
    small: pygame.font.Font,
    system: StarSystem,
    days: float,
    selected_body: str | None,
    camera: OrreryCamera,
    *,
    stardate: str = "",
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
    system_page: str = "bodies",
    system_index: int = 0,
):
    """Interactive 3D FE2 system map."""
    from render.console_bar import console_bar_height, draw_docked_console

    w, h = surface.get_size()
    ch = console_bar_height(h)
    map_h = h - ch
    surface.fill((0, 0, 0))

    cx, cy = int(w * 0.22), int(map_h * 0.28)

    # Ecliptic grid (y = 0 plane). F8 on this screen toggles it.
    grid_au = max(2, int(8 * 140 / max(camera.scale, 1)))
    if getattr(camera, "show_grid", True):
        for gx in range(-grid_au, grid_au + 1):
            line = []
            for gz in range(-grid_au, grid_au + 1):
                world = np.array([gx * AU_KM * 0.5, 0.0, gz * AU_KM * 0.5], dtype=np.float64)
                px, plane_y, _, _, _ = camera.project(world, cx, cy)
                line.append((px, plane_y))
            if len(line) >= 2:
                pygame.draw.lines(surface, (18, 45, 35), False, line, 1)
        for gz in range(-grid_au, grid_au + 1):
            line = []
            for gx in range(-grid_au, grid_au + 1):
                world = np.array([gx * AU_KM * 0.5, 0.0, gz * AU_KM * 0.5], dtype=np.float64)
                px, plane_y, _, _, _ = camera.project(world, cx, cy)
                line.append((px, plane_y))
            if len(line) >= 2:
                pygame.draw.lines(surface, (18, 45, 35), False, line, 1)

    if camera.show_orbits:
        for p in system.planets:
            if p.parent_id is not None:
                continue
            r_km = p.elements.semi_major_au * AU_KM
            _draw_orbit_ellipse(
                surface, camera, cx, cy, np.zeros(3), r_km,
                p.elements.inclination_deg, ORBIT, map_h,
            )
        for p in system.planets:
            if p.parent_id is None:
                continue
            parent = system.planet_by_id(p.parent_id)
            if not parent:
                continue
            parent_pos = body_world_km(system, parent.id, days, camera.scale)
            r = _moon_visual_radius_km(p, parent, camera.scale)
            _draw_orbit_ellipse(
                surface, camera, cx, cy, parent_pos, r, 12.0, ORBIT_DIM, map_h, segments=48,
            )

    # Star
    star_pos = np.zeros(3, dtype=np.float64)
    spx, splane, stip, _, _ = camera.project(star_pos, cx, cy)
    star_r = max(6, int(5 + math.log10(system.mass + 0.01) * 3))
    if camera.show_stems:
        pygame.draw.line(surface, STEM, (spx, splane), (spx, stip), 1)
    pygame.draw.circle(surface, system.star_color(), (spx, stip), star_r)
    pygame.draw.circle(surface, AMBER, (spx, stip), star_r + 1, 1)

    camera.picks = [("star", spx, stip)]
    items: list[tuple[float, str, object, int, int, int, int]] = []

    # Planets + moons
    for p in system.planets:
        pos = body_world_km(system, p.id, days, camera.scale)
        px, plane_y, tip_y, _, depth = camera.project(pos, cx, cy)
        items.append((depth, p.id, p, px, plane_y, tip_y, _body_screen_radius(p, camera.scale)))

    for st in system.stations:
        pos = station_world_km(system, st, days, camera.scale)
        px, plane_y, tip_y, _, depth = camera.project(pos, cx, cy)
        items.append((depth, st.id, st, px, plane_y, tip_y, 3))

    items.sort(key=lambda t: t[0])

    for _, bid, body, px, plane_y, tip_y, rad in items:
        if not (-30 <= px < w + 30 and -30 <= tip_y < map_h + 30):
            continue
        camera.picks.append((bid, px, tip_y))
        selected = bid == selected_body
        if camera.show_stems:
            pygame.draw.line(surface, STEM, (px, plane_y), (px, tip_y), 1)
        if isinstance(body, Planet):
            col = AMBER if selected else body.color
            pygame.draw.circle(surface, col, (px, tip_y), rad)
            if selected:
                pygame.draw.circle(surface, WHITE, (px, tip_y), rad + 2, 1)
            # Billboard label
            has_port = bool(body.starports) or body.has_starport
            label_col = CYAN if has_port else WHITE
            if body.parent_id:
                label_col = CYAN
            if camera.show_names:
                lab = small.render(body.name, True, AMBER if selected else label_col)
                surface.blit(lab, (px + rad + 3, tip_y - 5))
            # Starport markers (pickable for autopilot)
            for i, sp in enumerate(body.starports):
                spx, spy = px + rad + 8, tip_y + 8 + i * 12
                sp_sel = sp.id == selected_body
                surface.blit(
                    small.render(f"*{sp.name}", True, AMBER if sp_sel else CYAN),
                    (spx, spy),
                )
                camera.picks.append((sp.id, spx + 20, spy + 5))
        else:
            # Station
            col = AMBER if selected else CYAN
            pygame.draw.rect(surface, col, (px - 2, tip_y - 2, 5, 5))
            if selected:
                pygame.draw.rect(surface, WHITE, (px - 4, tip_y - 4, 9, 9), 1)
            if camera.show_names:
                surface.blit(small.render(body.name, True, CYAN), (px + 6, tip_y - 4))

    if system_page == "trade":
        _draw_trade(surface, small, system, map_h)
        desc = system.name
    elif system_page == "facts":
        _draw_facts(surface, small, system, map_h, system_index)
        desc = system.name
    else:
        lines = _info_lines(system, selected_body)
        y = map_h - 14 - 12 * len(lines)
        for i, line in enumerate(lines):
            surface.blit(small.render(line, True, YELLOW), (16, max(8, y + i * 12)))
        desc = lines[0] if lines else system.name
    hits = draw_docked_console(
        surface, small, stardate=stardate or "—", message=desc,
        docked=docked, comms=False, compression_index=compression_index,
        paused=paused, view_mode=view_mode, map_mode="system", screen="system", map_tools=map_tools,
        recess_label="System Map", info_page=info_page,
        engines_off=engines_off, gear_down=gear_down, hud_labels=camera.show_names,
        hyperspace_ready=hyperspace_ready, autopilot=autopilot,
    )
    surface.blit(small.render(system.name[:10], False, GREEN), (w // 2 - 130, h - ch + 4))
    return hits


def _draw_trade(surface, small, system: StarSystem, map_h: int) -> None:
    from sim.market import Market

    board = getattr(_draw_trade, "_market", None)
    if board is None:
        board = Market()
        _draw_trade._market = board
    columns = board.trade_board(system)
    headers = ("Major Imports", "Minor Imports", "Major Exports", "Minor Exports", "Illegal Goods")
    x0, y0 = 12, int(map_h * 0.46)
    col_w = max(90, (surface.get_width() - 24) // len(headers))
    for i, header in enumerate(headers):
        x = x0 + i * col_w
        surface.blit(small.render(header, False, WHITE), (x, y0))
        yy = y0 + 16
        for name in columns[header]:
            if yy > map_h - 8:
                break
            surface.blit(small.render(name, False, YELLOW), (x, yy))
            yy += 12


def _draw_facts(surface, small, system: StarSystem, map_h: int, system_index: int) -> None:
    sx = int(round(float(system.position_ly[0]) / 8))
    sz = int(round(float(system.position_ly[2]) / 8))
    rows = (
        ("Government Type:", system.government),
        ("Economy Type:", system.economy),
        ("Allegiance:", system.allegiance),
        ("System Sector Code:", f"{sx},{sz}"),
        ("System Number:", str(system_index)),
    )
    y = int(map_h * 0.52)
    for label, value in rows:
        surface.blit(small.render(label, False, YELLOW), (36, y))
        surface.blit(small.render(value, False, YELLOW), (220, y))
        y += 16


def _earth_masses(p: Planet) -> float:
    """Mass from the stored radius. Gas giants use a lower density than rock."""
    density = 1.3 / 5.51 if p.is_gas_giant else 1.0
    return (p.radius_km / 6371.0) ** 3 * density


def _surface_c(system: StarSystem, p: Planet) -> float:
    """Equilibrium temperature from the star mass and this orbit."""
    a = p.elements.semi_major_au
    if p.parent_id:
        parent = system.planet_by_id(p.parent_id)
        if parent is not None:
            a = parent.elements.semi_major_au
    a = max(a, 0.02)
    lum = max(system.mass, 0.05) ** 3.5
    return 278.0 * (lum ** 0.25) / math.sqrt(a) - 273.15


def _kind(p: Planet) -> str:
    if p.is_gas_giant:
        return "Small gas giant" if p.radius_km < 40000 else "Gas giant"
    if p.parent_id:
        return "Moon"
    if p.inhabited and p.has_atmosphere:
        return "Inhabited world"
    if not p.has_atmosphere:
        return "Airless world"
    return "Planet"


def _info_lines(system: StarSystem, selected: str | None) -> list[str]:
    if not selected:
        major = 1 + sum(1 for p in system.planets if p.parent_id is None)
        return [f"System with {major} major bodies."]
    if selected == "star":
        return [
            system.name,
            f"Type: {system.spectral} star",
            f"Mass  {system.mass:.2f} solar masses",
            f"Major bodies  {1 + sum(1 for p in system.planets if p.parent_id is None)}",
        ]
    p = system.planet_by_id(selected)
    if p is not None:
        ports = ", ".join(sp.name for sp in p.starports) or "None"
        a = p.elements.semi_major_au
        period = p.elements.period()
        if p.parent_id:
            period = p.moon_period_days
            a = p.moon_orbit_km / AU_KM
        return [
            f"{p.name}: {_kind(p)}",
            f"Mass                 {_earth_masses(p):.2f} Earth masses",
            f"Surface temperature  {_surface_c(system, p):.0f} C",
            f"Major Starports      {ports}",
            f"Orbital period       {period:.0f} days",
            f"Orbital radius       {a:.3f} A.U.",
            f"Orbit Ecc. and Incl. {p.elements.eccentricity:.3f}, {p.elements.inclination_deg:.1f}",
        ]
    for st in system.stations:
        if st.id == selected:
            parent = system.planet_by_id(st.parent_planet_id)
            return [
                st.name,
                "Type: Orbital station",
                f"Around  {parent.name if parent else st.parent_planet_id}",
                f"Orbital period  {st.orbit_period_days:.2f} days",
                f"Orbital radius  {st.orbit_radius_km / AU_KM:.5f} A.U.",
            ]
    for planet in system.planets:
        for sp in planet.starports:
            if sp.id == selected:
                return [
                    sp.name,
                    "Type: Starport",
                    f"On  {planet.name}",
                    f"Pads  {len(sp.pads)}",
                ]
    return [system.name]
