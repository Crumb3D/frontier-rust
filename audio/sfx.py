"""Simple synthesized SFX (no external sample files required)."""

from __future__ import annotations

import math
import struct

import pygame


def _white(seed: int) -> tuple[int, float]:
    seed = (1664525 * seed + 1013904223) & 0xFFFFFFFF
    return seed, ((seed >> 8) & 0xFFFF) / 32768.0 - 1.0


def _pack(samples: list[float], level: float) -> pygame.mixer.Sound:
    peak = max((abs(sample) for sample in samples), default=1.0) or 1.0
    buf = bytearray()
    for sample in samples:
        v = max(-1.0, min(1.0, sample / peak * level))
        buf += struct.pack("<h", int(v * 32767))
    return pygame.mixer.Sound(buffer=bytes(buf))


def _exhaust(n: int, rate: int, *, spool: bool) -> list[float]:
    """Idling thruster: a low detuned roar, with the hiss filtered out."""
    seed = 0xE4A15
    phase_a = phase_b = 0.0
    brown = 0.0
    body = 0.0
    out: list[float] = []
    for i in range(n):
        t = i / max(1, n - 1)
        base = 42.0 + (55.0 * t if spool else 0.0)
        phase_a += 2.0 * math.pi * base / rate
        phase_b += 2.0 * math.pi * base * 1.04 / rate
        # Odd harmonics make a rough jet note. Detune keeps it from beeping.
        tone = (
            math.sin(phase_a)
            + 0.45 * math.sin(phase_a * 3.0)
            + 0.7 * math.sin(phase_b)
            + 0.25 * math.sin(phase_b * 3.0)
        )
        seed, w = _white(seed)
        brown += (w - brown) * 0.004
        body += (tone * 0.22 + brown * 1.6 - body) * 0.05
        sample = body
        if spool:
            sample *= math.sin(math.pi * t)
        out.append(sample)
    return out


def _seam(samples: list[float], fade: int) -> None:
    n = len(samples)
    for i in range(fade):
        k = i / fade
        mixed = samples[i] * k + samples[n - fade + i] * (1.0 - k)
        samples[i] = mixed
        samples[n - fade + i] = mixed


def _loop(kind: str) -> pygame.mixer.Sound:
    """Looping bed. The ship is exhaust. A station is a quiet room tone."""
    rate = 22050
    n = rate * 2
    if kind == "ground":
        raw = _exhaust(n, rate, spool=False)
        _seam(raw, rate // 8)
        return _pack(raw, 0.42)
    seed = 0x1234
    low = 0.0
    raw = []
    for _ in range(n):
        seed, white = _white(seed)
        low += (white - low) * 0.02
        raw.append(low)
    _seam(raw, rate // 10)
    return _pack(raw, 0.7)


def _tone(freq: float, ms: int, volume: float = 0.35, decay: bool = True) -> pygame.mixer.Sound:
    rate = 22050
    n = int(rate * ms / 1000)
    buf = bytearray()
    for i in range(n):
        t = i / rate
        env = (1.0 - i / n) if decay else 1.0
        sample = int(volume * env * 32767 * math.sin(2 * math.pi * freq * t))
        buf += struct.pack("<h", max(-32767, min(32767, sample)))
    return pygame.mixer.Sound(buffer=bytes(buf))


class SfxPlayer:
    def __init__(self) -> None:
        self.enabled = False
        self._launch: pygame.mixer.Sound | None = None
        self._click: pygame.mixer.Sound | None = None
        try:
            if not pygame.mixer.get_init():
                pygame.mixer.init(frequency=22050, size=-16, channels=1, buffer=512)
            self._launch = _pack(_exhaust(int(22050 * 0.7), 22050, spool=True), 0.55)
            self._launch2 = None
            self._click = _tone(880, 40, 0.2)
            self._ground = _loop("ground")
            self._station = _loop("station")
            self._ambience: pygame.mixer.Channel | None = None
            self._ambience_kind = ""
            self.enabled = True
        except pygame.error:
            self.enabled = False
            self._ambience_kind = ""

    def launch(self) -> None:
        if not self.enabled:
            return
        try:
            if self._launch:
                self._launch.play()
            if getattr(self, "_launch2", None):
                self._launch2.play()
        except pygame.error:
            pass

    def ambience(self, kind: str | None) -> None:
        """Loop ship-idle or station-interior sound. None stops it."""
        if not self.enabled:
            return
        if kind == self._ambience_kind:
            return
        if self._ambience is not None:
            self._ambience.stop()
            self._ambience = None
        self._ambience_kind = kind or ""
        sound = self._station if kind == "station" else self._ground if kind == "ground" else None
        if sound is None:
            return
        try:
            self._ambience = sound.play(loops=-1)
        except pygame.error:
            self._ambience_kind = ""

    def click(self) -> None:
        if not self.enabled or not self._click:
            return
        try:
            self._click.play()
        except pygame.error:
            pass
