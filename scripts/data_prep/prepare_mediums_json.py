#!/usr/bin/env python3
"""
prepare_mediums_json.py — Generate mediums.json from NIST fluid data.

Input
-----
  raw_data/nist_fluid_data.csv
      NIST WebBook isobaric fluid data harvested by crawl_nist_webbook.py.
      Relevant columns:
        Substance         — chemical/common name
        ID                — NIST CAS-based ID
        Temperature (C)   — temperature in °C
        Pressure (MPa)    — pressure in MPa  → converted × 1e6 to Pa
        Density (kg/m3)   — density in kg/m³
        Sound Spd. (m/s)  — speed of sound in m/s
        Viscosity (Pa*s)  — dynamic viscosity in Pa·s
        Phase             — "liquid" | "gas" | …

Output
------
  src-tauri/resources/medium-and-material/mediums.json
      JSON array; one record per distinct Substance, chosen as the row with
      the maximum computed acoustic impedance (density × speed_of_sound).

      Record shape:
        {
          "substance": "Water",
          "source_id": "C7732185",
          "phase":     "liquid",
          "properties": {
            "temperature_c":           20.0,
            "pressure_pa":             101325.0,
            "density_kg_per_m3":       998.2,
            "speed_of_sound_m_per_s":  1482.4,
            "viscosity_pa_s":          0.001001,
            "impedance_m_rayl":        1480350.48
          }
        }

Usage
-----
    python scripts/data_prep/prepare_mediums_json.py

The script is idempotent; re-running overwrites the output file deterministically.

Selection rationale
-------------------
Choosing the row with maximum acoustic impedance = density × speed_of_sound
naturally selects the most "dense/fast" state of each substance (liquid phase
at high pressure), which is the physically interesting operating point for a
musical-acoustics application.  Rows with any missing or non-numeric required
field are discarded before selection.
"""

import csv
import sys
from pathlib import Path

# ---------------------------------------------------------------------------
# Allow running from any working directory.
# ---------------------------------------------------------------------------
sys.path.insert(0, str(Path(__file__).resolve().parent))
from config import MPA_TO_PA, MEDIUMS_JSON, NIST_CSV
from data_prep_utils import parse_float, write_json


# ---------------------------------------------------------------------------
# Main
# ---------------------------------------------------------------------------
def main() -> None:
    # best_by_substance[substance] = {impedance, record_dict}
    best_by_substance: dict[str, dict] = {}

    with NIST_CSV.open(newline="", encoding="utf-8-sig") as fh:
        reader = csv.DictReader(fh)
        for row in reader:
            substance = row.get("Substance", "").strip()
            if not substance:
                continue

            # --- required numeric fields ---
            temperature_c = parse_float(row.get("Temperature (C)"))
            pressure_mpa = parse_float(row.get("Pressure (MPa)"))
            density = parse_float(row.get("Density (kg/m3)"))
            speed = parse_float(row.get("Sound Spd. (m/s)"))
            viscosity = parse_float(row.get("Viscosity (Pa*s)"))

            # Discard rows with any missing required field.
            if any(v is None for v in (temperature_c, pressure_mpa, density, speed, viscosity)):
                continue

            pressure_pa = pressure_mpa * MPA_TO_PA  # type: ignore[operator]
            impedance = density * speed  # type: ignore[operator]

            current_best = best_by_substance.get(substance)
            if current_best is None or impedance > current_best["impedance"]:
                best_by_substance[substance] = {
                    "impedance": impedance,
                    "record": {
                        "substance": substance,
                        "source_id": row.get("ID", "").strip() or None,
                        "phase": row.get("Phase", "").strip() or None,
                        "properties": {
                            "temperature_c": temperature_c,
                            "pressure_pa": pressure_pa,
                            "density_kg_per_m3": density,
                            "speed_of_sound_m_per_s": speed,
                            "viscosity_pa_s": viscosity,
                            "impedance_m_rayl": impedance,
                        },
                    },
                }

    # Build output list sorted by substance name for determinism.
    records = [v["record"] for v in sorted(best_by_substance.values(), key=lambda x: x["record"]["substance"])]

    write_json(MEDIUMS_JSON, records)
    print(f"Wrote {len(records)} medium records → {MEDIUMS_JSON}")


if __name__ == "__main__":
    main()
