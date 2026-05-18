# Red Siren

Red Siren is a noise chime. It pulls the present into focus—a siren's call, loud, brief, true.


## Requirements

- [Tauri CLI](https://v2.tauri.app/start/prerequisites/): `cargo install tauri-cli`
- [Trunk](https://trunkrs.dev/): `cargo install trunk`
- Node dependencies: `npm install`
- Rust targets: `rustup target add wasm32-unknown-unknown`

### Mobile targets (optional)
```bash
rustup target add aarch64-apple-ios x86_64-apple-ios aarch64-apple-ios-sim
rustup target add aarch64-linux-android armv7-linux-androideabi i686-linux-android x86_64-linux-android
```

## Data Preparation

The JSON resource files bundled with the app are generated from raw CSVs under
`scripts/data_prep/raw_data/`.  Run the following scripts from the repository root
to regenerate them (standard library only, no extra dependencies):

```bash
# Generate src-tauri/resources/medium-and-material/materials.json
# Input: scripts/data_prep/raw_data/Materials and their Mechanical Properties.csv
#        scripts/data_prep/raw_data/Material Properties.csv
python3 scripts/data_prep/prepare_materials_json.py

# Generate src-tauri/resources/medium-and-material/mediums.json
# Input: scripts/data_prep/raw_data/nist_fluid_data.csv
python3 scripts/data_prep/prepare_mediums_json.py

# Generate src-tauri/resources/medium-and-material/material-medium.json
# Input: generated materials.json + mediums.json
# Score: 70% loudness mismatch + 30% energy transfer
python3 scripts/data_prep/prepare_material_medium_json.py

# Generate src-tauri/resources/medium-and-material/material-material-by-medium.json
# Input: generated materials.json + mediums.json
# Pipeline: per-medium seeds (top N) -> per-medium variant expansion (N) -> per-medium material-material top N
python3 scripts/data_prep/prepare_material_material_by_medium_json.py

# Generate JSON Schemas for all medium-and-material output JSON files
python3 scripts/data_prep/generate_medium_material_schemas.py

# Tune ranking weights/top-N and crawler defaults in scripts/data_prep/config.py
```

To re-crawl the NIST fluid data (requires `playwright`):

```bash
python3 scripts/data_prep/crawl_nist_webbook.py
```

## Commands

```bash
# Development
cargo tauri dev

# Build
cargo tauri build

# iOS
cargo tauri ios dev
cargo tauri ios build

# Android
cargo tauri android dev
cargo tauri android build
```

---

This work is licensed under a
[Creative Commons Attribution-ShareAlike 4.0 International License][cc-by-sa].

[![CC BY-SA 4.0][cc-by-sa-image]][cc-by-sa]

[cc-by-sa]: http://creativecommons.org/licenses/by-sa/4.0/
[cc-by-sa-image]: https://licensebuttons.net/l/by-sa/4.0/88x31.png

---

This software was developed with assistance from AI coding tools: GitHub Copilot, Claude.

While AI suggestions were used during development, all code has been reviewed, tested, and modified by human developers. Users are responsible for verifying the code meets their requirements.
