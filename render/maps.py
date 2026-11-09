"""Galactic chart and system orrery."""

from __future__ import annotations

import math

import pygame
import numpy as np

from render.chart_cam import ChartCamera, draw_galactic_chart  # noqa: F401
from render.orrery3d import OrreryCamera, body_world_km, draw_orrery  # noqa: F401
from sim.galaxy import Galaxy, StarSystem
from sim.kepler import position_at, AU_KM

AMBER = (255, 180, 40)
GREEN = (40, 220, 80)
DIM = (60, 50, 20)
WHITE = (220, 220, 220)
CYAN = (80, 200, 220)
RED = (255, 60, 40)
GREY = (140, 140, 150)

def _ship_silhouette(surface: pygame.Surface, cx: int, cy: int, color: tuple[int, int, int]) -> None:
    """Tiny top-down Eagle-style outline for the docked ship."""
    # Nose up
    pts = [
        (cx, cy - 10),
        (cx + 5, cy - 2),
        (cx + 8, cy + 6),
        (cx + 3, cy + 4),
        (cx, cy + 8),
        (cx - 3, cy + 4),
        (cx - 8, cy + 6),
        (cx - 5, cy - 2),
    ]
    pygame.draw.polygon(surface, color, pts, 1)
    pygame.draw.line(surface, color, (cx, cy - 6), (cx, cy + 4), 1)


def _pad_bay(
    surface: pygame.Surface,
    small: pygame.font.Font,
    x: int,
    y: int,
    bw: int,
    bh: int,
    label: str,
    selected: bool,
    occupied: bool,
) -> pygame.Rect:
    """Draw one landing bay with boundary, number, status light."""
    rect = pygame.Rect(x, y, bw, bh)
    # Bay tarmac
    pygame.draw.rect(surface, (55, 58, 52), rect)
    border = AMBER if selected else (140, 130, 70)
    pygame.draw.rect(surface, border, rect, 2 if selected else 1)
    # Inner dashed bay lines
    inset = rect.inflate(-6, -6)
    pygame.draw.rect(surface, (180, 160, 50) if selected else (90, 85, 50), inset, 1)
    # Centre cross marks
    pygame.draw.line(surface, (100, 95, 55), (rect.centerx, inset.top), (rect.centerx, inset.bottom), 1)
    pygame.draw.line(surface, (100, 95, 55), (inset.left, rect.centery), (inset.right, rect.centery), 1)
    # Status light (top-left)
    light = GREEN if occupied else (60, 60, 60)
    if selected and not occupied:
        light = AMBER
    pygame.draw.circle(surface, light, (rect.left + 6, rect.top + 6), 3)
    pygame.draw.circle(surface, WHITE, (rect.left + 6, rect.top + 6), 3, 1)
    # Yellow bay number / label
    lab = small.render(label, True, AMBER if selected else (220, 200, 80))
    surface.blit(lab, (rect.centerx - lab.get_width() // 2, rect.bottom - lab.get_height() - 2))
    # Selection reticle corners
    if selected:
        c = WHITE
        L = 8
        for ox, oy, dx, dy in (
            (rect.left, rect.top, 1, 1),
            (rect.right - 1, rect.top, -1, 1),
            (rect.left, rect.bottom - 1, 1, -1),
            (rect.right - 1, rect.bottom - 1, -1, -1),
        ):
            pygame.draw.line(surface, c, (ox, oy), (ox + dx * L, oy), 2)
            pygame.draw.line(surface, c, (ox, oy), (ox, oy + dy * L), 2)
    if occupied:
        _ship_silhouette(surface, rect.centerx, rect.centery - 2, CYAN if selected else (180, 190, 200))
    return rect


def draw_starport_map(
    surface: pygame.Surface,
    font: pygame.font.Font,
    small: pygame.font.Font,
    port_name: str,
    planet_name: str,
    pads: list[str],
    selected_pad: int,
    credits: int,
) -> None:
    """FE2-style top-down starport pad selector with tarmac, bays, and ship outline."""
    w, h = surface.get_size()
    # Ground / sky split — concrete grey tarmac field
    surface.fill((28, 32, 38))

    # Header (ASCII only — no unicode arrows)
    surface.blit(font.render(port_name, True, AMBER), (12, 8))
    surface.blit(
        small.render(f"Surface facility on {planet_name}    Cr {credits}", True, GREEN),
        (12, 26),
    )
    surface.blit(
        small.render("F4 Comms = Launch Request, then F7 = Launch", True, WHITE),
        (12, 40),
    )

    # Map frame
    mx, my = 16, 56
    mw, mh = w - 32, h - 72
    if mh < 80 or mw < 120:
        return
    # Outer apron
    pygame.draw.rect(surface, (42, 46, 42), (mx, my, mw, mh))
    pygame.draw.rect(surface, (90, 95, 80), (mx, my, mw, mh), 1)

    # Perspective-ish ground grid (tarmac tiles)
    tile = 16
    for gy in range(my, my + mh, tile):
        shade = 48 + ((gy - my) // tile % 2) * 4
        for gx in range(mx, mx + mw, tile):
            if ((gx - mx) // tile + (gy - my) // tile) % 2 == 0:
                pygame.draw.rect(surface, (shade, shade + 2, shade - 2), (gx, gy, tile, tile))
    # Taxiway (horizontal strip through middle)
    taxi_y = my + mh // 2 - 10
    pygame.draw.rect(surface, (60, 62, 55), (mx + 8, taxi_y, mw - 16, 20))
    for lx in range(mx + 16, mx + mw - 16, 18):
        pygame.draw.line(surface, (200, 190, 70), (lx, taxi_y + 10), (lx + 10, taxi_y + 10), 1)
    # Vertical taxi / runway stub
    runway_x = mx + mw // 2 - 12
    pygame.draw.rect(surface, (58, 60, 52), (runway_x, my + 8, 24, mh - 16))
    pygame.draw.line(surface, (210, 200, 80), (runway_x + 12, my + 12), (runway_x + 12, my + mh - 12), 1)

    # Facility buildings (top edge)
    buildings = [
        (mx + 10, my + 8, 50, 28, "TERM"),
        (mx + 70, my + 8, 40, 22, "FUEL"),
        (mx + mw - 100, my + 8, 44, 30, "CTRL"),
        (mx + mw - 48, my + 10, 36, 24, "RADAR"),
    ]
    for bx, by, bw, bh, tag in buildings:
        pygame.draw.rect(surface, (70, 78, 72), (bx, by, bw, bh))
        pygame.draw.rect(surface, (110, 120, 110), (bx, by, bw, bh), 1)
        # Windows
        for wi in range(2):
            pygame.draw.rect(surface, (40, 80, 100), (bx + 6 + wi * 14, by + 6, 8, 6))
        lab = small.render(tag, True, DIM)
        surface.blit(lab, (bx + 4, by + bh - 11))

    # Hangar block (bottom-right)
    hx, hy = mx + mw - 110, my + mh - 55
    pygame.draw.rect(surface, (65, 70, 68), (hx, hy, 95, 42))
    pygame.draw.rect(surface, (120, 125, 115), (hx, hy, 95, 42), 1)
    pygame.draw.rect(surface, (30, 30, 32), (hx + 20, hy + 18, 55, 22))  # hangar door
    surface.blit(small.render("HANGAR", True, GREY), (hx + 28, hy + 4))

    # Pad layout — structured grid, no overlap
    # Row of pads above taxiway, row below; hangar-named pads go near hangar
    n = len(pads)
    selected_pad = max(0, min(selected_pad, n - 1))
    bay_w, bay_h = 56, 40
    gap = 12

    # Classify hangar vs normal pads
    normal_idx = [i for i, p in enumerate(pads) if "hangar" not in p.lower()]
    hangar_idx = [i for i, p in enumerate(pads) if "hangar" in p.lower()]

    positions: dict[int, tuple[int, int]] = {}
    # Top row (above taxiway): first half of normal pads
    top_pads = normal_idx[: (len(normal_idx) + 1) // 2]
    bot_pads = normal_idx[(len(normal_idx) + 1) // 2:]
    top_y = taxi_y - bay_h - 14
    bot_y = taxi_y + 28

    def _row_x(count: int, row_i: int) -> int:
        total = count * bay_w + max(0, count - 1) * gap
        return mx + (mw - total) // 2 + row_i * (bay_w + gap)

    for j, pi in enumerate(top_pads):
        positions[pi] = (_row_x(len(top_pads), j), top_y)
    for j, pi in enumerate(bot_pads):
        positions[pi] = (_row_x(len(bot_pads), j), bot_y)
    # Hangars near hangar building
    for j, pi in enumerate(hangar_idx):
        positions[pi] = (hx + 8 + j * (bay_w + 4), hy - bay_h - 6)

    # Fallback if somehow empty
    if not positions:
        for i in range(n):
            positions[i] = (mx + 20 + (i % 3) * (bay_w + gap), my + 50 + (i // 3) * (bay_h + gap))

    for i, name in enumerate(pads):
        if i not in positions:
            continue
        px, py = positions[i]
        # Short label for bay number
        short = name
        if name.lower().startswith("pad "):
            short = name.split()[-1]
        elif "hangar" in name.lower():
            short = "H"
        occupied = i == selected_pad  # show ship on the currently selected / docked bay
        _pad_bay(surface, small, px, py, bay_w, bay_h, short, i == selected_pad, occupied)
        # Full name under bay if space
        full = small.render(name[:14], True, WHITE if i == selected_pad else DIM)
        surface.blit(full, (px + (bay_w - full.get_width()) // 2, py + bay_h + 1))

    # Selected pad caption
    if 0 <= selected_pad < n:
        caption = small.render(f"Selected: {pads[selected_pad]}", True, AMBER)
        surface.blit(caption, (mx + 8, my + mh - 14))
