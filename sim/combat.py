"""Combat: lasers, missiles, damage, legal status."""

from __future__ import annotations

from dataclasses import dataclass, field

import numpy as np

from sim.flight import ShipStats
from sim.frame import FrameState


@dataclass
class Projectile:
    kind: str  # laser | missile
    position: np.ndarray
    velocity: np.ndarray
    owner_id: str
    damage: float
    life: float
    target_id: str | None = None


@dataclass
class Combatant:
    id: str
    name: str
    role: str  # player | trader | pirate | police
    state: FrameState
    stats: ShipStats
    legal: bool = True
    bounty: int = 0
    alive: bool = True


@dataclass
class CombatWorld:
    combatants: dict[str, Combatant] = field(default_factory=dict)
    projectiles: list[Projectile] = field(default_factory=list)
    player_legal: bool = True
    fines: int = 0
    kills: int = 0
    player_kills: int = 0
    god_mode: bool = False

    def fire_laser(
        self,
        owner: Combatant,
        direction: np.ndarray,
        origin: np.ndarray,
    ) -> bool:
        if owner.stats.energy < 8:
            return False
        from sim.frontier import laser_damage_tons
        owner.stats.energy -= 8
        vel = direction / max(float(np.linalg.norm(direction)), 1e-9) * 80.0  # km/s beam packet
        mining = bool(getattr(owner.stats, "_mining_laser", False))
        self.projectiles.append(
            Projectile(
                kind="laser",
                position=origin.copy(),
                velocity=owner.state.velocity + vel,
                owner_id=owner.id,
                damage=laser_damage_tons(
                    owner.stats.laser_power,
                    beam=bool(getattr(owner.stats, "_beam_laser", False)),
                    mining=mining,
                ),
                life=0.8,
            )
        )
        return True

    def fire_missile(self, owner: Combatant, origin: np.ndarray, target_id: str | None) -> bool:
        if not owner.stats.has_missile or owner.stats.missiles <= 0:
            return False
        owner.stats.missiles -= 1
        fwd = owner.state.orientation[:, 2]
        self.projectiles.append(
            Projectile(
                kind="missile",
                position=origin.copy(),
                velocity=owner.state.velocity + fwd * 5.0,
                owner_id=owner.id,
                damage=60.0,
                life=30.0,
                target_id=target_id,
            )
        )
        return True

    def step(self, dt: float, frames_world_pos) -> list[str]:
        """Advance projectiles and resolve hits. frames_world_pos(combatant)->world pos."""
        messages: list[str] = []
        survivors: list[Projectile] = []
        for proj in self.projectiles:
            if proj.kind == "missile" and proj.target_id and proj.target_id in self.combatants:
                tgt = self.combatants[proj.target_id]
                if tgt.alive:
                    tw = frames_world_pos(tgt)
                    to_t = tw - proj.position
                    dist = float(np.linalg.norm(to_t))
                    if dist > 1e-6:
                        desired = to_t / dist * 25.0
                        proj.velocity = proj.velocity * 0.9 + desired * 0.1
            proj.position = proj.position + proj.velocity * dt
            proj.life -= dt
            if proj.life <= 0:
                continue
            hit = False
            for cid, c in self.combatants.items():
                if not c.alive or cid == proj.owner_id:
                    continue
                cw = frames_world_pos(c)
                if float(np.linalg.norm(cw - proj.position)) < 0.5:  # 500m hit sphere km
                    hit = True
                    self._apply_damage(proj, c, messages)
                    break
            if not hit:
                survivors.append(proj)
        self.projectiles = survivors
        return messages

    def _apply_damage(self, proj: Projectile, target: Combatant, messages: list[str]) -> None:
        if self.god_mode and target.role == "player":
            return
        from sim.frontier import apply_shield_hit
        shield, hull_dmg = apply_shield_hit(target.stats.shield, target.stats.max_shield, proj.damage)
        target.stats.shield = shield
        target.stats.hull -= hull_dmg
        owner = self.combatants.get(proj.owner_id)
        if target.stats.hull <= 0:
            target.alive = False
            self.kills += 1
            messages.append(f"{target.name} destroyed")
            if owner and owner.role == "player":
                self.player_kills += 1
                if target.legal and target.role != "pirate":
                    self.player_legal = False
                    self.fines += 5000
                    messages.append("Fine: unlawful killing (+5000)")
                elif target.role == "pirate":
                    owner.stats  # bounty payout handled outside
                    messages.append(f"Bounty claimed: {target.bounty} Cr")
        elif owner and owner.role == "player" and target.legal and target.role == "police":
            self.player_legal = False
            self.fines += 500
            messages.append("Assault on police (+500 fine)")
