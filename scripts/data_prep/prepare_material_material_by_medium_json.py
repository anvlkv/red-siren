#!/usr/bin/env python3
"""
prepare_material_material_json.py — Medium-first material/material ranking pipeline.

Input
-----
  src-tauri/resources/medium-and-material/materials.json
    src-tauri/resources/medium-and-material/mediums.json

Output
------
    src-tauri/resources/medium-and-material/material-material-by-medium.json

Pipeline (single N = COUPLING_TOP_N)
------------------------------------
    1) For each medium, score all material-medium matches and keep top N seed materials.
    2) For each medium, add up to N nearest material variants to the seed set.
    3) For each medium section, rank material-material pairs within that expanded set
         using diversity-aware greedy ranking and keep top N pairs.
"""

from __future__ import annotations

import json
import math
import sys
from collections import Counter
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))
from coupling_metrics import (
    FrictionTable,
    acoustic_impedance_rayl,
    impedance_ratio,
    longitudinal_wave_speed_m_per_s,
    reflection_coefficient,
    transmission_coefficient,
)
from config import (
    COUPLING_TOP_N,
    FRICTION_CSV,
    FRICTION_RANKING_WEIGHT,
    MATERIAL_MATERIAL_BY_MEDIUM_JSON,
    MATERIAL_MATERIAL_GREEDY_BASE_WEIGHT,
    MATERIAL_MATERIAL_GREEDY_CROSS_FAMILY_BONUS,
    MATERIAL_MATERIAL_GREEDY_FAMILY_NOVELTY_WEIGHT,
    MATERIALS_A_GREEDY_FAMILY_NOVELTY_WEIGHT,
    MATERIALS_A_GREEDY_MAX_PER_FAMILY,
    MATERIALS_B_GREEDY_MAX_PER_FAMILY,
    MATERIAL_SIGNATURE_DENSITY_DECIMALS,
    MATERIAL_SIGNATURE_IMPEDANCE_DECIMALS,
    MATERIAL_SIGNATURE_POISSON_DECIMALS,
    MATERIAL_SIGNATURE_YOUNGS_MPA_DECIMALS,
    MATERIAL_MEDIUM_LOUDNESS_WEIGHT,
    MATERIAL_MEDIUM_TRANSFER_WEIGHT,
    MATERIALS_JSON,
    MEDIUMS_JSON,
)
from data_prep_utils import normalize_name, write_json

_FAMILY_KEYWORDS: list[tuple[str, tuple[str, ...]]] = [
    ("stainless-steel", ("stainless",)),
    ("steel", ("steel",)),
    ("cast-iron", ("cast iron", "nodular cast iron", "gray iron", "grey cast iron", "malleable cast iron")),
    ("iron", ("iron",)),
    ("aluminum", ("aluminum", "aluminium")),
    ("copper-alloy", ("copper", "brass", "bronze")),
    ("titanium", ("titanium",)),
    ("nickel-alloy", ("nickel", "inconel", "superalloy", "cobalt")),
    ("magnesium", ("magnesium",)),
    ("zinc", ("zinc",)),
    ("lead", ("lead",)),
    ("tin", ("tin",)),
    ("ceramic-glass", ("ceramic", "glass", "alumina", "zirconia", "carbide", "nitride")),
    ("polymer", ("polymer", "rubber", "plastic", "elastomer")),
    ("wood", ("wood", "bamboo", "timber")),
]

_CODE_PREFIXES = (
    "astm",
    "bs",
    "csn",
    "din",
    "en",
    "gb",
    "gost",
    "iso",
    "jis",
    "sae",
)

_EPS = 1.0e-12


def _load_json(path: Path) -> list[dict]:
    with path.open(encoding="utf-8") as fh:
        data = json.load(fh)
    if not isinstance(data, list):
        raise ValueError(f"Expected JSON array in {path}")
    return data


def _material_impedance(material_row: dict) -> float | None:
    props = material_row.get("properties") or {}
    rho = props.get("reference_density_kg_per_m3")
    e_mpa = props.get("reference_youngs_modulus_mpa")
    nu = props.get("poisson_ratio")

    if not isinstance(rho, (int, float)):
        return None
    if not isinstance(e_mpa, (int, float)):
        return None
    if not isinstance(nu, (int, float)):
        return None

    c_l = longitudinal_wave_speed_m_per_s(
        youngs_modulus_pa=float(e_mpa) * 1.0e6,
        density_kg_per_m3=float(rho),
        poisson_ratio=float(nu),
    )
    if c_l is None:
        return None
    return acoustic_impedance_rayl(float(rho), c_l)


def _medium_impedance(medium_row: dict) -> float | None:
    props = medium_row.get("properties") or {}
    z = props.get("impedance_m_rayl")
    if isinstance(z, (int, float)) and z > 0.0:
        return float(z)

    rho = props.get("density_kg_per_m3")
    speed = props.get("speed_of_sound_m_per_s")
    if not isinstance(rho, (int, float)):
        return None
    if not isinstance(speed, (int, float)):
        return None
    return acoustic_impedance_rayl(float(rho), float(speed))


def _material_signature(material_row: dict, impedance: float) -> tuple[float, float, float, float] | None:
    props = material_row.get("properties") or {}
    rho = props.get("reference_density_kg_per_m3")
    e_mpa = props.get("reference_youngs_modulus_mpa")
    nu = props.get("poisson_ratio")

    if not isinstance(rho, (int, float)):
        return None
    if not isinstance(e_mpa, (int, float)):
        return None
    if not isinstance(nu, (int, float)):
        return None

    return (
        round(float(impedance), MATERIAL_SIGNATURE_IMPEDANCE_DECIMALS),
        round(float(rho), MATERIAL_SIGNATURE_DENSITY_DECIMALS),
        round(float(nu), MATERIAL_SIGNATURE_POISSON_DECIMALS),
        round(float(e_mpa), MATERIAL_SIGNATURE_YOUNGS_MPA_DECIMALS),
    )


def _material_numeric_features(material_row: dict) -> tuple[float, float, float] | None:
    props = material_row.get("properties") or {}
    rho = props.get("reference_density_kg_per_m3")
    e_mpa = props.get("reference_youngs_modulus_mpa")
    nu = props.get("poisson_ratio")
    if not isinstance(rho, (int, float)):
        return None
    if not isinstance(e_mpa, (int, float)):
        return None
    if not isinstance(nu, (int, float)):
        return None
    return float(rho), float(e_mpa), float(nu)


def _material_family(material_row: dict) -> str:
    material_name = normalize_name(str(material_row.get("material") or ""))
    for family, keywords in _FAMILY_KEYWORDS:
        if any(keyword in material_name for keyword in keywords):
            return family

    props = material_row.get("properties") or {}
    rho = props.get("reference_density_kg_per_m3")
    e_mpa = props.get("reference_youngs_modulus_mpa")
    first_token = material_name.split()[0] if material_name.split() else ""
    if first_token in _CODE_PREFIXES:
        inferred = _material_family_from_properties(rho, e_mpa)
        if inferred is not None:
            return inferred

    inferred = _material_family_from_properties(rho, e_mpa)
    if inferred is not None:
        return inferred

    tokens = material_name.split()
    if tokens:
        return tokens[0]
    return "unknown"


def _material_family_from_properties(rho: object, e_mpa: object) -> str | None:
    if not isinstance(rho, (int, float)):
        return None
    if not isinstance(e_mpa, (int, float)):
        return None

    density = float(rho)
    youngs_mpa = float(e_mpa)

    if 2400.0 <= density <= 2900.0 and 55000.0 <= youngs_mpa <= 85000.0:
        return "aluminum"
    if 1500.0 <= density <= 2100.0 and 30000.0 <= youngs_mpa <= 60000.0:
        return "magnesium"
    if 7800.0 <= density <= 9100.0 and 80000.0 <= youngs_mpa <= 150000.0:
        return "copper-alloy"
    if 4300.0 <= density <= 5200.0 and 90000.0 <= youngs_mpa <= 130000.0:
        return "titanium"
    if 7500.0 <= density <= 9200.0 and 150000.0 <= youngs_mpa <= 240000.0:
        return "nickel-alloy"
    if density >= 6800.0 and youngs_mpa >= 190000.0:
        return "steel"
    if 6800.0 <= density <= 7800.0 and 70000.0 <= youngs_mpa < 190000.0:
        return "cast-iron"

    return None


def _dedupe_materials_by_signature(materials: list[dict]) -> list[dict]:
    representatives: dict[tuple[float, float, float, float], dict] = {}

    for material in materials:
        z_material = _material_impedance(material)
        if z_material is None:
            continue

        signature = _material_signature(material, z_material)
        if signature is None:
            continue

        numeric = _material_numeric_features(material)
        if numeric is None:
            continue
        rho, e_mpa, nu = numeric

        candidate = {
            "row": material,
            "impedance": z_material,
            "signature": signature,
            "rho": rho,
            "e_mpa": e_mpa,
            "nu": nu,
        }
        current = representatives.get(signature)
        if current is None:
            representatives[signature] = candidate
            continue

        candidate_key = (
            str(material.get("material")),
            str(material.get("heat_treatment")),
            str(material.get("source_id")),
        )
        current_key = (
            str(current["row"].get("material")),
            str(current["row"].get("heat_treatment")),
            str(current["row"].get("source_id")),
        )
        if candidate_key < current_key:
            representatives[signature] = candidate

    return sorted(
        representatives.values(),
        key=lambda item: (
            str(item["row"].get("material")),
            str(item["row"].get("heat_treatment")),
            str(item["row"].get("source_id")),
        ),
    )


def _build_material_payload(material_entry: dict) -> dict:
    row = material_entry["row"]
    return {
        "standard": row.get("standard"),
        "source_id": row.get("source_id"),
        "material": row.get("material"),
        "heat_treatment": row.get("heat_treatment"),
        "family": material_entry["family"],
        "properties": row.get("properties"),
    }


def _material_medium_score(material_entry: dict, medium_entry: dict) -> dict | None:
    z_material = float(material_entry["impedance"])
    z_medium = _medium_impedance(medium_entry)
    if z_medium is None:
        return None

    transfer = transmission_coefficient(z_material, z_medium)
    reflect = reflection_coefficient(z_material, z_medium)
    ratio = impedance_ratio(z_material, z_medium)
    if transfer is None or reflect is None or ratio is None:
        return None

    score = (MATERIAL_MEDIUM_LOUDNESS_WEIGHT * reflect) + (
        MATERIAL_MEDIUM_TRANSFER_WEIGHT * transfer
    )
    return {
        "score": score,
        "score_components": {
            "loudness_component": reflect,
            "transfer_component": transfer,
            "weights": {
                "loudness": MATERIAL_MEDIUM_LOUDNESS_WEIGHT,
                "transfer": MATERIAL_MEDIUM_TRANSFER_WEIGHT,
            },
        },
        "interface_metrics": {
            "material_impedance_m_rayl": z_material,
            "medium_impedance_m_rayl": z_medium,
            "impedance_ratio": ratio,
            "transmission_coefficient": transfer,
            "reflection_coefficient": reflect,
        },
    }


def _variant_distance(candidate: dict, seed: dict) -> float:
    dz = abs(math.log((candidate["impedance"] + _EPS) / (seed["impedance"] + _EPS)))
    drho = abs(math.log((candidate["rho"] + _EPS) / (seed["rho"] + _EPS)))
    de = abs(math.log((candidate["e_mpa"] + _EPS) / (seed["e_mpa"] + _EPS)))
    dnu = abs(candidate["nu"] - seed["nu"])
    return 0.50 * dz + 0.20 * drho + 0.20 * de + 0.10 * dnu


def _expand_materials_for_medium(all_materials: list[dict], seed_materials: list[dict]) -> list[dict]:
    seed_ids = {str(m["row"].get("source_id")) for m in seed_materials}
    if not seed_materials:
        return []

    variant_candidates: list[dict] = []
    for material in all_materials:
        source_id = str(material["row"].get("source_id"))
        if source_id in seed_ids:
            continue
        min_distance = min(_variant_distance(material, seed) for seed in seed_materials)
        variant_candidates.append(
            {
                "material": material,
                "distance": min_distance,
            }
        )

    variant_candidates.sort(
        key=lambda r: (
            r["distance"],
            str(r["material"]["row"].get("material")),
            str(r["material"]["row"].get("source_id")),
        )
    )
    top_variants = [r["material"] for r in variant_candidates[:COUPLING_TOP_N]]

    expanded: dict[str, dict] = {}
    for material in seed_materials + top_variants:
        expanded[str(material["row"].get("source_id"))] = material

    return sorted(
        expanded.values(),
        key=lambda m: (
            str(m["row"].get("material")),
            str(m["row"].get("source_id")),
        ),
    )


def _build_seed_records(seed_materials: list[dict], medium_entry: dict) -> list[dict]:
    scored: list[tuple[float, dict]] = []
    for material in seed_materials:
        result = _material_medium_score(material, medium_entry)
        if result is None:
            continue
        payload = _build_material_payload({
            "row": material["row"],
            "family": _material_family(material["row"]),
        })
        scored.append((result["score"], {**payload, "radiation": result["interface_metrics"]}))

    scored.sort(
        key=lambda t: (
            -t[0],
            str(t[1]["material"]),
            str(t[1]["source_id"]),
        )
    )
    return [r for _, r in scored]


def _select_seed_materials_diverse(scored_materials: list[dict]) -> list[dict]:
    selected: list[dict] = []
    family_usage: Counter[str] = Counter()
    remaining = list(scored_materials)

    while remaining and len(selected) < COUPLING_TOP_N:
        best_index = None
        best_score = None
        best_tiebreak = None

        for family_overflow_allowance in range(COUPLING_TOP_N + 1):
            best_index = None
            best_score = None
            best_tiebreak = None

            for index, row in enumerate(remaining):
                material = row.get("material") or {}
                material_name = str((material.get("row") or {}).get("material"))
                source_id = str((material.get("row") or {}).get("source_id"))
                family = _material_family(material.get("row") or {})

                if family_usage[family] >= MATERIALS_A_GREEDY_MAX_PER_FAMILY + family_overflow_allowance:
                    continue

                base_score = float(row.get("score") or 0.0)
                family_novelty = 1.0 / (1.0 + family_usage[family])
                adjusted_score = base_score + MATERIALS_A_GREEDY_FAMILY_NOVELTY_WEIGHT * family_novelty

                tiebreak = (material_name, source_id)
                if best_score is None or adjusted_score > best_score or (
                    adjusted_score == best_score and tiebreak < best_tiebreak
                ):
                    best_index = index
                    best_score = adjusted_score
                    best_tiebreak = tiebreak

            if best_index is not None:
                break

        if best_index is None:
            break

        selected_row = remaining.pop(best_index)
        family = _material_family((selected_row.get("material") or {}).get("row") or {})
        family_usage[family] += 1
        selected.append(selected_row)

    return selected


def _build_materials_b_for_seed(
    seed_material: dict,
    expanded_materials: list[dict],
    medium_entry: dict,
    friction_table: "FrictionTable",
    prefer_lubricated: bool,
) -> list[dict]:
    seed_id = str((seed_material.get("row") or {}).get("source_id"))
    seed_family = _material_family(seed_material.get("row") or {})
    seed_impedance = float(seed_material.get("impedance") or 0.0)

    candidates: list[dict] = []
    for counterpart in expanded_materials:
        counterpart_id = str((counterpart.get("row") or {}).get("source_id"))
        if counterpart_id == seed_id:
            continue

        counterpart_impedance = float(counterpart.get("impedance") or 0.0)
        transfer = transmission_coefficient(seed_impedance, counterpart_impedance)
        reflect = reflection_coefficient(seed_impedance, counterpart_impedance)
        ratio = impedance_ratio(seed_impedance, counterpart_impedance)
        if transfer is None or reflect is None or ratio is None:
            continue

        counterpart_family = _material_family(counterpart.get("row") or {})
        friction_coeff, friction_source = friction_table.lookup(
            seed_family, counterpart_family, prefer_lubricated
        )
        medium_score = _material_medium_score(counterpart, medium_entry)
        candidates.append(
            {
                "base_score": transfer,
                "friction_coefficient": friction_coeff,
                "medium_support_score": (medium_score or {}).get("score", 0.0),
                    "conduction": {
                    "seed_impedance_m_rayl": seed_impedance,
                    "counterpart_impedance_m_rayl": counterpart_impedance,
                    "impedance_ratio": ratio,
                    "transmission_coefficient": transfer,
                    "reflection_coefficient": reflect,
                },
                "material": _build_material_payload(
                    {
                        "row": counterpart.get("row"),
                        "family": counterpart_family,
                    }
                ),
            }
        )

    selected: list[dict] = []
    family_usage: Counter[str] = Counter()
    used_material_ids: set[str] = set()

    while candidates and len(selected) < COUPLING_TOP_N:
        best_index = None
        best_score = None
        best_row = None
        best_tiebreak = None

        for family_overflow_allowance in range(COUPLING_TOP_N + 1):
            best_index = None
            best_score = None
            best_row = None
            best_tiebreak = None

            for index, row in enumerate(candidates):
                material = row.get("material") or {}
                material_id = str(material.get("source_id"))
                family = str(material.get("family") or "unknown")

                if material_id in used_material_ids:
                    continue
                if family_usage[family] >= MATERIALS_B_GREEDY_MAX_PER_FAMILY + family_overflow_allowance:
                    continue

                base_score = float(row.get("base_score") or 0.0)
                medium_support = float(row.get("medium_support_score") or 0.0)
                family_novelty = 1.0 / (1.0 + family_usage[family])
                cross_family_bonus = 1.0 if family != seed_family else 0.0
                friction_coeff: float | None = row.get("friction_coefficient")
                # Normalise friction to (0, 1) — coefficients range ~0.02–1.6;
                # cap at 1.5 so the weight is always the true max contribution.
                friction_norm = min(friction_coeff / 1.5, 1.0) if friction_coeff is not None else 0.0
                adjusted = (
                    MATERIAL_MATERIAL_GREEDY_BASE_WEIGHT * base_score
                    + 0.15 * medium_support
                    + MATERIAL_MATERIAL_GREEDY_FAMILY_NOVELTY_WEIGHT * family_novelty
                    + MATERIAL_MATERIAL_GREEDY_CROSS_FAMILY_BONUS * cross_family_bonus
                    + FRICTION_RANKING_WEIGHT * friction_norm
                )

                tiebreak = (
                    str(material.get("material")),
                    str(material.get("source_id")),
                )
                if best_score is None or adjusted > best_score or (
                    adjusted == best_score and tiebreak < best_tiebreak
                ):
                    best_index = index
                    best_score = adjusted
                    best_row = {
                            "_score": adjusted,
                            **material,
                            "conduction": row.get("conduction"),
                            "friction_coefficient": row.get("friction_coefficient"),
                    }
                    best_tiebreak = tiebreak

            if best_index is not None:
                break

        if best_index is None or best_row is None:
            break

        chosen = candidates.pop(best_index)
        chosen_material = chosen.get("material") or {}
        chosen_id = str(chosen_material.get("source_id"))
        chosen_family = str(chosen_material.get("family") or "unknown")
        used_material_ids.add(chosen_id)
        family_usage[chosen_family] += 1
        selected.append(best_row)

    selected.sort(
        key=lambda row: (
            -row["_score"],
            str(row.get("material")),
            str(row.get("source_id")),
        )
    )
    return [{k: v for k, v in row.items() if k != "_score"} for row in selected]


def _nested_materials_a(
    seed_records: list[dict],
    seed_materials: list[dict],
    expanded_materials: list[dict],
    medium_entry: dict,
    friction_table: "FrictionTable",
    prefer_lubricated: bool,
) -> list[dict]:
    by_seed_id = {str((m.get("row") or {}).get("source_id")): m for m in seed_materials}
    nested: list[dict] = []
    for seed_record in seed_records:
        seed_id = str(seed_record.get("source_id"))
        seed_material = by_seed_id.get(seed_id)
        if seed_material is None:
            continue
        nested.append({
            **seed_record,
            "couplings": _build_materials_b_for_seed(
                seed_material, expanded_materials, medium_entry, friction_table, prefer_lubricated
            ),
        })
    return nested


def main() -> None:
    materials = _load_json(MATERIALS_JSON)
    mediums = _load_json(MEDIUMS_JSON)
    deduped_materials = _dedupe_materials_by_signature(materials)
    friction_table = FrictionTable(FRICTION_CSV)

    medium_sections: list[dict] = []
    for medium in mediums:
        prefer_lubricated = (str(medium.get("phase") or "").lower() == "liquid")

        scored_materials: list[dict] = []
        for material in deduped_materials:
            scored = _material_medium_score(material, medium)
            if scored is None:
                continue
            scored_materials.append(
                {
                    "material": material,
                    "score": scored["score"],
                }
            )

        scored_materials.sort(
            key=lambda r: (
                -r["score"],
                str(r["material"]["row"].get("material")),
                str(r["material"]["row"].get("source_id")),
            )
        )
        selected_seed_entries = _select_seed_materials_diverse(scored_materials)
        seed_materials = [r["material"] for r in selected_seed_entries]
        expanded_materials = _expand_materials_for_medium(deduped_materials, seed_materials)
        seed_records = _build_seed_records(seed_materials, medium)
        materials = _nested_materials_a(
            seed_records, seed_materials, expanded_materials, medium,
            friction_table, prefer_lubricated
        )

        medium_sections.append(
            {
                "medium": {
                    "substance": medium.get("substance"),
                    "source_id": medium.get("source_id"),
                    "phase": medium.get("phase"),
                    "properties": medium.get("properties"),
                },
                "materials": materials,
            }
        )

    write_json(MATERIAL_MATERIAL_BY_MEDIUM_JSON, medium_sections)
    print(
        f"Wrote {len(medium_sections)} medium sections -> {MATERIAL_MATERIAL_BY_MEDIUM_JSON}"
    )


if __name__ == "__main__":
    main()
