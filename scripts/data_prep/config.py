"""
Central configuration for data-preparation scripts.

Keeping constants in one place makes ranking and dataset generation easy to tune
without editing multiple scripts.
"""

from __future__ import annotations

from data_prep_utils import repo_path

# ---------------------------------------------------------------------------
# Shared numeric constants
# ---------------------------------------------------------------------------
REFERENCE_TEMPERATURE_C = 20.0
MPA_TO_PA = 1.0e6

# Coupling-ranking configuration
COUPLING_TOP_N = 25
MATERIAL_MEDIUM_LOUDNESS_WEIGHT = 0.70
MATERIAL_MEDIUM_TRANSFER_WEIGHT = 0.30
MATERIAL_SIGNATURE_IMPEDANCE_DECIMALS = 3
MATERIAL_SIGNATURE_DENSITY_DECIMALS = 3
MATERIAL_SIGNATURE_POISSON_DECIMALS = 6
MATERIAL_SIGNATURE_YOUNGS_MPA_DECIMALS = 3
MATERIAL_MATERIAL_GREEDY_MAX_MATERIAL_APPEARANCES = 1
MATERIAL_MATERIAL_GREEDY_MAX_FAMILY_EXPOSURE = 10
MATERIAL_MATERIAL_GREEDY_MAX_PAIR_FAMILY_COMBINATION = 3
MATERIAL_MATERIAL_GREEDY_BASE_WEIGHT = 0.75
MATERIAL_MATERIAL_GREEDY_FAMILY_NOVELTY_WEIGHT = 0.20
MATERIAL_MATERIAL_GREEDY_MATERIAL_NOVELTY_WEIGHT = 0.10
MATERIAL_MATERIAL_GREEDY_CROSS_FAMILY_BONUS = 0.15
MATERIALS_A_GREEDY_MAX_PER_FAMILY = 2
MATERIALS_A_GREEDY_FAMILY_NOVELTY_WEIGHT = 0.25
MATERIALS_B_GREEDY_MAX_PER_FAMILY = 2

# NIST crawler defaults
LANDING_URL = "https://webbook.nist.gov/chemistry/fluid/"
DEFAULT_P_MPA = "0.101325"
DEFAULT_T_LOW_C = "-275"
DEFAULT_T_HIGH_C = "300"
DEFAULT_T_INC_C = "15"

# ---------------------------------------------------------------------------
# Paths
# ---------------------------------------------------------------------------
MECH_CSV = repo_path(
    "scripts", "data_prep", "raw_data", "Materials and their Mechanical Properties.csv"
)
PROP_CSV = repo_path("scripts", "data_prep", "raw_data", "Material Properties.csv")
NIST_CSV = repo_path("scripts", "data_prep", "raw_data", "nist_fluid_data.csv")

MATERIALS_JSON = repo_path(
    "src-tauri", "resources", "medium-and-material", "materials.json"
)
MEDIUMS_JSON = repo_path(
    "src-tauri", "resources", "medium-and-material", "mediums.json"
)
MATERIAL_MATERIAL_BY_MEDIUM_JSON = repo_path(
    "src-tauri", "resources", "medium-and-material", "material-material-by-medium.json"
)
