"""Load DOS Frontier .GRF pictures and crop console icons from them.

PIC000–PIC002 are 320×200 VGA images: 768-byte 6-bit palette + PackBits pixels.
They are the demo stills the DOS title cycles. Icons are cropped from the
cockpit strip already baked into those stills.

EL2G.GRF is the larger UI graphics bank. Its stream is not PackBits (and is not
documented in fe2-intro), so icons are taken from PIC000 instead of inventing
shapes.

Layout constants below come from watsonmw/fe2-intro (render.h / fintro.c):
  320×168 3D view, bottom 32 rows for cockpit / Frontier logo.
"""

from __future__ import annotations

from pathlib import Path

import pygame

# fe2-intro render.h — original Amiga / DOS logical layout
VIEW_W = 320
VIEW_H = 168
CONSOLE_H = 32  # bottom strip (logo in the intro, cockpit in play)
SCREEN_H = VIEW_H + CONSOLE_H  # 200

# Intro_SceneList from fe2-intro src/fintro.c
# (frame_end, frame_start, duration_centiseconds, label)
# duration is in 1/100 s as in Intro_GetFrameOffsetAtTime
INTRO_SCENES: list[tuple[int, int, int, str]] = [
    (0x200, 0x000, 768, "Spinny Intro Text"),
    (0x400, 0x000, 2100, "Animate Logo"),
    (0x100, 0x000, 384, "Courier flyby"),
    (0x200, 0x000, 768, "Courier rotate 180"),
    (0x400, 0x000, 768, "Courier dive"),
    (0x400, 0x026, 1920, "Courier land"),
    (0x200, 0x000, 384, "Bridge & power station"),
    (0x200, 0x000, 384, "City scene 1"),
    (0x200, 0x000, 384, "City scene 2"),
    (0x200, 0x000, 384, "City scene 3"),
    (0x200, 0x001, 1150, "Eagles dive"),
    (0x1C0, 0x070, 576, "Eagles planet attack"),
    (0x200, 0x040, 1536, "Courier climb"),
    (0x400, 0x000, 768, "Courier escape"),
    (0x400, 0x000, 768, "Eagles space attack"),
    (0x400, 0x000, 768, "Courier hyper arrive"),
    (0x400, 0x000, 960, "Station flyby"),
    (0x200, 0x000, 768, "Eagles hyper arrive"),
    (0x200, 0x000, 384, "Eagles attack over ship"),
    (0x1FE, 0x000, 1152, "Courier attack eagle 1"),
    (0x200, 0x000, 576, "Courier attack eagle 2"),
    (0x400, 0x001, 2208, "Courier kill eagle 2"),
    (0x800, 0x400, 1728, "Rotate courier"),
]

# Map each intro shot to one of the three DOS stills we can decode.
# 3D model shots reuse the closest still rather than inventing hulls.
_SCENE_STILL = (
    0, 0,  # text / logo
    0, 0, 0, 2,  # courier → surface still
    2, 2, 2, 2,  # city / land
    1, 1,  # eagles / attack
    2, 0, 1, 0,  # climb / escape / space
    0, 1, 1, 1, 1, 1, 0,  # station / combat / rotate
)

# Icon cells measured off PIC000 cockpit strip (320×200).
_ICON_Y = 186
_ICON_WH = (16, 14)
_LEFT_X = (0, 16, 32, 48)
_RIGHT_X = (244, 260, 276, 292)


def _default_frontier_dir() -> Path:
    return Path(__file__).resolve().parents[1] / "Frontier - PC ORIGINAL FOR REFERENCE" / "frontier"


def packbits_decode(data: bytes, limit: int) -> bytes:
    """Apple PackBits / TIFF PackBits (same scheme the PIC files use)."""
    out = bytearray()
    i = 0
    n = len(data)
    while i < n and len(out) < limit:
        ctrl = data[i]
        i += 1
        if ctrl <= 127:
            cnt = ctrl + 1
            out.extend(data[i : i + cnt])
            i += cnt
        elif ctrl == 128:
            continue
        else:
            cnt = 257 - ctrl
            if i >= n:
                break
            out.extend([data[i]] * cnt)
            i += 1
    return bytes(out[:limit])


def load_pic_grf(path: Path | str) -> pygame.Surface:
    """Decode a 320×200 PIC*.GRF into a pygame surface."""
    raw = Path(path).read_bytes()
    if len(raw) < 768 + 16:
        raise ValueError(f"GRF too small: {path}")
    pal = [(raw[i * 3] * 4, raw[i * 3 + 1] * 4, raw[i * 3 + 2] * 4) for i in range(256)]
    pixels = packbits_decode(raw[768:], VIEW_W * SCREEN_H)
    if len(pixels) < VIEW_W * SCREEN_H:
        pixels = pixels + bytes(VIEW_W * SCREEN_H - len(pixels))
    surf = pygame.Surface((VIEW_W, SCREEN_H))
    # Build via temporary 8-bit surface for speed
    idx = pygame.image.frombuffer(pixels, (VIEW_W, SCREEN_H), "P")
    idx.set_palette(pal)
    surf.blit(idx.convert(), (0, 0))
    return surf


def find_frontier_dir() -> Path | None:
    d = _default_frontier_dir()
    if (d / "PIC000.GRF").is_file():
        return d
    return None


class FrontierGraphics:
    """Cached PIC stills and F-key icons from the local DOS reference folder."""

    def __init__(self, root: Path | None = None) -> None:
        self.root = root or find_frontier_dir()
        self.pics: list[pygame.Surface] = []
        self.icons: dict[int, pygame.Surface] = {}
        self.ok = False
        if self.root is not None:
            self._load()

    def _load(self) -> None:
        assert self.root is not None
        for name in ("PIC000.GRF", "PIC001.GRF", "PIC002.GRF"):
            path = self.root / name
            if not path.is_file():
                return
            self.pics.append(load_pic_grf(path))
        if not self.pics:
            return
        src = self.pics[0]
        iw, ih = _ICON_WH
        for num, x in zip((1, 2, 3, 4), _LEFT_X):
            self.icons[num] = src.subsurface((x, _ICON_Y, iw, ih)).copy()
        for num, x in zip((7, 8, 9, 10), _RIGHT_X):
            if x + iw <= src.get_width():
                self.icons[num] = src.subsurface((x, _ICON_Y, iw, ih)).copy()
        self.ok = True

    def still_for_scene(self, scene: int) -> pygame.Surface | None:
        if not self.pics:
            return None
        idx = _SCENE_STILL[scene] if 0 <= scene < len(_SCENE_STILL) else scene % len(self.pics)
        return self.pics[idx % len(self.pics)]


def intro_scene_at_time(t_seconds: float) -> tuple[int, str, float]:
    """Return (scene_index, label, local_01) for fe2-intro timing.

    Durations in INTRO_SCENES are centiseconds (1/100 s), matching fintro.c.
    """
    if t_seconds < 0:
        t_seconds = 0.0
    cs = int(t_seconds * 100.0)
    total = sum(d for _e, _s, d, _n in INTRO_SCENES)
    if total <= 0:
        return 0, INTRO_SCENES[0][3], 0.0
    cs %= total
    for i, (_end, _start, dur, name) in enumerate(INTRO_SCENES):
        if cs < dur:
            return i, name, cs / max(dur, 1)
        cs -= dur
    return len(INTRO_SCENES) - 1, INTRO_SCENES[-1][3], 1.0


def intro_total_seconds() -> float:
    return sum(d for _e, _s, d, _n in INTRO_SCENES) / 100.0


_GRAPHICS: FrontierGraphics | None = None


def get_graphics() -> FrontierGraphics:
    global _GRAPHICS
    if _GRAPHICS is None:
        _GRAPHICS = FrontierGraphics()
    return _GRAPHICS
