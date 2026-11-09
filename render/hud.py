"""FE2 Amiga cockpit HUD — speed strip + authentic console bar."""

from __future__ import annotations

import math

import numpy as np
import pygame

from render.console_bar import ConsoleHits, console_bar_height, draw_docked_console

ORANGE = (255, 168, 32)
GREEN = (40, 220, 70)
RED = (220, 40, 40)
WHITE = (240, 240, 240)
GREY = (140, 140, 150)


def console_height(h: int) -> int:
    """Flight HUD console — match docked FE2 strip (~18% of height)."""
    return console_bar_height(h)


def draw_hud(
    surface: pygame.Surface,
    font: pygame.font.Font,
    small: pygame.font.Font,
    *,
    speed: float,
    altitude: float,
    fuel: float,
    max_fuel: float,
    cabin_temp: float,
    energy: float,
    max_energy: float,
    shield: float,
    max_shield: float,
    hull: float,
    max_hull: float,
    stardate: str,
    compression: int,
    compression_index: int,
    system_name: str,
    set_speed: float | None,
    target_name: str,
    credits: int,
    legal: bool,
    messages: list[str],
    relative_to: str = "",
    view_name: str = "Front View",
    paused: bool = False,
    active_icon: int | None = None,
    show_hud: bool = True,
    nav_status: str = "",
    view_mode: str = "front",
    map_mode: str = "galaxy",
    info_page: int = 0,
    engines_off: bool = False,
    gear_down: bool = True,
    hud_labels: bool = True,
    hyperspace_ready: bool = False,
    autopilot: bool = False,
    has_autopilot: bool = False,
    center_mode: str = "scanner",
    has_scanner: bool = False,
    weapon_items: list | None = None,
    mapper_line: str = "",
    camera_line: str = "",
) -> ConsoleHits | None:
    w, h = surface.get_size()
    ch = console_height(h)
    top = h - ch

    # Speed strip just above console (on 3D view)
    speed_kmh = abs(speed) * 3600.0
    set_kmh = (abs(set_speed) * 3600.0) if set_speed is not None else speed_kmh
    strip_y = top - 14
    surface.blit(small.render(f"Actual: {speed_kmh:.1f} km/h", True, WHITE), (6, strip_y))
    surface.blit(small.render(f"Set: {set_kmh:.1f} km/h", True, WHITE), (w // 2 - 50, strip_y))
    rel = relative_to or system_name
    rel_s = small.render(f"Relative to: {rel}", False, WHITE)
    rx = int(2304 * w / 2928)
    surface.blit(rel_s, (rx, strip_y))
    if altitude < 500:
        surface.blit(small.render(f"Alt: {max(0, altitude)*1000:.0f} m", True, WHITE), (6, strip_y - 12))

    msg = messages[-1] if messages else view_name
    hits = draw_docked_console(
        surface,
        small,
        stardate=stardate,
        message=msg,
        docked=False,
        comms=False,
        compression_index=compression_index,
        paused=paused,
        active_icon=active_icon,
        view_mode=view_mode,
        map_mode=map_mode,
        info_page=info_page,
        engines_off=engines_off,
        gear_down=gear_down,
        hud_labels=hud_labels,
        hyperspace_ready=hyperspace_ready,
        autopilot=autopilot,
        has_autopilot=has_autopilot,
        view_label=view_name,
        center_mode=center_mode,
        has_scanner=has_scanner,
        weapon_items=weapon_items,
    )

    from render.console_bar import scanner_well_rect

    if center_mode != "weapons":
        well = scanner_well_rect(w, h)
        inst = small.render(
            f"Fuel {fuel:.1f}/{max_fuel:.1f}t   Sh {shield:.0f}   Hull {hull:.0f}",
            True,
            ORANGE,
        )
        surface.blit(inst, (well.x + 6, well.bottom - inst.get_height() - 1))

    if show_hud:
        # Messages stacked top-left; nav status stays on the right so they don't collide
        for i, m in enumerate(messages[-2:]):
            text = m if len(m) < 42 else m[:40] + ".."
            surface.blit(small.render(text, True, ORANGE), (6, 4 + i * 12))
        if nav_status:
            ns = small.render(nav_status, True, GREEN)
            surface.blit(ns, (w - ns.get_width() - 8, 4))
        extra = [ln for ln in (mapper_line, camera_line) if ln]
        for i, ln in enumerate(extra):
            surface.blit(small.render(ln, True, (80, 220, 230)), (6, 28 + i * 11))

    return hits


def draw_scanner(
    surface: pygame.Surface,
    contacts: list[tuple[np.ndarray, str, tuple[int, int, int]]],
    forward: np.ndarray,
    right: np.ndarray,
    up: np.ndarray,
) -> None:
    """Blips + vertical stalks on the FE2 3D scanner (centre console dish)."""
    from render.console_bar import scanner_well_rect

    w, h = surface.get_size()
    well = scanner_well_rect(w, h)
    scx, scy = well.centerx, well.centery + 2
    srx, sry = well.w // 2 - 14, well.h // 2 - 8
    range_km = 120.0
    # Own-ship pip
    pygame.draw.rect(surface, GREEN, (scx - 1, scy - 1, 3, 3))
    for rel, _name, color in contacts:
        x = float(np.dot(rel, right))
        z = float(np.dot(rel, forward))
        y = float(np.dot(rel, up))
        dist = math.sqrt(x * x + z * z + y * y)
        if dist > range_km or dist < 0.01:
            continue
        sx = scx + (x / range_km) * srx
        sy = scy - (z / range_km) * sry
        if not well.inflate(-4, -4).collidepoint(int(sx), int(sy)):
            continue
        stalk = int(np.clip(y / range_km * (well.h * 0.35), -well.h // 3, well.h // 3))
        pygame.draw.line(surface, color, (int(sx), int(sy)), (int(sx), int(sy - stalk)), 1)
        pygame.draw.rect(surface, color, (int(sx) - 1, int(sy - stalk) - 1, 3, 3))


def draw_compass(
    surface: pygame.Surface,
    forward: np.ndarray,
    to_target: np.ndarray | None,
) -> None:
    pass


def draw_gunsight(surface: pygame.Surface) -> None:
    w, h = surface.get_size()
    ch = console_height(h)
    cx, cy = w // 2, (h - ch) // 2
    for a in range(0, 360, 45):
        rad = math.radians(a)
        x0 = cx + int(math.cos(rad) * 5)
        y0 = cy + int(math.sin(rad) * 5)
        x1 = cx + int(math.cos(rad) * 12)
        y1 = cy + int(math.sin(rad) * 12)
        pygame.draw.line(surface, WHITE, (x0, y0), (x1, y1), 1)
