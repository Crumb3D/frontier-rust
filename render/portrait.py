"""Pixel merchant portrait for FE2 shipyard left panel."""

from __future__ import annotations

import pygame

# Chunky Amiga-style face — red hat, shades (inspired by FE2 shipyard NPCs)


def draw_merchant_portrait(surface: pygame.Surface, x: int, y: int, size: int = 96) -> None:
    """Draw a low-res portrait into a purple-framed square."""
    s = max(64, size)
    # Scale art from a 32x32 logical grid
    cell = s // 32
    frame = pygame.Rect(x, y, s, s)
    pygame.draw.rect(surface, (0, 0, 0), frame)
    pygame.draw.rect(surface, (168, 72, 200), frame, 1)

    def px(gx: int, gy: int, col: tuple[int, int, int], gw: int = 1, gh: int = 1) -> None:
        pygame.draw.rect(
            surface,
            col,
            (x + gx * cell, y + gy * cell, gw * cell, gh * cell),
        )

    # Background
    px(0, 0, (40, 20, 50), 32, 32)
    # Shoulders / coat
    px(4, 24, (60, 30, 40), 24, 8)
    px(6, 22, (80, 40, 50), 20, 4)
    # Neck
    px(13, 20, (140, 90, 70), 6, 4)
    # Face
    px(10, 10, (160, 100, 75), 12, 12)
    px(11, 11, (175, 115, 85), 10, 10)
    # Jaw shadow
    px(10, 18, (120, 70, 55), 12, 3)
    # Sunglasses
    px(11, 13, (20, 15, 25), 10, 3)
    px(12, 13, (60, 40, 30), 3, 3)
    px(17, 13, (60, 40, 30), 3, 3)
    # Hat brim (wide red)
    px(6, 8, (180, 40, 40), 20, 2)
    px(8, 4, (200, 50, 45), 16, 5)
    px(10, 2, (160, 30, 35), 12, 3)
    # Hat highlight
    px(12, 3, (220, 80, 70), 4, 2)
    # Mouth
    px(14, 19, (90, 40, 40), 4, 1)
