"""Attract-mode intro. Frames extracted from assets/Images/intro.mp4 at 25 fps."""

from __future__ import annotations

from pathlib import Path

import pygame

_DIR = Path(__file__).resolve().parent.parent / "assets" / "Images" / "intro_frames"
_FPS = 25.0


class IntroFilm:
    def __init__(self) -> None:
        self.paths = sorted(_DIR.glob("frame_*.png"))
        self.index = 0
        self._acc = 0.0
        self._loaded = -1
        self.image: pygame.Surface | None = None

    def reset(self) -> None:
        self.index = 0
        self._acc = 0.0
        self._loaded = -1

    def advance(self, dt: float) -> bool:
        """Step the film. Returns True when it wraps to the first frame."""
        if not self.paths:
            return False
        self._acc += dt
        step = 1.0 / _FPS
        wrapped = False
        while self._acc >= step:
            self._acc -= step
            nxt = self.index + 1
            if nxt >= len(self.paths):
                nxt = 0
                wrapped = True
            self.index = nxt
        if self._loaded != self.index:
            raw = pygame.image.load(str(self.paths[self.index])).convert()
            self.image = raw
            self._loaded = self.index
        return wrapped

    def draw(self, surface: pygame.Surface) -> None:
        if self.image is None:
            surface.fill((0, 0, 0))
            return
        scaled = pygame.transform.scale(self.image, surface.get_size())
        surface.blit(scaled, (0, 0))
