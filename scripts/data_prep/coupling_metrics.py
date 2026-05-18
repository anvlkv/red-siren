"""
Shared acoustic-coupling math for material/medium ranking scripts.

All functions are deterministic and return finite values for finite inputs.
"""

from __future__ import annotations

import math


def longitudinal_wave_speed_m_per_s(
    youngs_modulus_pa: float,
    density_kg_per_m3: float,
    poisson_ratio: float,
) -> float | None:
    """Return isotropic longitudinal wave speed in m/s.

    c_L = sqrt(E(1-nu) / (rho(1+nu)(1-2nu)))
    """
    if youngs_modulus_pa <= 0.0 or density_kg_per_m3 <= 0.0:
        return None

    denom = density_kg_per_m3 * (1.0 + poisson_ratio) * (1.0 - 2.0 * poisson_ratio)
    numer = youngs_modulus_pa * (1.0 - poisson_ratio)
    if denom <= 0.0 or numer <= 0.0:
        return None
    return math.sqrt(numer / denom)


def acoustic_impedance_rayl(density_kg_per_m3: float, wave_speed_m_per_s: float) -> float | None:
    if density_kg_per_m3 <= 0.0 or wave_speed_m_per_s <= 0.0:
        return None
    return density_kg_per_m3 * wave_speed_m_per_s


def transmission_coefficient(z1: float, z2: float) -> float | None:
    """Energy transmission coefficient at a normal-incidence interface."""
    if z1 <= 0.0 or z2 <= 0.0:
        return None
    denom = (z1 + z2) ** 2
    if denom <= 0.0:
        return None
    return (4.0 * z1 * z2) / denom


def reflection_coefficient(z1: float, z2: float) -> float | None:
    """Energy reflection coefficient at a normal-incidence interface."""
    t = transmission_coefficient(z1, z2)
    if t is None:
        return None
    return max(0.0, min(1.0, 1.0 - t))


def impedance_ratio(z1: float, z2: float) -> float | None:
    if z1 <= 0.0 or z2 <= 0.0:
        return None
    hi = max(z1, z2)
    lo = min(z1, z2)
    return hi / lo
