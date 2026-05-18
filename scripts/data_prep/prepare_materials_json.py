#!/usr/bin/env python3
"""
prepare_materials_json.py — Generate materials.json from mechanical-properties CSV data.

Inputs
------
  raw_data/Materials and their Mechanical Properties.csv
      Authoritative row set.  Columns used: Std, ID, Material, Heat treatment,
      E (MPa), G (MPa), mu, Ro (kg/m³).  One output record per CSV row —
      no deduplication.

  raw_data/Material Properties.csv
      Supplemental data.  Columns used: MATERIAL, Type,
      Thermal Expansion (a,10⁻⁶/C).  Used to:
        • back-fill linear_thermal_expansion_per_c (converted × 1e-6)
        • classify material family for dln_e_dtemp_per_c estimation.

Output
------
  src-tauri/resources/medium-and-material/materials.json
      JSON array; each element:
        {
          "standard":       "ANSI",
          "source_id":      "<UUID>",
          "material":       "Steel SAE 1015",
          "heat_treatment": "as-rolled",
          "properties": {
            "reference_density_kg_per_m3":      7860.0,
            "poisson_ratio":                    0.3,
            "reference_youngs_modulus_mpa":     207000.0,
            "reference_temperature_c":          20.0,
            "linear_thermal_expansion_per_c":   1.2e-05,
            "dln_e_dtemp_per_c":               -2.0e-04
          }
        }

Usage
-----
    python scripts/data_prep/prepare_materials_json.py

The script is idempotent; re-running overwrites the output file deterministically.

dln_e_dtemp_per_c estimation
-----------------------------
Young's modulus decreases with temperature for most structural materials.  The
log-slope  d(ln E)/dT  is estimated per material family using published typical
values [see table below].  A match against Material Properties.csv Type column
is attempted first; if no match is found the material name itself is scanned for
family keywords.

Family slope table (all values [1/°C], negative = softens with heat):

  metal    (generic):   -2.0e-4   (~2 % per 100 K, typical metals)
  steel / iron:         -2.0e-4   (carbon & alloy steels, ASTM)
  stainless steel:      -2.0e-4
  aluminum alloy:       -4.5e-4   (relatively strong softening)
  copper / brass:       -3.5e-4
  titanium alloy:       -3.0e-4
  nickel / superalloy:  -1.5e-4   (very refractory)
  ceramic:              -1.0e-4   (brittle; stiff until fracture)
  polymer / rubber:     -5.0e-3   (strong viscoelastic softening)
  composite / wood:     -3.0e-4
  bone / biological:    -2.0e-4
  default (unknown):    -2.0e-4
"""

import csv
import sys
from pathlib import Path

# ---------------------------------------------------------------------------
# Allow running from any working directory.
# ---------------------------------------------------------------------------
sys.path.insert(0, str(Path(__file__).resolve().parent))
from config import MECH_CSV, PROP_CSV, MATERIALS_JSON, REFERENCE_TEMPERATURE_C
from data_prep_utils import normalize_name, parse_float, write_json

# ---------------------------------------------------------------------------
# dln_e_dtemp_per_c family table [1/°C].
# Keys are matched against the *normalized* Type string from Material
# Properties.csv first, then against the normalized material name.
# ---------------------------------------------------------------------------
# fmt: off
_FAMILY_SLOPE: list[tuple[list[str], float]] = [
    # keywords (any must appear in the normalized name/type)   slope [1/°C]
    (["polymer", "rubber", "elastomer", "plastic"],           -5.0e-3),
    (["aluminum", "aluminium"],                               -4.5e-4),
    (["copper", "brass", "bronze"],                           -3.5e-4),
    (["titanium"],                                            -3.0e-4),
    (["composite", "wood", "timber", "bamboo", "bone",
      "tendon", "cartilage", "skin", "muscle"],               -3.0e-4),
    (["nickel", "cobalt", "superalloy", "inconel", "waspaloy"],-1.5e-4),
    (["ceramic", "glass", "silicon", "alumina", "zirconia",
      "carbide", "nitride"],                                   -1.0e-4),
    (["cast iron", "iron"],                                   -2.0e-4),
    (["stainless"],                                           -2.0e-4),
    (["steel"],                                               -2.0e-4),
    (["metal"],                                               -2.0e-4),
]
_DEFAULT_SLOPE = -2.0e-4
# fmt: on


def _family_slope(name: str, family_type: str | None) -> float:
    """Return estimated d(ln E)/dT slope for a material.

    *family_type* comes from the ``Type`` column of Material Properties.csv.
    *name* is the material name from the main CSV.
    """
    # Prefer classification from the supplemental Type column.
    candidates = []
    if family_type:
        candidates.append(normalize_name(family_type))
    candidates.append(normalize_name(name))

    for keywords, slope in _FAMILY_SLOPE:
        for candidate in candidates:
            if any(kw in candidate for kw in keywords):
                return slope
    return _DEFAULT_SLOPE


# ---------------------------------------------------------------------------
# Alias table: maps *normalized* names from the main CSV → *normalized* names
# in Material Properties.csv so the cross-CSV thermal-expansion lookup works
# for common mismatches.
# ---------------------------------------------------------------------------
# Format: (main_csv_fragment, props_csv_fragment)
# The lookup uses token-based prefix/substring matching after normalization.
_ALIASES: list[tuple[str, str]] = [
    # Main CSV uses "Steel SAE ..." while Props CSV uses "Steel (AISI/SAE ...)"
    ("steel sae",    "steel"),
    ("steel aisi",   "steel"),
    ("stainless",    "stainless"),
    ("cast iron",    "cast iron"),
    ("gray iron",    "cast iron"),
    ("nodular iron", "cast iron"),
    ("aluminum",     "aluminum alloy"),
    ("aluminium",    "aluminum alloy"),
    ("copper",       "copper"),
    ("brass",        "brass"),
    ("titanium",     "titanium alloy"),
    ("nickel",       "nickel alloy"),
    ("magnesium",    "magnesium alloy"),
    ("beryllium",    "beryllium alloy"),
]


def _alias_lookup(norm_main: str) -> str:
    """Return the preferred *normalized* key to use for lookup in the props
    table, applying alias rules when the exact name is absent."""
    for frag_main, frag_props in _ALIASES:
        if frag_main in norm_main:
            return frag_props
    return norm_main


# ---------------------------------------------------------------------------
# Load supplemental material properties (thermal expansion + Type).
# ---------------------------------------------------------------------------
def _load_props_table(csv_path: Path) -> dict[str, dict]:
    """Return a dict keyed by *normalized* material name → row dict.

    Columns captured:
      expansion_per_c : linear thermal expansion in [1/°C] (converted from 10⁻⁶/°C)
      type            : material family string (e.g. "metal", "ceramic", …)
    """
    table: dict[str, dict] = {}
    with csv_path.open(newline="", encoding="utf-8-sig") as fh:
        reader = csv.DictReader(fh)
        for row in reader:
            raw_name = row.get("MATERIAL", "").strip()
            if not raw_name:
                continue
            key = normalize_name(raw_name)
            exp_raw = parse_float(row.get("Thermal Expansion (a,10-6/C)", ""))
            table[key] = {
                "expansion_per_c": exp_raw * 1e-6 if exp_raw is not None else None,
                "type": row.get("Type", "").strip() or None,
            }
    return table


# ---------------------------------------------------------------------------
# Cross-CSV thermal-expansion lookup with alias + best-token-overlap fallback.
# ---------------------------------------------------------------------------
def _find_expansion(norm_main: str, props_table: dict[str, dict]) -> tuple[float | None, str | None]:
    """Return (linear_thermal_expansion_per_c, material_type) for *norm_main*.

    Resolution order:
    1. Exact normalized-name match.
    2. Alias-rewrite then exact match.
    3. Best substring/token-overlap match (≥2 tokens must overlap).

    Returns (None, None) when no match with sufficient confidence is found.
    """
    # 1. Exact match
    if norm_main in props_table:
        r = props_table[norm_main]
        return r["expansion_per_c"], r["type"]

    # 2. Alias rewrite
    alias_key = _alias_lookup(norm_main)
    if alias_key != norm_main and alias_key in props_table:
        r = props_table[alias_key]
        return r["expansion_per_c"], r["type"]

    # 3. Token overlap – find the props-table entry that shares the most tokens
    main_tokens = set(norm_main.split())
    best_overlap = 0
    best_key = None
    for k in props_table:
        overlap = len(main_tokens & set(k.split()))
        if overlap > best_overlap:
            best_overlap = overlap
            best_key = k

    # Require at least 2 shared tokens OR alias_key is a substring of best key
    if best_key is not None and (
        best_overlap >= 2 or (alias_key and alias_key in best_key)
    ):
        r = props_table[best_key]
        return r["expansion_per_c"], r["type"]

    # Also try: does any props key start with alias_key?
    if alias_key:
        for k, r in props_table.items():
            if k.startswith(alias_key) or alias_key.startswith(k.split()[0]):
                return r["expansion_per_c"], r["type"]

    return None, None


# ---------------------------------------------------------------------------
# Main
# ---------------------------------------------------------------------------
def main() -> None:
    props_table = _load_props_table(PROP_CSV)

    records: list[dict] = []
    with MECH_CSV.open(newline="", encoding="utf-8-sig") as fh:
        reader = csv.DictReader(fh)
        for row in reader:
            mat_name = row.get("Material", "").strip()
            norm = normalize_name(mat_name)

            # --- numeric fields always present ---
            e_mpa = parse_float(row.get("E"))
            mu = parse_float(row.get("mu"))
            ro = parse_float(row.get("Ro"))

            if e_mpa is None or mu is None or ro is None:
                # Rows with missing primary data are skipped (should be rare).
                continue

            # --- thermal expansion backfill ---
            exp_per_c, mat_type = _find_expansion(norm, props_table)

            # --- dln_e_dtemp estimation ---
            dln_slope = _family_slope(mat_name, mat_type)

            record = {
                "standard": row.get("Std", "").strip() or None,
                "source_id": row.get("ID", "").strip() or None,
                "material": mat_name or None,
                "heat_treatment": row.get("Heat treatment", "").strip() or None,
                "properties": {
                    "reference_density_kg_per_m3": ro,
                    "poisson_ratio": mu,
                    "reference_youngs_modulus_mpa": e_mpa,
                    "reference_temperature_c": REFERENCE_TEMPERATURE_C,
                    "linear_thermal_expansion_per_c": exp_per_c if exp_per_c is not None else 0.0,
                    "dln_e_dtemp_per_c": dln_slope,
                },
            }
            records.append(record)

    write_json(MATERIALS_JSON, records)
    print(f"Wrote {len(records)} material records → {MATERIALS_JSON}")


if __name__ == "__main__":
    main()
