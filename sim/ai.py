"""Simple AI pilots sharing the Newtonian flight model."""

from __future__ import annotations

import random

import numpy as np

from sim.combat import Combatant
from sim.flight import FlightControls, FlightModel, ShipStats
from sim.frame import FrameState, FrameSystem


def _make_ship(role: str) -> ShipStats:
    if role == "pirate":
        return ShipStats(
            name="Pirate Eagle",
            mass_t=20,
            thrust_kn=260,
            hull=70,
            shield=30,
            laser_power=10,
            jump_range_ly=6,
            cargo_capacity=2,
        )
    if role == "police":
        return ShipStats(
            name="Police Viper",
            mass_t=45,
            thrust_kn=320,
            hull=140,
            shield=90,
            laser_power=22,
            jump_range_ly=8,
            missiles=2,
            has_missile=True,
        )
    return ShipStats(
        name="Trader Adder",
        mass_t=35,
        thrust_kn=200,
        hull=110,
        shield=50,
        laser_power=8,
        jump_range_ly=9,
        cargo_capacity=12,
    )


class AIController:
    def __init__(self, combatant: Combatant, rng: random.Random | None = None) -> None:
        self.c = combatant
        self.flight = FlightModel(combatant.stats)
        self.rng = rng or random.Random()
        self.cooldown = 0.0

    def update(
        self,
        dt: float,
        player: Combatant,
        frames: FrameSystem,
        days: float,
        fire_laser,
    ) -> None:
        if not self.c.alive:
            return
        self.cooldown = max(0.0, self.cooldown - dt)
        me = frames.ship_world_pos(self.c.state, days)
        them = frames.ship_world_pos(player.state, days)
        to_player = them - me
        dist = float(np.linalg.norm(to_player))
        role = self.c.role

        # Orient roughly toward interest
        if dist > 1e-6:
            from sim.nav import steer_toward
            pitch, yaw, roll, _align = steer_toward(self.c.state.orientation, to_player)
            self.flight.controls.pitch = pitch
            self.flight.controls.yaw = yaw
            self.flight.controls.roll = roll

        if role == "pirate":
            # Hunt: close from scanner range, fire when lined up
            self.flight.controls.set_speed = 12.0 if dist > 8 else 1.5
            align = float(np.dot(self.c.state.orientation[:, 2], to_player / max(dist, 1e-9)))
            if dist < 25 and align > 0.85 and self.cooldown <= 0:
                fire_laser(self.c)
                self.cooldown = 0.55
        elif role == "police":
            chase = (not player.legal) or (not getattr(player, "_legal_flag", True))
            # Use combat world flag via player.legal on combatant — set externally
            hostile = not player.legal if hasattr(player, "legal") else False
            # Always check player legal from combatant field
            hostile = not player.legal
            if hostile:
                self.flight.controls.set_speed = 12.0
                if dist < 20 and self.cooldown <= 0:
                    fire_laser(self.c)
                    self.cooldown = 0.35
            else:
                self.flight.controls.set_speed = 3.0
                # Patrol — gentle orbit
                self.flight.controls.yaw = 0.2
        else:  # trader
            self.flight.controls.set_speed = 4.0
            self.flight.controls.yaw = 0.05
            # Flee pirates if close — handled simply by speeding up near player if player illegal
            if not player.legal and dist < 30:
                self.flight.controls.set_speed = 15.0

        self.c.state = self.flight.step(self.c.state, dt)


def spawn_traffic(
    frames: FrameSystem,
    parent_id: str,
    count: int = 3,
    seed: int = 0,
    near: FrameState | None = None,
    days: float = 0.0,
    roles: list[str] | None = None,
) -> list[Combatant]:
    """Spawn ships in open space near the player, not at the planet centre."""
    rng = random.Random(seed)
    roles = roles or ["pirate", "pirate", "trader", "trader", "police"]
    out: list[Combatant] = []
    parent_pos = frames.world_position(parent_id, days)
    if near is not None:
        origin = frames.ship_world_pos(near, days)
    else:
        origin = parent_pos
    for i in range(count):
        role = roles[i % len(roles)]
        stats = _make_ship(role)
        ang = rng.random() * 6.28318
        # Pirates a bit closer so they can actually reach you
        dist = rng.uniform(18, 45) if role == "pirate" else rng.uniform(30, 90)
        world = origin + np.array(
            [
                float(np.cos(ang)) * dist,
                rng.uniform(-6, 6),
                float(np.sin(ang)) * dist,
            ],
            dtype=np.float64,
        )
        offset = world - parent_pos
        state = FrameState(parent_id=parent_id, position=offset)
        to_player = origin - world
        n = float(np.linalg.norm(to_player))
        if role == "pirate" and n > 1.0:
            fwd = to_player / n
            up = np.array([0.0, 1.0, 0.0])
            right = np.cross(up, fwd)
            rn = float(np.linalg.norm(right))
            if rn < 1e-6:
                right = np.array([1.0, 0.0, 0.0])
            else:
                right = right / rn
            up = np.cross(fwd, right)
            state.orientation = np.column_stack((right, up, fwd))
        else:
            yaw = rng.random() * 6.28
            c, s = np.cos(yaw), np.sin(yaw)
            state.orientation = np.array([[c, 0, s], [0, 1, 0], [-s, 0, c]], dtype=np.float64)
        out.append(
            Combatant(
                id=f"ai_{role}_{i}",
                name=stats.name,
                role=role,
                state=state,
                stats=stats,
                legal=(role != "pirate"),
                bounty=2000 if role == "pirate" else 0,
            )
        )
    return out
