"""
Shared acoustic-coupling math for material/medium ranking scripts.

All functions are deterministic and return finite values for finite inputs.
"""

from __future__ import annotations

import csv
import math
from pathlib import Path
from typing import TYPE_CHECKING

# ---------------------------------------------------------------------------
# Family → friction-CSV name candidates (ordered: preferred first, siblings after)
# ---------------------------------------------------------------------------
_FAMILY_TO_FRICTION_NAMES: dict[str, list[str]] = {
    "stainless-steel": ["stainless", "steel"],
    "steel":           ["steel"],
    "cast-iron":       ["cast iron", "iron"],
    "iron":            ["iron", "cast iron"],
    "aluminum":        ["aluminum", "aluminium"],
    "copper-alloy":    ["copper", "brass", "bronze"],
    "titanium":        ["titanium"],
    "nickel-alloy":    ["nickel"],
    "magnesium":       ["magnesium"],
    "zinc":            ["zinc"],
    "lead":            ["lead"],
    "tin":             ["tin"],
    "ceramic-glass":   ["glass", "sapphire", "diamond", "carbon"],
    "polymer":         ["rubber", "nylon", "polyethylene", "polytetrafluoroethylene", "teflon", "plexiglas", "polystyrene"],
    "wood":            ["wood", "oak"],
}

# Surface-condition keywords
_LUBRICANT_KEYWORDS = ("lubricat", "grease", "greasy", "oil", "castor", "stearic", "lard", "wet")
_DRY_KEYWORDS = ("clean and dry", "clean", "dry", "freshly")


def _friction_condition_preference(condition: str, prefer_lubricated: bool) -> int:
    """Return a sort key (lower = more preferred) for a surface-condition string."""
    norm = condition.lower()
    if prefer_lubricated:
        if any(k in norm for k in _LUBRICANT_KEYWORDS):
            return 0
        if any(k in norm for k in _DRY_KEYWORDS):
            return 1
        return 2
    else:
        if any(k in norm for k in _DRY_KEYWORDS):
            return 0
        if any(k in norm for k in _LUBRICANT_KEYWORDS):
            return 1
        return 2


def _row_coefficient(row: dict) -> float | None:
    """Average kinetic + static when both present; return whichever is available."""
    from data_prep_utils import parse_friction_range
    kinetic = parse_friction_range(row.get("Kinetic Friction Coefficient", ""))
    static = parse_friction_range(row.get("Static Friction Coefficient", ""))
    if kinetic is not None and static is not None:
        return (kinetic + static) / 2.0
    return kinetic if kinetic is not None else static


class FrictionTable:
    """Lookup table for material-pair friction coefficients parsed from a CSV.

    The CSV format has columns::

        Material 1, Material 2, Surface Condition,
        Kinetic Friction Coefficient, Static Friction Coefficient

    Values may be blank or expressed as ranges like ``"0.5 - 0.8"``;
    these are resolved to midpoints via :func:`parse_friction_range`.

    Usage::

        table = FrictionTable(path)
        coeff, source = table.lookup("steel", "cast-iron", prefer_lubricated=False)
    """

    def __init__(self, csv_path: Path) -> None:
        from data_prep_utils import normalize_name
        self._rows: list[dict] = []
        with csv_path.open(newline="", encoding="utf-8-sig") as fh:
            reader = csv.DictReader(fh)
            for raw_row in reader:
                # The CSV has leading spaces on every column name after the first;
                # strip all keys and values defensively.
                row = {k.strip(): (v.strip() if v is not None else "") for k, v in raw_row.items() if k is not None}
                mat1 = normalize_name(row.get("Material 1", ""))
                mat2 = normalize_name(row.get("Material 2", ""))
                if not mat1 or not mat2:
                    continue
                self._rows.append({
                    "mat1": mat1,
                    "mat2": mat2,
                    "condition": row.get("Surface Condition", ""),
                    "Kinetic Friction Coefficient": row.get("Kinetic Friction Coefficient", ""),
                    "Static Friction Coefficient": row.get("Static Friction Coefficient", ""),
                })

    def _select_best_row(self, candidates: list[dict], prefer_lubricated: bool) -> float | None:
        """Pick the preferred-condition row and return its averaged coefficient."""
        if not candidates:
            return None
        candidates_sorted = sorted(
            candidates,
            key=lambda r: (
                _friction_condition_preference(r["condition"], prefer_lubricated),
                -sum(1 for k in ("Kinetic Friction Coefficient", "Static Friction Coefficient")
                     if (r.get(k) or "").strip()),
            ),
        )
        for row in candidates_sorted:
            coeff = _row_coefficient(row)
            if coeff is not None:
                return coeff
        return None

    def _rows_for_name_pair(self, name_a: str, name_b: str) -> list[dict]:
        """Return all rows where (mat1, mat2) matches the given pair in either order."""
        from data_prep_utils import normalize_name
        na = normalize_name(name_a)
        nb = normalize_name(name_b)
        return [
            r for r in self._rows
            if (r["mat1"] == na and r["mat2"] == nb)
            or (r["mat1"] == nb and r["mat2"] == na)
        ]

    def _rows_containing_name(self, name: str) -> list[dict]:
        """Return all rows where either material column contains *name* as a substring."""
        from data_prep_utils import normalize_name
        n = normalize_name(name)
        return [r for r in self._rows if n in r["mat1"] or n in r["mat2"]]

    def lookup(
        self,
        family_a: str,
        family_b: str,
        prefer_lubricated: bool,
    ) -> tuple[float | None, str]:
        """Return ``(coefficient, source)`` for a family pair.

        *source* is one of:
        - ``"direct"``    — matched using both families' primary friction names
        - ``"sibling"``   — matched via a sibling/fallback friction name
        - ``"substring"`` — loose substring match across all rows
        - ``"none"``      — no match found
        """
        names_a = _FAMILY_TO_FRICTION_NAMES.get(family_a, [family_a])
        names_b = _FAMILY_TO_FRICTION_NAMES.get(family_b, [family_b])

        # direct: try primary names for each family (first element)
        if names_a and names_b:
            rows = self._rows_for_name_pair(names_a[0], names_b[0])
            coeff = self._select_best_row(rows, prefer_lubricated)
            if coeff is not None:
                return coeff, "direct"

        # sibling: try all name combinations for both families
        for na in names_a:
            for nb in names_b:
                if na == names_a[0] and nb == names_b[0]:
                    continue
                rows = self._rows_for_name_pair(na, nb)
                coeff = self._select_best_row(rows, prefer_lubricated)
                if coeff is not None:
                    return coeff, "sibling"

        # substring: rows referencing any name of family_a AND any name of family_b
        for na in names_a:
            for nb in names_b:
                rows_a_ids = {id(r) for r in self._rows_containing_name(na)}
                rows_both = [r for r in self._rows_containing_name(nb) if id(r) in rows_a_ids]
                coeff = self._select_best_row(rows_both, prefer_lubricated)
                if coeff is not None:
                    return coeff, "substring"

        # estimated: average the self-pair coefficients for each family
        # (μ_AB ≈ (μ_AA + μ_BB) / 2 — reasonable when no cross-pair data exists)
        self_coeffs: list[float] = []
        for names in (names_a, names_b):
            for n in names:
                rows = self._rows_for_name_pair(n, n)
                c = self._select_best_row(rows, prefer_lubricated)
                if c is not None:
                    self_coeffs.append(c)
                    break  # one representative per family
        if self_coeffs:
            return sum(self_coeffs) / len(self_coeffs), "estimated"

        # last-resort: for metal families use steel×steel as a generic metal proxy
        _METAL_FAMILIES = {
            "steel", "stainless-steel", "cast-iron", "iron",
            "aluminum", "copper-alloy", "titanium", "nickel-alloy",
            "magnesium", "zinc", "lead", "tin",
        }
        if family_a in _METAL_FAMILIES and family_b in _METAL_FAMILIES:
            rows = self._rows_for_name_pair("steel", "steel")
            coeff = self._select_best_row(rows, prefer_lubricated)
            if coeff is not None:
                return coeff, "estimated"

        return None, "none"


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
