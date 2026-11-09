"""Hierarchical reference frames and origin rebasing."""

from __future__ import annotations

from dataclasses import dataclass, field
from typing import Callable

import numpy as np


def _identity() -> np.ndarray:
    return np.eye(3, dtype=np.float64)


@dataclass
class BodyRef:
    """A body that can be a coordinate parent."""

    id: str
    name: str
    radius_km: float
    # World position callback: days -> km (system barycentric)
    world_pos: Callable[[float], np.ndarray]
    parent_id: str | None = None
    is_station: bool = False
    is_starport: bool = False
    atmosphere_km: float = 0.0
    spin_period_days: float = 0.0
    spin_phase: float = 0.0


@dataclass
class FrameState:
    """Ship/camera state relative to a parent body."""

    parent_id: str
    position: np.ndarray = field(default_factory=lambda: np.zeros(3, dtype=np.float64))
    velocity: np.ndarray = field(default_factory=lambda: np.zeros(3, dtype=np.float64))  # km/s
    orientation: np.ndarray = field(default_factory=_identity)  # body->parent rotation
    angular_velocity: np.ndarray = field(default_factory=lambda: np.zeros(3, dtype=np.float64))  # rad/s body

    def copy(self) -> "FrameState":
        return FrameState(
            parent_id=self.parent_id,
            position=self.position.copy(),
            velocity=self.velocity.copy(),
            orientation=self.orientation.copy(),
            angular_velocity=self.angular_velocity.copy(),
        )


class FrameSystem:
    """Tracks bodies and rebases ship frames when leaving a parent sphere."""

    def __init__(self) -> None:
        self.bodies: dict[str, BodyRef] = {}

    def add(self, body: BodyRef) -> None:
        self.bodies[body.id] = body

    def world_position(self, body_id: str, days: float) -> np.ndarray:
        return self.bodies[body_id].world_pos(days)

    def ship_world_pos(self, state: FrameState, days: float) -> np.ndarray:
        parent = self.bodies[state.parent_id]
        return parent.world_pos(days) + state.position

    def ship_world_vel(self, state: FrameState, days: float) -> np.ndarray:
        # Approximate: parent velocity via finite difference + relative
        parent = self.bodies[state.parent_id]
        p0 = parent.world_pos(days)
        p1 = parent.world_pos(days + 1e-6)
        parent_vel = (p1 - p0) / (1e-6 * 86400.0)
        return parent_vel + state.velocity

    def rebase(self, state: FrameState, days: float, new_parent_id: str) -> FrameState:
        """Change parent without changing world position/velocity."""
        if new_parent_id == state.parent_id:
            return state
        world_pos = self.ship_world_pos(state, days)
        world_vel = self.ship_world_vel(state, days)
        new_parent = self.bodies[new_parent_id]
        new_pos = world_pos - new_parent.world_pos(days)
        p0 = new_parent.world_pos(days)
        p1 = new_parent.world_pos(days + 1e-6)
        parent_vel = (p1 - p0) / (1e-6 * 86400.0)
        new_vel = world_vel - parent_vel
        out = state.copy()
        out.parent_id = new_parent_id
        out.position = new_pos
        out.velocity = new_vel
        return out

    def auto_rebase(self, state: FrameState, days: float) -> FrameState:
        """Pick the nearest suitable parent (prefer stations/planets over star)."""
        world = self.ship_world_pos(state, days)
        best_id = state.parent_id
        best_score = float("inf")
        for bid, body in self.bodies.items():
            dist = float(np.linalg.norm(world - body.world_pos(days)))
            # Capture sphere: stations tighter, planets ~10x radius, star huge
            if body.is_station:
                sphere = max(body.radius_km * 20.0, 50.0)
            elif body.is_starport:
                sphere = body.radius_km + max(body.atmosphere_km, 100.0)
            else:
                sphere = body.radius_km * 15.0
            if dist < sphere:
                # Prefer closer / smaller bodies
                score = dist / max(body.radius_km, 1.0)
                if body.is_station:
                    score *= 0.1
                if score < best_score:
                    best_score = score
                    best_id = bid
        if best_id != state.parent_id:
            return self.rebase(state, days, best_id)
        return state

    def altitude(self, state: FrameState, days: float) -> float:
        """Altitude above parent surface (km)."""
        body = self.bodies[state.parent_id]
        return float(np.linalg.norm(state.position)) - body.radius_km
