"""Stardate clock and time compression."""

from __future__ import annotations

from dataclasses import dataclass

# Seconds per game day (Earth day)
SECONDS_PER_DAY = 86400.0

# Compression ladder — FE2 Stardreamer primary tiers first, then finer steps
COMPRESSION_LEVELS = (
    1,
    10,
    100,
    1000,
    10000,
    2,
    4,
    8,
    16,
    32,
    64,
    128,
    256,
    512,
    1024,
    2048,
    4096,
    8192,
    16384,
    32768,
    65536,
    100000,
)

# VCR / Shift+F1..F5 stardreamer tiers
STARDREAMER_FACTORS = (1, 10, 100, 1000, 10000)


@dataclass
class Stardate:
    """Game clock starting 1 January 3200."""

    days: float = 0.0  # fractional days since 3200-01-01 00:00

    def advance(self, real_dt: float, compression: int) -> float:
        """Advance by real_dt seconds of wall time at compression. Return sim seconds."""
        sim_dt = real_dt * float(compression)
        self.days += sim_dt / SECONDS_PER_DAY
        return sim_dt

    @property
    def total_seconds(self) -> float:
        return self.days * SECONDS_PER_DAY

    def format(self) -> str:
        total = int(self.days)
        year = 3200 + total // 365
        rem = total % 365
        month = rem // 30 + 1
        day = rem % 30 + 1
        frac = self.days - total
        hour = int(frac * 24)
        minute = int((frac * 24 - hour) * 60)
        second = int((((frac * 24 - hour) * 60) - minute) * 60)
        months = (
            "Jan", "Feb", "Mar", "Apr", "May", "Jun",
            "Jul", "Aug", "Sep", "Oct", "Nov", "Dec",
        )
        mon = months[min(month, 12) - 1]
        # Amiga FE2 style: "12:03:57 1 Jan 3200"
        return f"{hour}:{minute:02d}:{second:02d} {day} {mon} {year}"


class TimeCompression:
    """Manages compression level with auto-drop near threats/bodies."""

    def __init__(self) -> None:
        self.index = 0
        self.force_realtime = False

    @property
    def factor(self) -> int:
        if self.force_realtime:
            return 1
        return COMPRESSION_LEVELS[self.index]

    @property
    def selected_factor(self) -> int:
        """User-chosen compression, ignoring the momentary realtime lock."""
        return COMPRESSION_LEVELS[self.index]

    def faster(self) -> None:
        if self.index < len(COMPRESSION_LEVELS) - 1:
            self.index += 1

    def slower(self) -> None:
        if self.index > 0:
            self.index -= 1

    def set_index(self, index: int) -> None:
        self.index = max(0, min(index, len(COMPRESSION_LEVELS) - 1))

    def set_vcr(self, vcr_slot: int) -> None:
        """FE2 VCR: 1=>10×  2=>>100×  3=>>>1000×  4=>>>>10000×. Slot 0=pause."""
        factors = {1: 10, 2: 100, 3: 1000, 4: 10000}
        if vcr_slot not in factors:
            return
        target = factors[vcr_slot]
        if target in COMPRESSION_LEVELS:
            self.index = COMPRESSION_LEVELS.index(target)
        else:
            self.index = min(
                range(len(COMPRESSION_LEVELS)),
                key=lambda i: abs(COMPRESSION_LEVELS[i] - target),
            )

    def set_stardreamer(self, tier: int) -> None:
        """Shift+F1..F5 → tiers 0..4 = 1x, 10x, 100x, 1000x, 10000x."""
        tier = max(0, min(4, tier))
        factor = STARDREAMER_FACTORS[tier]
        if factor in COMPRESSION_LEVELS:
            self.index = COMPRESSION_LEVELS.index(factor)

    def vcr_slot(self) -> int:
        """Which VCR play button (1..4) matches the selected factor.

        Uses the chosen speed, not the momentary realtime lock, so the
        button you clicked stays lit.
        """
        f = self.selected_factor
        if f <= 1:
            return 1
        if f <= 10:
            return 1
        if f <= 100:
            return 2
        if f <= 1000:
            return 3
        return 4

    def reset(self) -> None:
        self.index = 0

    def update_auto(self, near_body: bool, in_combat: bool) -> None:
        self.force_realtime = near_body or in_combat
        if self.force_realtime and self.index > 0:
            # Soft-drop toward realtime when forced
            pass
