"""Frontier: Elite II title and career-start screens. 640x400, Amiga presentation."""

from __future__ import annotations

import math
import random

import pygame

NAVY = (6, 8, 22)
WHITE = (232, 232, 228)
AMBER = (232, 196, 96)
GREY = (140, 144, 150)
DIM = (72, 76, 84)
CYAN = (120, 168, 176)
LINE = (70, 90, 110)

_STARS: list[tuple[float, float, float, int]] | None = None


def _starfield() -> list[tuple[float, float, float, int]]:
    global _STARS
    if _STARS is None:
        rng = random.Random(154)
        _STARS = [
            (rng.random(), rng.random() * 0.92, 0.004 + rng.random() * 0.012, rng.choice((90, 140, 180, 220)))
            for _ in range(90)
        ]
    return _STARS


def _title_ship(surface: pygame.Surface, t: float) -> None:
    """Slow courier pass behind the title. Wire outline, short engine trail."""
    w, h = surface.get_size()
    ang = t * 0.35
    cx = int(w * 0.5 + math.cos(ang) * w * 0.38)
    cy = int(h * 0.22 + math.sin(ang * 2) * 18)
    face = ang + math.pi / 2
    c, s = math.cos(face), math.sin(face)

    def pt(x: float, y: float) -> tuple[int, int]:
        return (int(cx + x * c - y * s), int(cy + x * s + y * c))

    hull = [pt(18, 0), pt(6, 5), pt(-10, 4), pt(-14, 0), pt(-10, -4), pt(6, -5)]
    pygame.draw.polygon(surface, (150, 156, 168), hull)
    pygame.draw.polygon(surface, (210, 214, 220), hull, 1)
    pygame.draw.line(surface, (90, 110, 140), pt(4, -2), pt(10, 0), 1)
    pygame.draw.line(surface, (90, 110, 140), pt(4, 2), pt(10, 0), 1)
    for i in range(1, 10):
        fade = 1.0 - i / 10
        col = (int(30 * fade), int(90 * fade), int(180 * fade))
        pygame.draw.line(surface, col, pt(-14 - i * 3, 1), pt(-14 - i * 3, -1))


def _draw_space(surface: pygame.Surface, t: float, ship: bool = False) -> None:
    w, h = surface.get_size()
    surface.fill(NAVY)
    for x, y, spd, bri in _starfield():
        px = int((x + t * spd) % 1.0 * w)
        py = int(y * h)
        col = (bri, bri, min(255, bri + 12))
        surface.set_at((px, py), col)
        if bri > 180:
            surface.set_at((px + 1, py), (bri // 3, bri // 3, bri // 3))
    sy = int((t * 18) % h)
    pygame.draw.line(surface, (24, 36, 52), (0, sy), (w, sy))
    if ship:
        _title_ship(surface, t)


def _fade_col(col: tuple[int, int, int], a: float) -> tuple[int, int, int]:
    a = max(0.0, min(1.0, a))
    return (int(col[0] * a), int(col[1] * a), int(col[2] * a))


def _draw_intro_backdrop(surface: pygame.Surface, t: float) -> None:
    """Title is its own screen. The galactic chart is not the menu."""
    _draw_space(surface, t, ship=True)


def draw_title(
    surface: pygame.Surface,
    font_title: pygame.font.Font,
    font: pygame.font.Font,
    small: pygame.font.Font,
    t: float,
    intro: float,
    menu: bool,
    selected: int,
    load_ok: bool,
) -> list[pygame.Rect]:
    """Title. intro is 0..1 reveal. menu True once the choices are live."""
    w, h = surface.get_size()
    _draw_intro_backdrop(surface, t)
    reveal = 1.0 if menu else max(0.0, min(1.0, intro))
    title_a = max(0.0, min(1.0, (reveal - 0.05) / 0.45))
    sub_a = max(0.0, min(1.0, (reveal - 0.35) / 0.4))

    title = font_title.render("FRONTIER", False, _fade_col(WHITE, title_a))
    sub = font.render("ELITE II", False, _fade_col(AMBER, sub_a))
    tx = w // 2 - title.get_width() // 2
    ty = h // 2 - 70 - int((1.0 - title_a) * 6)
    surface.blit(title, (tx, ty))
    surface.blit(sub, (w // 2 - sub.get_width() // 2, ty + title.get_height() + 2))

    rule_w = int((title.get_width() + 24) * sub_a)
    if rule_w > 2:
        pygame.draw.line(
            surface,
            _fade_col(LINE, sub_a),
            (w // 2 - rule_w // 2, ty + title.get_height() + 20),
            (w // 2 + rule_w // 2, ty + title.get_height() + 20),
        )

    if sub_a > 0.4:
        read = small.render("STARDATE  1 JAN 3200", False, DIM)
        surface.blit(read, (w // 2 - read.get_width() // 2, h - 28))

    rects: list[pygame.Rect] = []
    if not menu:
        return rects

    options = ["START NEW GAME", "LOAD GAME", "QUIT"]
    base_y = ty + title.get_height() + 36
    for i, label in enumerate(options):
        y = base_y + i * 22
        enabled = load_ok or i != 1
        text = label if enabled else "LOAD GAME"
        if i == selected and enabled:
            col = AMBER
        elif enabled:
            col = GREY
        else:
            col = DIM
        surf = font.render(text, False, col)
        x = w // 2 - 78
        rect = pygame.Rect(x - 8, y - 2, 180, 18)
        rects.append(rect)
        if i == selected and enabled:
            pygame.draw.rect(surface, LINE, rect, 1)
            pygame.draw.rect(surface, AMBER, (x - 6, y + 4, 4, 4))
        surface.blit(surf, (x + 6, y))
        if i == 1 and not load_ok:
            note = small.render("NO SAVED POSITION", False, DIM)
            surface.blit(note, (x + surf.get_width() + 14, y + 2))
    return rects


def career_facts(system, dock_id: str | None) -> dict[str, str]:
    """Facts already on the system. Empty moon when the berth is on a planet."""
    planet = "—"
    moon = "—"
    port = "—"
    chosen = dock_id
    for body in system.planets:
        for sp in body.starports:
            if chosen is None:
                chosen = sp.id
            if sp.id != chosen:
                continue
            port = sp.name
            if body.parent_id:
                parent = system.planet_by_id(body.parent_id)
                planet = parent.name if parent else "—"
                moon = body.name
            else:
                planet = body.name
                moon = "—"
            return {
                "system": system.name,
                "planet": planet,
                "moon": moon,
                "starport": port,
            }
    for st in system.stations:
        if chosen is None or st.id == chosen:
            parent = system.planet_by_id(st.parent_planet_id)
            return {
                "system": system.name,
                "planet": parent.name if parent else "—",
                "moon": "—",
                "starport": st.name,
            }
    return {"system": system.name, "planet": planet, "moon": moon, "starport": port}


def _bevel_button(surface: pygame.Surface, x: int, y: int, pressed: bool = False) -> None:
    """Raised button. Pressed swaps the light and dark edges."""
    size = 9
    light = (214, 214, 220)
    dark = (0, 0, 0)
    pygame.draw.rect(surface, (85, 68, 153), (x, y, size, size))
    top_left, bottom_right = (dark, light) if pressed else (light, dark)
    pygame.draw.line(surface, top_left, (x, y), (x + size - 1, y))
    pygame.draw.line(surface, top_left, (x, y), (x, y + size - 1))
    pygame.draw.line(surface, bottom_right, (x + size - 1, y), (x + size - 1, y + size - 1))
    pygame.draw.line(surface, bottom_right, (x, y + size - 1), (x + size - 1, y + size - 1))


def draw_start(
    surface: pygame.Surface,
    font: pygame.font.Font,
    small: pygame.font.Font,
    t: float,
    selected: int,
    *,
    load_ok: bool = False,
) -> list[pygame.Rect]:
    """Position list drawn on top of whatever is already on the screen."""
    w, h = surface.get_size()
    del t
    lines = (
        "1. Ross 154 System (Recommended Start Position)",
        "2. Mars in the Sol System",
        "3. Lave System",
        "4. Last Saved Position",
        "5. Select Position From disc",
    )
    rendered = [font.render(line, False, WHITE) for line in lines]
    block_w = max(lab.get_width() for lab in rendered) + 22
    row_h = 26
    block_h = 28 + len(lines) * row_h
    left = w // 2 - block_w // 2
    top = h // 2 - block_h // 2
    head = font.render("Select Start Position:", False, WHITE)
    surface.blit(head, (left, top))
    rects: list[pygame.Rect] = []
    for i, lab in enumerate(rendered):
        y = top + 30 + i * row_h
        row = pygame.Rect(left - 8, y - 2, block_w + 16, 22)
        rects.append(row)
        _bevel_button(surface, left, y + 2)
        enabled = i < 3 or load_ok
        col = WHITE if i == selected and enabled else (GREY if enabled else DIM)
        surface.blit(font.render(lines[i], False, col), (left + 18, y))
    copy = small.render("Copyright © D.J.Braben 1993, All rights reserved.", False, WHITE)
    surface.blit(copy, (w // 2 - copy.get_width() // 2, h - 28))
    return rects
