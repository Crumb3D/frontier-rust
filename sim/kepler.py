"""Kepler orbital mechanics — solve positions from elements at time T."""

from __future__ import annotations

import math
from dataclasses import dataclass

import numpy as np

# Gravitational parameter scale: GM in km^3/s^2 for Sol-mass star
GM_SOL = 1.32712440018e11  # km^3/s^2
AU_KM = 1.495978707e8  # km
# Game units: 1 unit = 1 km for local system work


@dataclass
class OrbitalElements:
    """Classic Keplerian elements relative to a parent barycenter."""

    semi_major_au: float  # a in AU
    eccentricity: float = 0.0
    inclination_deg: float = 0.0
    long_asc_node_deg: float = 0.0  # Omega
    arg_periapsis_deg: float = 0.0  # omega
    mean_anomaly0_deg: float = 0.0  # M0 at epoch (stardate day 0)
    period_days: float | None = None  # if None, derived from a and parent_mass
    parent_mass_sol: float = 1.0

    def period(self) -> float:
        if self.period_days is not None:
            return self.period_days
        # Kepler's 3rd: T^2 = a^3 / M  (years for a in AU, M in solar masses)
        years = math.sqrt(self.semi_major_au**3 / max(self.parent_mass_sol, 1e-9))
        return years * 365.25


def _solve_kepler(M: float, e: float, tol: float = 1e-10) -> float:
    """Solve Kepler's equation for eccentric anomaly E. M, e in radians / unitless."""
    M = ((M + math.pi) % (2 * math.pi)) - math.pi
    if e < 1e-8:
        return M
    E = M if e < 0.8 else math.pi
    for _ in range(32):
        f = E - e * math.sin(E) - M
        fp = 1.0 - e * math.cos(E)
        d = f / fp
        E -= d
        if abs(d) < tol:
            break
    return E


def position_at(elements: OrbitalElements, days: float) -> np.ndarray:
    """Return cartesian position (km) relative to parent at stardate day `days`."""
    a_km = elements.semi_major_au * AU_KM
    e = elements.eccentricity
    T = elements.period()
    n = 2.0 * math.pi / T  # rad/day
    M0 = math.radians(elements.mean_anomaly0_deg)
    M = M0 + n * days
    E = _solve_kepler(M, e)

    # Perifocal coordinates
    cos_E, sin_E = math.cos(E), math.sin(E)
    x_p = a_km * (cos_E - e)
    y_p = a_km * math.sqrt(1 - e * e) * sin_E
    z_p = 0.0

    # Rotate by omega, i, Omega
    w = math.radians(elements.arg_periapsis_deg)
    i = math.radians(elements.inclination_deg)
    O = math.radians(elements.long_asc_node_deg)

    cos_w, sin_w = math.cos(w), math.sin(w)
    cos_O, sin_O = math.cos(O), math.sin(O)
    cos_i, sin_i = math.cos(i), math.sin(i)

    # R = Rz(O) Rx(i) Rz(w)
    r11 = cos_O * cos_w - sin_O * sin_w * cos_i
    r12 = -cos_O * sin_w - sin_O * cos_w * cos_i
    r13 = sin_O * sin_i
    r21 = sin_O * cos_w + cos_O * sin_w * cos_i
    r22 = -sin_O * sin_w + cos_O * cos_w * cos_i
    r23 = -cos_O * sin_i
    r31 = sin_w * sin_i
    r32 = cos_w * sin_i
    r33 = cos_i

    x = r11 * x_p + r12 * y_p + r13 * z_p
    y = r21 * x_p + r22 * y_p + r23 * z_p
    z = r31 * x_p + r32 * y_p + r33 * z_p
    return np.array([x, y, z], dtype=np.float64)


def velocity_at(elements: OrbitalElements, days: float, dt: float = 1e-4) -> np.ndarray:
    """Finite-difference velocity (km/day) then convert to km/s."""
    p0 = position_at(elements, days)
    p1 = position_at(elements, days + dt)
    return (p1 - p0) / (dt * 86400.0)  # km/s
