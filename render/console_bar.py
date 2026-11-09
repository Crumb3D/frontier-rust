"""Authentic FE2 Amiga docked bottom console — compact ~18% height + real icons."""

from __future__ import annotations

from dataclasses import dataclass, field

import pygame

from render.fe2_icons import draw_fe2_icon
from render.fe2_icons import (
    icon_bulletin,
    icon_cargo,
    icon_cockpit,
    icon_comms,
    icon_equipment,
    icon_external,
    icon_gear,
    icon_gear_up,
    icon_hyperspace,
    icon_id,
    icon_inertial,
    icon_launch,
    icon_manual,
    icon_rear,
    icon_starmap,
    icon_system,
    icon_commander,
)

ORANGE = (255, 168, 32)
GREEN = (40, 220, 70)
CRT_GREEN = (48, 230, 72)
GREY = (140, 140, 150)
WHITE = (248, 248, 248)
BLACK = (0, 0, 0)
PURPLE = (168, 72, 200)
PURPLE_DIM = (110, 48, 140)
PURPLE_BORDER = (190, 90, 220)
PANEL = (48, 48, 52)
PANEL_DARK = (20, 20, 24)
KEYCAP = (56, 56, 62)
BEZEL = (150, 150, 158)
HAZARD_A = (70, 70, 74)
HAZARD_B = (28, 28, 32)


@dataclass
class ConsoleHits:
    vcr: list[pygame.Rect] = field(default_factory=list)
    # (function_key_number, rect) — avoids off-by-one between left/right blocks
    icons: list[tuple[int, pygame.Rect]] = field(default_factory=list)
    icons_left: list[pygame.Rect] = field(default_factory=list)  # compat
    icons_right: list[pygame.Rect] = field(default_factory=list)  # compat
    top: int = 0
    height: int = 0
    flight_mode: pygame.Rect | None = None
    weapons: list[tuple[str, pygame.Rect]] = field(default_factory=list)


def _panel_surface(width: int) -> pygame.Surface:
    """The bare Amiga console, scaled to the logical screen width."""
    key = getattr(_panel_surface, "_key", None)
    if key == width and getattr(_panel_surface, "_img", None) is not None:
        return _panel_surface._img
    from pathlib import Path

    raw = pygame.image.load(str(Path(__file__).resolve().parents[1] / "assets" / "bottom panel.jpg")).convert()
    # The photograph has one Stardreamer glyph already lit, and a cursor on F1.
    # Both are game state, so they are painted back to the panel grey once.
    for y in range(100, 165):
        for x in range(210, 280):
            r, g, b, _a = raw.get_at((x, y))
            if r > 160 and r > g + 30 and b < 140:
                raw.set_at((x, y), (162, 162, 162))
    for y in range(185, 250):
        for x in range(50, 120):
            r, g, b, _a = raw.get_at((x, y))
            if b > 140 and r < 160:
                raw.set_at((x, y), (132, 132, 132))
    ph = max(48, round(raw.get_height() * width / raw.get_width()))
    img = pygame.transform.smoothscale(raw, (width, ph)).convert()
    _panel_surface._key = width
    _panel_surface._img = img
    return img


def console_bar_height(h: int) -> int:
    """Height of the reference console scaled to the 640-wide logical screen."""
    return _panel_surface(640).get_height()


def status_strip_height() -> int:
    return 11


def _v_line(surface: pygame.Surface, x: int, y0: int, y1: int, col: tuple[int, int, int]) -> None:
    pygame.draw.line(surface, col, (x, y0), (x, y1), 1)


def _keycap(
    surface: pygame.Surface,
    small: pygame.font.Font,
    rect: pygame.Rect,
    key_num: int,
    active: bool,
    docked: bool,
    *,
    warn: bool = False,
) -> None:
    fill = (36, 90, 48) if active else KEYCAP
    pygame.draw.rect(surface, fill, rect)
    border = GREEN if active else (18, 18, 22)
    pygame.draw.rect(surface, border, rect, 2 if active else 1)
    if active:
        pygame.draw.rect(surface, (80, 180, 90), rect.inflate(-2, -2), 1)
    draw_fe2_icon(surface, key_num, rect.x + (rect.w - 16) // 2, rect.y + 1, docked=docked)
    n = small.render(str(key_num), True, GREY)
    surface.blit(n, (rect.right - n.get_width() - 1, rect.bottom - n.get_height()))
    # FE2: warning triangle over Take Off when no launch clearance
    if warn:
        cx = rect.centerx
        top = rect.y + 2
        pygame.draw.polygon(
            surface,
            (220, 40, 40),
            [(cx, top), (cx - 5, top + 9), (cx + 5, top + 9)],
        )
        pygame.draw.polygon(
            surface,
            (255, 220, 40),
            [(cx, top + 1), (cx - 3, top + 7), (cx + 3, top + 7)],
            1,
        )
        pygame.draw.line(surface, (20, 20, 20), (cx, top + 3), (cx, top + 5), 1)
        surface.set_at((cx, top + 7), (20, 20, 20))


def _vcr_glyph(surface: pygame.Surface, rect: pygame.Rect, kind: int, active: bool) -> None:
    cx, cy = rect.centerx, rect.centery
    col = WHITE if active else (160, 160, 170)
    if kind == 0:
        pygame.draw.rect(surface, col, (cx - 3, cy - 3, 2, 6))
        pygame.draw.rect(surface, col, (cx + 1, cy - 3, 2, 6))
    elif kind == 1:
        pygame.draw.polygon(surface, col, [(cx - 2, cy - 3), (cx - 2, cy + 3), (cx + 3, cy)])
    else:
        for i in range(min(kind, 4)):
            ox = cx - 5 + i * 3
            pygame.draw.polygon(surface, col, [(ox, cy - 3), (ox, cy + 3), (ox + 3, cy)])


def _hazard_strip(surface: pygame.Surface, rect: pygame.Rect) -> None:
    pygame.draw.rect(surface, HAZARD_B, rect)
    step = 4
    for i in range(-rect.h, rect.w + rect.h, step):
        pts = [
            (rect.left + i, rect.top),
            (rect.left + i + max(1, step // 2), rect.top),
            (rect.left + i + max(1, step // 2) - rect.h, rect.bottom),
            (rect.left + i - rect.h, rect.bottom),
        ]
        pygame.draw.polygon(surface, HAZARD_A, pts)
    pygame.draw.circle(surface, GREEN, (rect.right - 5, rect.centery), 1)


def _ship_computer(
    surface: pygame.Surface,
    small: pygame.font.Font,
    mid: int,
    top: int,
    ch: int,
    message: str,
) -> None:
    plate_h = 16
    well_top = top + (ch - plate_h) // 2
    well_bottom = well_top + plate_h
    top_half = 120
    bot_half = 102
    pts = [
        (mid - top_half, well_top),
        (mid + top_half, well_top),
        (mid + bot_half, well_bottom),
        (mid - bot_half, well_bottom),
    ]
    pygame.draw.polygon(surface, PURPLE_BORDER, pts)
    inner = [
        (mid - top_half + 2, well_top + 1),
        (mid + top_half - 2, well_top + 1),
        (mid + bot_half - 2, well_bottom - 1),
        (mid - bot_half + 2, well_bottom - 1),
    ]
    pygame.draw.polygon(surface, BLACK, inner)
    pygame.draw.polygon(surface, GREEN, inner, 1)
    _hazard_strip(surface, pygame.Rect(mid - top_half + 10, well_top - 3, top_half * 2 - 20, 3))
    if not message:
        return
    max_w = bot_half * 2 - 14
    text = message
    while small.size(text)[0] > max_w and len(text) > 8:
        text = text[:-2]
    if text != message:
        text = text.rstrip() + ".."
    lab = small.render(text, True, CRT_GREEN)
    surface.blit(lab, (mid - lab.get_width() // 2, well_top + (plate_h - lab.get_height()) // 2))


def scanner_well_rect(w: int, h: int) -> pygame.Rect:
    """Dark centre well of the reference console (scanner / readout)."""
    ch = console_bar_height(h)
    top = h - ch
    s = w / 2928.0
    return pygame.Rect(int(860 * s), top + int(36 * s), int(1220 * s), int(240 * s))


def draw_flight_scanner_frame(
    surface: pygame.Surface,
    small: pygame.font.Font,
    *,
    message: str = "",
) -> pygame.Rect:
    """Draw the FE2 scanner dish / perspective grid in the centre console."""
    w, h = surface.get_size()
    ch = console_bar_height(h)
    top = h - ch
    mid = (128 + (w - 128)) // 2  # between left/right divs — same as docked mid
    left_div = 128
    right_div = w - 128
    mid = (left_div + right_div) // 2

    well = scanner_well_rect(w, h)
    # Red perspective grid, drawn on the reference well. The chrome stays.
    ell = pygame.Rect(well.x + 6, well.y + 2, max(8, well.w - 12), max(8, well.h - 6))
    red = (180, 30, 30)
    pygame.draw.ellipse(surface, red, ell, 1)
    cx, cy = ell.centerx, ell.centery
    for frac in (0.45, 0.72, 1.0):
        rw = int(ell.w * frac / 2)
        rh = int(ell.h * frac / 2)
        pygame.draw.ellipse(surface, red, (cx - rw, cy - rh, rw * 2, rh * 2), 1)
    pygame.draw.line(surface, red, (ell.left + 2, cy), (ell.right - 2, cy), 1)
    pygame.draw.line(surface, (220, 220, 220), (cx, cy - 2), (cx, cy + 2), 1)
    if message:
        max_w = 200
        text = message
        while small.size(text)[0] > max_w and len(text) > 8:
            text = text[:-2]
        if text != message:
            text = text.rstrip() + ".."
        lab = small.render(text, True, CRT_GREEN)
        surface.blit(lab, (mid - lab.get_width() // 2, top + 2))
    return well


def draw_weapon_panel(
    surface: pygame.Surface,
    small: pygame.font.Font,
    items: list[tuple[str, bool, int]],
) -> list[tuple[str, pygame.Rect]]:
    """Fitted-equipment controls in the centre well. Replaces the scanner."""
    w, h = surface.get_size()
    well = scanner_well_rect(w, h)
    hits: list[tuple[str, pygame.Rect]] = []
    x = well.x + 6
    y = well.y + 4
    for label, enabled, pips in items:
        col = (230, 230, 220) if enabled else (80, 80, 84)
        lab = small.render(label, True, col)
        if x + lab.get_width() > well.right - 6:
            x = well.x + 6
            y += lab.get_height() + 3
        rect = pygame.Rect(x, y, lab.get_width(), lab.get_height())
        surface.blit(lab, (x, y))
        hits.append((label, rect))
        x += lab.get_width() + 4
        for _i in range(min(int(pips), 8)):
            pygame.draw.rect(surface, (210, 36, 36) if enabled else (70, 40, 40), (x, y + 3, 2, 2))
            x += 4
        if pips:
            x += 4
    return hits


def draw_status_strip(
    surface: pygame.Surface,
    small: pygame.font.Font,
    hint: str,
    *,
    more: bool = False,
) -> None:
    w, h = surface.get_size()
    ch = console_bar_height(h)
    sh = status_strip_height()
    y = h - ch - sh
    pygame.draw.rect(surface, (56, 24, 96), (0, y, w, sh))
    pygame.draw.line(surface, PURPLE_BORDER, (0, y), (w, y), 1)
    if hint:
        surface.blit(small.render(hint, True, (220, 200, 80)), (8, y + 1))
    if more:
        lab = small.render("More...", True, ORANGE)
        surface.blit(lab, (w - lab.get_width() - 8, y + 1))


# Key wells on bottom panel.jpg (2928×301), inclusive pixel bounds.
_WELLS_L = ((9, 183, 157, 292), (167, 183, 313, 292), (323, 183, 470, 292), (480, 183, 628, 292))
_WELLS_R = ((2306, 183, 2450, 292), (2461, 183, 2608, 292), (2618, 183, 2764, 292), (2775, 183, 2917, 292))


def _slot_rects(width: int, top: int) -> tuple[list[pygame.Rect], list[pygame.Rect], list[pygame.Rect]]:
    """F1–F4, F7–F10, and the Stardreamer keys, measured off the reference image."""
    s = width / 2928.0

    def well(x0: int, y0: int, x1: int, y1: int) -> pygame.Rect:
        return pygame.Rect(int(x0 * s), top + int(y0 * s), max(8, int((x1 - x0 + 1) * s)), max(8, int((y1 - y0 + 1) * s)))

    def box(x: int, w: int, y0: int, y1: int) -> pygame.Rect:
        return pygame.Rect(int(x * s), top + int(y0 * s), max(8, int(w * s)), max(8, int((y1 - y0) * s)))

    keys_l = [well(*b) for b in _WELLS_L]
    keys_r = [well(*b) for b in _WELLS_R]
    # Button faces on bottom panel.jpg, y 109–163. Widths match each key, not the gap.
    vcr_slots = ((10, 30), (49, 30), (135, 40), (200, 76), (285, 186))
    vcr = [box(x, w, 109, 164) for x, w in vcr_slots]
    return keys_l, keys_r, vcr


def _manual_icon(name: str) -> pygame.Surface | None:
    cache = getattr(_manual_icon, "_cache", None)
    if cache is None:
        cache = {}
        _manual_icon._cache = cache
    if name in cache:
        return cache[name]
    from pathlib import Path

    path = Path(__file__).resolve().parents[1] / "assets" / "icons" / name
    if not path.is_file():
        cache[name] = None
        return None
    img = pygame.image.load(str(path)).convert()
    if img.get_size() != (48, 48):
        img = pygame.transform.scale(img, (48, 48))
    cache[name] = img
    return cache[name]


def _blit_vcr(surface: pygame.Surface, rect: pygame.Rect, name: str, *, selected: bool) -> None:
    """Fit a Stardreamer glyph in its own slot. Selected replaces the grey with white."""
    src = _manual_icon(name)
    if src is None:
        if selected:
            pygame.draw.rect(surface, (255, 255, 255), rect)
        return
    side = max(1, min(rect.w, rect.h) - 2)
    tile = pygame.transform.scale(src, (side, side))
    if selected:
        lit = tile.copy()
        for y in range(lit.get_height()):
            for x in range(lit.get_width()):
                r, g, b, a = lit.get_at((x, y))
                if a < 16:
                    continue
                if abs(r - g) < 28 and abs(g - b) < 28 and 50 < r < 210:
                    lit.set_at((x, y), (255, 255, 255, a))
        tile = lit
    surface.blit(tile, (rect.x + (rect.w - tile.get_width()) // 2, rect.y + (rect.h - tile.get_height()) // 2))


def _blit_state_icon(surface: pygame.Surface, rect: pygame.Rect, name: str, fn, *, muted: bool = False) -> None:
    """Fill the measured key well with the manual icon for that key."""
    src = _manual_icon(name) if name else None
    if src is None:
        return
    tile = pygame.transform.scale(src, rect.size)
    if muted:
        dim = pygame.Surface(tile.get_size())
        dim.fill((70, 70, 74))
        tile = tile.copy()
        tile.blit(dim, (0, 0), special_flags=pygame.BLEND_RGB_MULT)
    surface.blit(tile, rect.topleft)


def draw_docked_console(
    surface: pygame.Surface,
    small: pygame.font.Font,
    *,
    stardate: str,
    message: str,
    docked: bool = True,
    comms: bool = True,
    compression_index: int = 0,
    paused: bool = False,
    active_icon: int | None = None,
    warn_launch: bool = False,
    view_mode: str = "front",
    view_label: str = "",
    map_mode: str = "galaxy",
    info_page: int = 0,
    engines_off: bool = False,
    gear_down: bool = True,
    hud_labels: bool = True,
    hyperspace_ready: bool = False,
    autopilot: bool = False,
    has_autopilot: bool = False,
    screen: str = "view",
    map_tools: bool = False,
    recess_label: str = "",
    bare_left: bool = False,
    bare_right: bool = False,
    center_mode: str = "scanner",
    has_scanner: bool = False,
    weapon_items: list[tuple[str, bool, int]] | None = None,
) -> ConsoleHits:
    """Blit the Frontier console photograph, then paint stateful icons on it."""
    del active_icon  # icons carry the state; keys are not lit as a group
    w, h = surface.get_size()
    panel = _panel_surface(w)
    ch = panel.get_height()
    top = h - ch 
    hits = ConsoleHits(top=top, height=ch)
    surface.blit(panel, (0, top))

    keys_l, keys_r, vcr = _slot_rects(w, top)
    if paused:
        active_vcr = 0
    else:
        active_vcr = max(1, min(4, compression_index if compression_index >= 1 else 1))
    _VCR_FILES = (
        "pause.png",
        "StarDreamer Normal Speed.png",
        "StarDreamer speed 2.png",
        "StarDreamer speed 3.png",
        "StarDreamer speed 4.png",
    )
    for i, r in enumerate(vcr):
        _blit_vcr(surface, r, _VCR_FILES[i], selected=(i == active_vcr))
        hits.vcr.append(r)

    # F1 shows the view you are in. The other icons show what that key will do.
    on_galaxy = screen == "galaxy"
    on_system = screen == "system"
    view_now = {"front": "F1 Front View.png", "rear": "F1 Rear View.png"}.get(view_mode, "F1 Front View.png")
    if on_system:
        map_name = "F2 galactic Map.png"
    elif on_galaxy:
        map_name = "F2 Map.png"
    else:
        map_name = "F2 galactic Map.png"
    info_names = ("F3 Captain Status.png", "F3 Cargo Hold.png", "F3 Ship Status.png")
    info_name = info_names[info_page % 3]
    if docked:
        flight_name = "F7 Take Off No Clearance.png"
    elif autopilot or has_autopilot:
        flight_name = "F7 Engines On AutoPilot.png"
    else:
        flight_name = "F7 Empty.png"
    if on_system:
        right_names = ("F7 system.png", "F8 system.png", "F9 Empty.png", "F10 system.png")
    elif map_tools:
        right_names = ("F7 Settings.png", "F8 Settings.png", "F9 Settings.png", "F10 Settings.png")
    elif on_galaxy:
        right_names = ("F7 Galatic Map.png", "F8 Galactic Map.png", "F9 Galactic Map.png", "F10 Galactic Map.png")
    else:
        right_names = (
            flight_name,
            "F8 Empty.png",
            "F9 Landing Gear Up.png" if gear_down else "F9 Landing Down.png",
            "F10 Text Off.png" if hud_labels else "F10 Text On.png",
        )
    left_icons = (
        (1, view_now, False),
        (2, map_name, False),
        (3, info_name, False),
        (4, "F4 Comms.png", not comms),
    )
    right_icons = tuple((num, name, False) for num, name in zip((7, 8, 9, 10), right_names))
    for rect, (num, name, muted) in zip(keys_l, left_icons):
        _blit_state_icon(surface, rect, name, None, muted=muted)
        hits.icons.append((num, rect))
        hits.icons_left.append(rect)
    for rect, (num, name, muted) in zip(keys_r, right_icons):
        if not bare_right:
            _blit_state_icon(surface, rect, name, None, muted=muted)
            hits.icons.append((num, rect))
            hits.icons_right.append(rect)
    if on_galaxy or on_system:
        # Same size and gap as F7, sitting in the well immediately to its left.
        f7 = keys_r[0]
        gap = keys_r[1].x - keys_r[0].right
        f6 = pygame.Rect(f7.x - gap - f7.w, f7.y, f7.w, f7.h)
        _blit_state_icon(surface, f6, "F6 system.png" if on_system else "F6 Map.png", None)
        hits.icons.append((6, f6))

    if stardate:
        # Grey recess is rows 6-15. The font has 2 blank rows above the digits.
        surface.blit(small.render(stardate, False, ORANGE), (22, top + 5))

    well = scanner_well_rect(w, h)
    if docked:
        if message:
            from render.fonts import load_frontier

            talk = load_frontier(12)
            max_w = max(80, well.w - 16)
            lines: list[str] = []
            cur = ""
            for word in message.split():
                trial = word if not cur else f"{cur} {word}"
                if talk.size(trial)[0] <= max_w:
                    cur = trial
                else:
                    if cur:
                        lines.append(cur)
                    cur = word
            if cur:
                lines.append(cur)
            line_h = talk.get_height() - 2
            block_h = line_h * min(len(lines), 4)
            y = well.y + max(0, (well.h - block_h) // 2)
            for line in lines[:4]:
                lab = talk.render(line, False, CRT_GREEN)
                surface.blit(lab, (well.centerx - lab.get_width() // 2, y))
                y += line_h
    elif center_mode == "weapons":
        hits.weapons = draw_weapon_panel(surface, small, weapon_items or [])
    elif has_scanner:
        draw_flight_scanner_frame(surface, small, message=message)

    # Two dark grooves left of F7. Source rows on the 2928-wide panel.
    scale = w / 2928.0
    rx = int(2304 * scale)
    upper_y = top + int(34 * scale)
    lower_y = top + int(112 * scale)
    if autopilot:
        mode_label, mode_col = "Autopilot", GREEN
    elif engines_off:
        mode_label, mode_col = "Engines Off", ORANGE
    else:
        mode_label, mode_col = "Manual Control", ORANGE
    if docked:
        upper, upper_col = "Landed", ORANGE
        lower = recess_label or ("Comms Link" if comms else view_label)
    elif recess_label:
        upper, upper_col = mode_label, mode_col
        lower = recess_label
    else:
        upper, upper_col = mode_label, mode_col
        lower = view_label
    lab = small.render(upper, False, upper_col)
    surface.blit(lab, (rx, upper_y))
    hits.flight_mode = pygame.Rect(rx, upper_y, max(lab.get_width(), 70), lab.get_height())
    if lower:
        surface.blit(small.render(lower, False, ORANGE), (rx, lower_y))
    return hits


def hit_console(hits: ConsoleHits, pos: tuple[int, int]) -> tuple[str, int] | None:
    x, y = pos
    if hits.flight_mode is not None and hits.flight_mode.collidepoint(x, y):
        return ("flightmode", 0)
    for i, (_label, r) in enumerate(hits.weapons):
        if r.collidepoint(x, y):
            return ("weapon", i)
    for i, r in enumerate(hits.vcr):
        if r.collidepoint(x, y):
            return ("vcr", i)
    for num, r in hits.icons:
        # Generous hit target — icons are only ~24x18
        if r.inflate(6, 10).collidepoint(x, y):
            return ("icon", num)
    # Legacy fallback
    for i, r in enumerate(hits.icons_left):
        if r.inflate(6, 10).collidepoint(x, y):
            return ("icon", i + 1)
    for i, r in enumerate(hits.icons_right):
        if r.inflate(6, 10).collidepoint(x, y):
            return ("icon", 6 + i)
    # Whole console strip: snap to nearest icon / VCR if click is in the bar
    if hits.top and y >= hits.top:
        best: tuple[str, int] | None = None
        best_d = 40.0
        for i, r in enumerate(hits.vcr):
            d = abs(r.centerx - x) + abs(r.centery - y) * 0.5
            if d < best_d:
                best_d = d
                best = ("vcr", i)
        for num, r in hits.icons:
            d = abs(r.centerx - x) + abs(r.centery - y) * 0.35
            if d < best_d:
                best_d = d
                best = ("icon", num)
        return best
    return None
