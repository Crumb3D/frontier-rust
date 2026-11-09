"""FE2 classical soundtrack from assets/music."""

from __future__ import annotations

from pathlib import Path

import pygame

MUSIC_DIR = Path(__file__).resolve().parent.parent / "assets" / "music"

TRACKS = {
    "intro": "01. Intro.mp3",
    "intro2": "02. Intro 2.mp3",
    "docked": "03. Hall King.mp3",
    "baba": "04. Baba Yaga.mp3",
    "bare": "05. Bare M.mp3",
    "flight": "06. Valkries.mp3",
    "space": "07. Blue Dan.mp3",
    "combat": "08. G G of Kiev.mp3",
}

MODE_TRACK = {
    "title": "intro",
    "start": "intro",
    "docked": "docked",
    "flight": "space",
    "chart": "bare",
    "orrery": "bare",
}


class MusicPlayer:
    def __init__(self) -> None:
        self.enabled = False
        self.current: str | None = None
        self._volume = 0.55
        try:
            if not pygame.mixer.get_init():
                pygame.mixer.init(frequency=44100, size=-16, channels=2, buffer=1024)
            self.enabled = True
            pygame.mixer.music.set_volume(self._volume)
        except pygame.error:
            self.enabled = False

    def _path(self, key: str) -> Path | None:
        name = TRACKS.get(key)
        if not name:
            return None
        p = MUSIC_DIR / name
        return p if p.is_file() else None

    def play(self, key: str, *, loop: bool = True, restart: bool = False) -> None:
        if not self.enabled or (key == self.current and not restart):
            return
        if restart:
            self.current = None
        path = self._path(key)
        if path is None:
            return
        try:
            pygame.mixer.music.load(str(path))
            pygame.mixer.music.play(-1 if loop else 0)
            self.current = key
        except pygame.error:
            pass

    def set_mode(self, mode: str, allowed: set[str] | None = None, *, continuous: bool = False) -> None:
        if not self.enabled:
            return
        if mode in ("paused", "settings"):
            if pygame.mixer.music.get_busy():
                pygame.mixer.music.pause()
            return
        try:
            pygame.mixer.music.unpause()
        except pygame.error:
            pass
        key = MODE_TRACK.get(mode)
        if allowed is not None:
            if key not in allowed:
                key = next((k for k in TRACKS if k in allowed), None)
            if continuous and self.current in allowed:
                key = self.current
        if key:
            self.play(key)
        elif allowed is not None:
            self.stop()

    def stop(self) -> None:
        if self.enabled:
            pygame.mixer.music.stop()
            self.current = None
