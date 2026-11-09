"""FE2-style hyperspace tunnel and docking-bay approach cinematics."""

from __future__ import annotations

import math
import random

import numpy as np
import pygame

NAVY = (4, 8, 28)
WHITE = (240, 240, 255)
CYAN = (80, 200, 255)
ORANGE = (255, 168, 40)
AMBER = (255, 200, 60)
PURPLE = (120, 60, 180)
GREEN = (48, 230, 72)


def _seed_rng(seed: int) -> random.Random:
    return random.Random(seed & 0xFFFFFFFF)


def draw_hyperspace(
    surface: pygame.Surface,
    t: float,
    duration: float,
    *,
    dest_name: str = "",
    font: pygame.font.Font | None = None,
) -> None:
    """Star-streak tunnel → contracting squares → white flash. t in [0, duration]."""
    w, h = surface.get_size()
    u = max(0.0, min(1.0, t / max(duration, 1e-6)))
    cx, cy = w // 2, h // 2
    surface.fill(NAVY)

    rng = _seed_rng(0xFE2A11)
    # Phase weights
    streak = min(1.0, u * 2.2)
    tunnel = float(np.clip((u - 0.15) / 0.55, 0.0, 1.0))
    flash = float(np.clip((u - 0.78) / 0.22, 0.0, 1.0))

    # Radial star streaks (FE2 hyperspace feel)
    n_streaks = 90
    for i in range(n_streaks):
        ang = rng.random() * math.tau
        # Start radius grows with time so streaks rush outward from centre
        r0 = 4 + rng.random() * 30
        speed = 180 + rng.random() * 420
        r1 = r0 + streak * speed * (0.35 + rng.random())
        r2 = r1 + 8 + streak * 40 * rng.random()
        c = (
            int(180 + 75 * rng.random()),
            int(180 + 75 * rng.random()),
            255,
        )
        x1 = cx + math.cos(ang) * r1
        y1 = cy + math.sin(ang) * r1 * 0.72
        x2 = cx + math.cos(ang) * r2
        y2 = cy + math.sin(ang) * r2 * 0.72
        if r1 < max(w, h):
            pygame.draw.line(surface, c, (int(x1), int(y1)), (int(x2), int(y2)), 1)

    # Contracting targeting-style squares (tunnel)
    if tunnel > 0:
        n_sq = 14
        for i in range(n_sq):
            # Squares rush toward camera
            phase = (i / n_sq + tunnel * 2.8) % 1.0
            scale = 0.04 + phase * phase * 1.35
            half_w = int(w * 0.48 * scale)
            half_h = int(h * 0.42 * scale)
            if half_w < 2 or half_h < 2:
                continue
            alpha_boost = int(40 + 160 * (1.0 - phase))
            col = (min(255, 40 + alpha_boost), min(255, 120 + alpha_boost // 2), 255)
            rect = pygame.Rect(cx - half_w, cy - half_h, half_w * 2, half_h * 2)
            pygame.draw.rect(surface, col, rect, 1)
            # Corner ticks
            tick = max(2, half_w // 10)
            for ox, oy in ((-1, -1), (1, -1), (-1, 1), (1, 1)):
                px = cx + ox * half_w
                py = cy + oy * half_h
                pygame.draw.line(surface, col, (px, py), (px - ox * tick, py), 1)
                pygame.draw.line(surface, col, (px, py), (px, py - oy * tick), 1)

    # Soft purple haze
    if 0.2 < u < 0.85:
        haze = pygame.Surface((w, h), pygame.SRCALPHA)
        a = int(30 + 50 * math.sin(u * math.pi))
        haze.fill((90, 40, 140, a))
        surface.blit(haze, (0, 0))

    # Arrival flash
    if flash > 0:
        wh = pygame.Surface((w, h), pygame.SRCALPHA)
        a = int(255 * (flash if flash < 0.5 else (1.0 - flash) * 2))
        wh.fill((255, 255, 255, max(0, min(255, a))))
        surface.blit(wh, (0, 0))

    if font and dest_name and u < 0.85:
        lab = font.render(f"HYPERSPACE — {dest_name}", True, AMBER)
        surface.blit(lab, (cx - lab.get_width() // 2, 18))


def draw_docking(
    surface: pygame.Surface,
    t: float,
    duration: float,
    *,
    station_name: str = "",
    is_station: bool = True,
    font: pygame.font.Font | None = None,
) -> None:
    """Approach through airlock / pad descent into docking bay."""
    w, h = surface.get_size()
    u = max(0.0, min(1.0, t / max(duration, 1e-6)))
    cx, cy = w // 2, int(h * 0.42)
    surface.fill((8, 10, 18))

    if is_station:
        # Rotating station corridor — concentric octagonal frames rushing past
        spin = u * 1.8
        n_rings = 18
        for i in range(n_rings):
            phase = (i / n_rings + u * 1.6) % 1.0
            scale = 0.08 + phase * phase * 1.6
            rad = int(min(w, h) * 0.55 * scale)
            if rad < 6:
                continue
            col_v = int(40 + 180 * (1.0 - phase))
            col = (col_v, col_v // 2, 30 + col_v // 3)
            pts = []
            sides = 8
            for s in range(sides):
                ang = spin + s * math.tau / sides + phase * 0.4
                pts.append((cx + int(math.cos(ang) * rad), cy + int(math.sin(ang) * rad * 0.85)))
            pygame.draw.polygon(surface, col, pts, 2)
            # Inner guide lights
            if i % 2 == 0 and phase < 0.7:
                for s in range(sides):
                    ang = spin + s * math.tau / sides + 0.2
                    lx = cx + int(math.cos(ang) * rad * 0.92)
                    ly = cy + int(math.sin(ang) * rad * 0.78)
                    pygame.draw.circle(surface, ORANGE if (i + s) % 3 else WHITE, (lx, ly), 2)

        # Docking bay doors parting near end
        if u > 0.55:
            door = float(np.clip((u - 0.55) / 0.35, 0.0, 1.0))
            gap = int(h * 0.35 * door)
            pygame.draw.rect(surface, (20, 22, 30), (0, 0, w, cy - gap // 2))
            pygame.draw.rect(surface, (20, 22, 30), (0, cy + gap // 2, w, h))
            # Bay interior glow
            if door > 0.3:
                glow = pygame.Surface((w, gap), pygame.SRCALPHA)
                glow.fill((255, 180, 60, int(40 + 80 * door)))
                surface.blit(glow, (0, cy - gap // 2))
                # Pad chevrons
                for k in range(5):
                    yy = cy - gap // 2 + 8 + k * max(6, gap // 6)
                    pygame.draw.polygon(
                        surface,
                        ORANGE,
                        [(cx - 12, yy), (cx + 12, yy), (cx, yy + 8)],
                        1,
                    )
    else:
        # Surface starport descent — blue haze, pad rising
        haze = pygame.Surface((w, h), pygame.SRCALPHA)
        haze.fill((40, 90, 160, int(30 + 100 * u)))
        surface.blit(haze, (0, 0))
        # Horizon
        hy = int(h * (0.35 + 0.4 * u))
        pygame.draw.rect(surface, (30, 50, 40), (0, hy, w, h - hy))
        # Pad
        pad_w = int(40 + 200 * u)
        pad_h = int(8 + 40 * u)
        pygame.draw.ellipse(surface, (60, 60, 70), (cx - pad_w // 2, hy - pad_h // 2, pad_w, pad_h))
        pygame.draw.ellipse(surface, ORANGE, (cx - pad_w // 2, hy - pad_h // 2, pad_w, pad_h), 1)
        # Approach lights
        for side in (-1, 1):
            for k in range(6):
                lx = cx + side * int(30 + k * 18 * (1.2 - u * 0.5))
                ly = hy - int(20 + k * 12 * (1.0 - u))
                pygame.draw.circle(surface, ORANGE if k % 2 == 0 else WHITE, (lx, ly), 2)

    # Fade to docked UI
    if u > 0.85:
        fade = pygame.Surface((w, h), pygame.SRCALPHA)
        fade.fill((0, 0, 0, int(255 * (u - 0.85) / 0.15)))
        surface.blit(fade, (0, 0))

    if font and station_name:
        lab = font.render(
            ("DOCKING — " if is_station else "LANDING — ") + station_name,
            True,
            GREEN,
        )
        surface.blit(lab, (w // 2 - lab.get_width() // 2, h - 36))
