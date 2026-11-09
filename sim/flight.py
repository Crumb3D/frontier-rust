"""Newtonian flight model with set-speed and fuel burn."""

from __future__ import annotations

from dataclasses import dataclass, field

import numpy as np

from sim.frame import FrameState


def _normalize(v: np.ndarray) -> np.ndarray:
    n = float(np.linalg.norm(v))
    if n < 1e-12:
        return np.zeros(3, dtype=np.float64)
    return v / n


def _skew(v: np.ndarray) -> np.ndarray:
    x, y, z = v
    return np.array([[0, -z, y], [z, 0, -x], [-y, x, 0]], dtype=np.float64)


def rotate_local(orientation: np.ndarray, rates: np.ndarray, dt: float) -> np.ndarray:
    """Integrate body angular rates into orientation matrix (body axes in parent)."""
    # rates: [roll, pitch, yaw] in body frame rad/s
    omega = rates
    w = float(np.linalg.norm(omega))
    if w < 1e-12 or abs(dt) < 1e-12:
        return orientation
    axis = omega / w
    angle = w * dt
    K = _skew(axis)
    R = np.eye(3) + math_sin(angle) * K + (1 - math_cos(angle)) * (K @ K)
    # Apply in body frame: new = old @ R
    out = orientation @ R
    # Orthonormalize
    u, _, vt = np.linalg.svd(out)
    return u @ vt


def math_sin(x: float) -> float:
    return float(np.sin(x))


def math_cos(x: float) -> float:
    return float(np.cos(x))


@dataclass
class ShipStats:
    name: str = "Eagle"
    mass_t: float = 20.0
    thrust_kn: float = 200.0  # kN
    retro_kn: float = 100.0
    lateral_kn: float = 40.0
    torque: float = 1.5  # rad/s^2 scale
    max_fuel_t: float = 5.0
    fuel_t: float = 5.0
    jump_range_ly: float = 7.0
    hull: float = 100.0
    max_hull: float = 100.0
    shield: float = 50.0
    max_shield: float = 50.0
    energy: float = 100.0
    max_energy: float = 100.0
    laser_power: float = 10.0
    has_missile: bool = True
    missiles: int = 1
    has_docking_computer: bool = False
    cargo_capacity: int = 4
    cargo: dict[str, int] = field(default_factory=dict)
    price: int = 44000


@dataclass
class FlightControls:
    main: float = 0.0  # -1 retro .. +1 main
    lateral: float = 0.0  # strafe / vertical combined y
    vertical: float = 0.0
    roll: float = 0.0
    pitch: float = 0.0
    yaw: float = 0.0
    set_speed: float | None = None  # km/s along nose, None = manual
    engines_off: bool = False  # pure inertia: no thrust, velocity is kept


class FlightModel:
    """Integrates Newtonian motion with optional set-speed hold."""

    def __init__(self, stats: ShipStats | None = None) -> None:
        self.stats = stats or ShipStats()
        self.controls = FlightControls()
        self.cabin_temp = 20.0  # C, rises near stars / with time compression heat

    def forward(self, orientation: np.ndarray) -> np.ndarray:
        # Body +Z is forward (nose)
        return orientation[:, 2].copy()

    def up(self, orientation: np.ndarray) -> np.ndarray:
        return orientation[:, 1].copy()

    def right(self, orientation: np.ndarray) -> np.ndarray:
        return orientation[:, 0].copy()

    def step(self, state: FrameState, dt: float, near_star_heat: float = 0.0) -> FrameState:
        if dt <= 0:
            return state
        s = state.copy()
        stats = self.stats
        ctrl = self.controls

        # Angular rates in body frame (X=right, Y=up, Z=fwd).
        # Right-hand rule about +X pitches nose DOWN, so pitch command is negated.
        # Same for roll about +Z. Yaw about +Y already turns nose right.
        max_rate = 1.0  # rad/s
        target_rates = np.array(
            [
                -ctrl.pitch * max_rate,
                ctrl.yaw * max_rate,
                -ctrl.roll * max_rate,
            ],
            dtype=np.float64,
        )
        s.angular_velocity = target_rates  # Frontier-style: stick maps to rate
        s.orientation = rotate_local(s.orientation, s.angular_velocity, dt)

        fwd = self.forward(s.orientation)
        right = self.right(s.orientation)
        up = self.up(s.orientation)

        # Acceleration from thrusters (km/s^2)
        # F=ma: a = thrust_kN / mass_t  -> m/s^2 = kN/t = 1000 N / 1000 kg = 1 m/s^2 per (kN/t)
        main_thrust = stats.thrust_kn if ctrl.main >= 0 else stats.retro_kn
        # thrust_kn is g * 9.81, so * 0.001 is km/s^2 at that constant g
        a_main = (ctrl.main * main_thrust) * 0.001
        a_lat = (ctrl.lateral * stats.lateral_kn) * 0.001
        a_vert = (ctrl.vertical * stats.lateral_kn) * 0.001

        accel = fwd * a_main + right * a_lat + up * a_vert

        # Engines off: keep the velocity already on the state. No thrust, no hold.
        if ctrl.engines_off:
            accel = np.zeros(3)
        # Set-speed: hold speed along nose
        elif ctrl.set_speed is not None:
            along = float(np.dot(s.velocity, fwd))
            err = ctrl.set_speed - along
            # PD-ish: accelerate along nose to close error
            max_a = stats.thrust_kn * 0.001
            a_cmd = float(np.clip(err * 0.5, -max_a, max_a))
            accel = fwd * a_cmd

        # Fuel burn proportional to thrust usage
        thrust_use = abs(ctrl.main) + 0.3 * abs(ctrl.lateral) + 0.3 * abs(ctrl.vertical)
        if ctrl.engines_off:
            thrust_use = 0.0
        elif ctrl.set_speed is not None and abs(ctrl.set_speed) > 0.01:
            thrust_use = max(thrust_use, 0.2)
        burn = thrust_use * 0.00002 * dt  # tonnes
        if stats.fuel_t > 0 and thrust_use > 0.01:
            used = min(burn, stats.fuel_t)
            stats.fuel_t -= used
        elif stats.fuel_t <= 0:
            accel *= 0.0

        # Exact constant-acceleration step
        s.position = s.position + s.velocity * dt + 0.5 * accel * dt * dt
        s.velocity = s.velocity + accel * dt

        # Cabin temperature
        self.cabin_temp += (near_star_heat - (self.cabin_temp - 20.0) * 0.01) * min(dt, 1.0)
        self.cabin_temp = float(np.clip(self.cabin_temp, -50, 200))

        # Energy recharge
        stats.energy = min(stats.max_energy, stats.energy + 5.0 * dt)
        from sim.frontier import BOOSTER_RECHARGE, SHIELD_RECHARGE
        rate = BOOSTER_RECHARGE if getattr(stats, "_energy_booster", False) else SHIELD_RECHARGE
        if stats.max_shield > 0:
            stats.shield = min(stats.max_shield, stats.shield + stats.max_shield * rate * dt)

        return s

    def adjust_set_speed(self, delta: float) -> None:
        cur = self.controls.set_speed
        if cur is None:
            cur = 0.0
        self.controls.set_speed = float(np.clip(cur + delta, -50.0, 500.0))

    def zero_set_speed(self) -> None:
        self.controls.set_speed = 0.0
