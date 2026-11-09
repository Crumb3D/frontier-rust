"""Surface starport / city approach rendering."""

from __future__ import annotations

import math

import numpy as np
import pygame


def draw_surface_approach(
    surface: pygame.Surface,
    altitude_km: float,
    speed: float,
    pad_names: list[str] | None = None,
) -> None:
    """Simple flat-shaded ground + pads when very low over a starport planet."""
    if altitude_km > 80 or altitude_km < -1:
        return
    w, h = surface.get_size()
    hud_h = 90
    view_h = h - hud_h
    # Horizon rises as you get lower
    horizon = int(view_h * (0.35 + 0.45 * max(0, min(1, 1 - altitude_km / 80))))
    # Sky already drawn; paint ground
    ground = pygame.Surface((w, view_h - horizon), pygame.SRCALPHA)
    # Terrain bands
    base = (35, 55, 40) if altitude_km < 40 else (30, 45, 55)
    ground.fill((*base, 230))
    # Perspective grid
    for i in range(1, 12):
        y = int((i / 12) ** 1.5 * (view_h - horizon))
        shade = 40 + i * 4
        pygame.draw.line(ground, (shade, shade + 10, shade), (0, y), (w, y), 1)
    for i in range(-8, 9):
        x0 = w // 2 + i * 40
        pygame.draw.line(ground, (50, 65, 55), (x0, 0), (w // 2 + i * 180, view_h - horizon), 1)

    # City / pads near center when low
    if altitude_km < 25:
        cx = w // 2
        cy = int((view_h - horizon) * 0.55)
        # Buildings
        for dx, bw, bh, col in [
            (-120, 28, 50, (55, 70, 65)),
            (-80, 22, 70, (60, 75, 70)),
            (-40, 30, 40, (50, 65, 60)),
            (50, 26, 55, (58, 72, 68)),
            (95, 35, 45, (52, 68, 62)),
            (140, 20, 65, (48, 62, 58)),
        ]:
            scale = max(0.4, 1.0 - altitude_km / 30)
            bw2, bh2 = int(bw * scale * 2), int(bh * scale * 2)
            pygame.draw.rect(ground, col, (cx + int(dx * scale) - bw2 // 2, cy - bh2, bw2, bh2))
            pygame.draw.rect(ground, (80, 100, 90), (cx + int(dx * scale) - bw2 // 2, cy - bh2, bw2, bh2), 1)

        # Landing pads
        pads = pad_names or ["Pad 1", "Pad 2", "Pad 3", "Pad 4"]
        for i, name in enumerate(pads[:6]):
            px = cx - 90 + (i % 3) * 90
            pz = cy + 20 + (i // 3) * 40
            pygame.draw.rect(ground, (200, 180, 60), (px, pz, 36, 18), 1)
            pygame.draw.line(ground, (220, 200, 80), (px + 4, pz + 9), (px + 32, pz + 9), 1)

    surface.blit(ground, (0, horizon))
    # Horizon line
    pygame.draw.line(surface, (90, 120, 100), (0, horizon), (w, horizon), 1)
    if altitude_km < 5:
        tip = pygame.font.SysFont("consolas", 14).render(
            "STARPORT BELOW — slow down and press L to land",
            True,
            (255, 180, 40),
        )
        surface.blit(tip, (w // 2 - tip.get_width() // 2, horizon - 24))
