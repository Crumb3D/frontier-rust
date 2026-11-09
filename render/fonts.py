"""Load the authentic Frontier typeface shipped with the project."""

from __future__ import annotations

from pathlib import Path

import pygame

FONT_PATH = Path(__file__).resolve().parent.parent / "assets" / "Frontier.ttf"
_SYSTEM = Path.home() / "AppData" / "Local" / "Microsoft" / "Windows" / "Fonts" / "Frontier.ttf"


class FrontierFont(pygame.font.Font):
    """pygame Font that always renders crisp (no antialias) — Amiga FE2 look."""

    def render(self, text, antialias, color, background=None):  # noqa: ARG002
        if background is None:
            return super().render(text, False, color)
        return super().render(text, False, color, background)


def load_frontier(size: int) -> pygame.font.Font:
    """Return the Frontier typeface at ``size`` (pixels / points)."""
    for path in (FONT_PATH, _SYSTEM):
        if path.is_file():
            return FrontierFont(str(path), size)
    try:
        # SysFont returns a Font; wrap via file if possible
        f = pygame.font.SysFont("Frontier", size)
        return f
    except Exception:
        return pygame.font.SysFont("couriernew", size)
