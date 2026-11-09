"""FE2-style pixel icons for the bottom console function keys."""

from __future__ import annotations

import pygame

WHITE = (240, 240, 245)
GREY = (160, 160, 170)
CYAN = (80, 200, 230)
GREEN = (40, 220, 70)
AMBER = (255, 180, 40)
RED = (220, 60, 50)
BLUE = (80, 140, 255)


def _px(surface: pygame.Surface, x: int, y: int, col: tuple[int, int, int]) -> None:
    if 0 <= x < surface.get_width() and 0 <= y < surface.get_height():
        surface.set_at((x, y), col)


def _hline(s, x0, x1, y, c):
    for x in range(x0, x1 + 1):
        _px(s, x, y, c)


def _vline(s, x, y0, y1, c):
    for y in range(y0, y1 + 1):
        _px(s, x, y, c)


def icon_cockpit(s: pygame.Surface, ox: int, oy: int) -> None:
    """Front view / cockpit — trapezoid canopy."""
    pts = [(ox + 2, oy + 9), (ox + 5, oy + 2), (ox + 11, oy + 2), (ox + 14, oy + 9)]
    pygame.draw.polygon(s, CYAN, [(p[0], p[1]) for p in pts], 1)
    _hline(s, ox + 5, ox + 11, oy + 5, WHITE)
    _px(s, ox + 8, oy + 7, AMBER)


def icon_starmap(s: pygame.Surface, ox: int, oy: int) -> None:
    """3D starmap — grid + star dots."""
    for i in range(4):
        _hline(s, ox + 2, ox + 14, oy + 2 + i * 3, (50, 120, 60))
        _vline(s, ox + 2 + i * 4, oy + 2, oy + 11, (50, 120, 60))
    _px(s, ox + 5, oy + 4, AMBER)
    _px(s, ox + 10, oy + 7, WHITE)
    _px(s, ox + 7, oy + 10, CYAN)
    _vline(s, ox + 10, oy + 7, oy + 11, (140, 60, 40))


def icon_commander(s: pygame.Surface, ox: int, oy: int) -> None:
    """Commander / status — head silhouette."""
    pygame.draw.circle(s, WHITE, (ox + 8, oy + 4), 3, 1)
    pygame.draw.ellipse(s, WHITE, (ox + 4, oy + 7, 9, 5), 1)


def icon_comms(s: pygame.Surface, ox: int, oy: int) -> None:
    """Comms — radio waves / antenna."""
    _vline(s, ox + 8, oy + 3, oy + 11, RED)
    _hline(s, ox + 6, ox + 10, oy + 11, RED)
    pygame.draw.arc(s, CYAN, (ox + 4, oy + 2, 8, 8), 0.4, 2.8, 1)
    pygame.draw.arc(s, CYAN, (ox + 2, oy + 1, 12, 10), 0.5, 2.7, 1)


def icon_launch(s: pygame.Surface, ox: int, oy: int) -> None:
    """Launch / thrusters — upward chevron + flame."""
    pygame.draw.polygon(s, WHITE, [(ox + 8, oy + 2), (ox + 3, oy + 8), (ox + 13, oy + 8)], 1)
    _vline(s, ox + 8, oy + 8, oy + 11, AMBER)
    _px(s, ox + 7, oy + 12, RED)
    _px(s, ox + 8, oy + 13, AMBER)
    _px(s, ox + 9, oy + 12, RED)


def icon_hyperspace(s: pygame.Surface, ox: int, oy: int) -> None:
    """Hyperspace — star burst / warp."""
    cx, cy = ox + 8, oy + 7
    for dx, dy in ((0, -5), (0, 5), (-5, 0), (5, 0), (-3, -3), (3, -3), (-3, 3), (3, 3)):
        _px(s, cx + dx, cy + dy, AMBER)
    _px(s, cx, cy, WHITE)


def icon_gear(s: pygame.Surface, ox: int, oy: int) -> None:
    """Landing gear — wheel under strut."""
    _vline(s, ox + 8, oy + 2, oy + 7, WHITE)
    pygame.draw.circle(s, WHITE, (ox + 8, oy + 10), 3, 1)
    _px(s, ox + 8, oy + 10, GREY)


def icon_system(s: pygame.Surface, ox: int, oy: int) -> None:
    """System info — star + orbit."""
    pygame.draw.circle(s, AMBER, (ox + 8, oy + 7), 2)
    pygame.draw.ellipse(s, CYAN, (ox + 2, oy + 3, 13, 9), 1)
    _px(s, ox + 14, oy + 7, WHITE)


def icon_rear(s: pygame.Surface, ox: int, oy: int) -> None:
    """Rear view — reverse canopy."""
    pts = [(ox + 2, oy + 3), (ox + 14, oy + 3), (ox + 11, oy + 10), (ox + 5, oy + 10)]
    pygame.draw.polygon(s, CYAN, pts, 1)


def icon_external(s: pygame.Surface, ox: int, oy: int) -> None:
    """External 3D view — ship wedge."""
    pygame.draw.polygon(s, WHITE, [(ox + 8, oy + 2), (ox + 14, oy + 11), (ox + 2, oy + 11)], 1)
    _px(s, ox + 8, oy + 7, CYAN)


def icon_target(s: pygame.Surface, ox: int, oy: int) -> None:
    """Target / scanner reticle."""
    pygame.draw.circle(s, GREEN, (ox + 8, oy + 7), 5, 1)
    _hline(s, ox + 3, ox + 13, oy + 7, GREEN)
    _vline(s, ox + 8, oy + 2, oy + 12, GREEN)


def icon_autopilot(s: pygame.Surface, ox: int, oy: int) -> None:
    """Autopilot / engine mode — 'A' with arrow."""
    # Letter A
    pygame.draw.lines(s, AMBER, False, [(ox + 4, oy + 11), (ox + 8, oy + 2), (ox + 12, oy + 11)], 1)
    _hline(s, ox + 5, ox + 11, oy + 7, AMBER)


def icon_id(s: pygame.Surface, ox: int, oy: int) -> None:
    """ID text toggle — label tag."""
    pygame.draw.rect(s, WHITE, (ox + 3, oy + 4, 11, 7), 1)
    _hline(s, ox + 5, ox + 11, oy + 7, CYAN)


def icon_manual(s: pygame.Surface, ox: int, oy: int) -> None:
    """Manual flight — stick and grip."""
    _vline(s, ox + 8, oy + 5, oy + 12, WHITE)
    _hline(s, ox + 5, ox + 11, oy + 4, WHITE)
    _px(s, ox + 8, oy + 3, AMBER)


def icon_inertial(s: pygame.Surface, ox: int, oy: int) -> None:
    """Engines off — stick with a cut through it."""
    icon_manual(s, ox, oy)
    pygame.draw.line(s, RED, (ox + 3, oy + 2), (ox + 13, oy + 12), 1)


def icon_cargo(s: pygame.Surface, ox: int, oy: int) -> None:
    """Cargo hold — crate."""
    pygame.draw.rect(s, AMBER, (ox + 3, oy + 4, 10, 8), 1)
    _hline(s, ox + 3, ox + 12, oy + 7, AMBER)
    _vline(s, ox + 8, oy + 4, oy + 11, AMBER)


def icon_equipment(s: pygame.Surface, ox: int, oy: int) -> None:
    """Fitted weapons — laser on a mount."""
    _hline(s, ox + 2, ox + 13, oy + 10, WHITE)
    _hline(s, ox + 4, ox + 12, oy + 6, CYAN)
    _px(s, ox + 12, oy + 5, RED)
    _px(s, ox + 13, oy + 4, AMBER)


def icon_gear_up(s: pygame.Surface, ox: int, oy: int) -> None:
    """Gear retracted — strut only, wheel stowed."""
    _vline(s, ox + 8, oy + 3, oy + 8, GREY)
    _hline(s, ox + 5, ox + 11, oy + 3, GREY)


def icon_bulletin(s: pygame.Surface, ox: int, oy: int) -> None:
    """Starport bulletin board / local services — clipboard / noticeboard."""
    pygame.draw.rect(s, WHITE, (ox + 3, oy + 2, 10, 12), 1)
    _hline(s, ox + 5, ox + 11, oy + 1, AMBER)
    _hline(s, ox + 5, ox + 11, oy + 5, GREY)
    _hline(s, ox + 5, ox + 11, oy + 8, GREY)
    _hline(s, ox + 5, ox + 9, oy + 11, GREY)


# Docked: F1–F10
DOCKED_ICONS = {
    1: icon_cockpit,
    2: icon_starmap,
    3: icon_commander,
    4: icon_comms,
    5: icon_bulletin,
    6: icon_system,
    7: icon_launch,
    8: icon_hyperspace,
    9: icon_gear,
    10: icon_id,
}

# Flight: F1–F10
FLIGHT_ICONS = {
    1: icon_cockpit,
    2: icon_rear,
    3: icon_external,
    4: icon_target,
    5: icon_starmap,
    6: icon_commander,
    7: icon_autopilot,
    8: icon_hyperspace,
    9: icon_gear,
    10: icon_id,
}


def draw_fe2_icon(
    surface: pygame.Surface,
    key_num: int,
    x: int,
    y: int,
    *,
    docked: bool,
) -> None:
    """Blit a 16×14 icon at (x,y) for function key key_num."""
    from render.grf import get_graphics

    gfx = get_graphics()
    src = gfx.icons.get(key_num) if gfx.ok else None
    if src is not None:
        surface.blit(src, (x, y))
        return
    icons = DOCKED_ICONS if docked else FLIGHT_ICONS
    fn = icons.get(key_num)
    if not fn:
        return
    tile = pygame.Surface((16, 14), pygame.SRCALPHA)
    fn(tile, 0, 0)
    surface.blit(tile, (x, y))
