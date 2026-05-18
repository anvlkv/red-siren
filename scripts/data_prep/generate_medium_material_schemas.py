#!/usr/bin/env python3
"""
Generate JSON Schema files for all medium-and-material JSON outputs.

For each *.json in src-tauri/resources/medium-and-material/ (excluding existing
*.schema.json files), this script infers a Draft 2020-12 schema and writes a
neighboring *.schema.json file.
"""

from __future__ import annotations

import json
import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))
from data_prep_utils import repo_path, write_json

SCHEMA_URI = "https://json-schema.org/draft/2020-12/schema"
OUTPUT_DIR = repo_path("src-tauri", "resources", "medium-and-material")


def _type_of(value: object) -> str:
    if value is None:
        return "null"
    if isinstance(value, bool):
        return "boolean"
    if isinstance(value, int):
        return "integer"
    if isinstance(value, float):
        return "number"
    if isinstance(value, str):
        return "string"
    if isinstance(value, list):
        return "array"
    if isinstance(value, dict):
        return "object"
    return "string"


def _canonical(schema: dict) -> str:
    return json.dumps(schema, sort_keys=True, separators=(",", ":"), ensure_ascii=False)


def _wrap_any_of(schemas: list[dict]) -> dict:
    unique: dict[str, dict] = {}
    for schema in schemas:
        key = _canonical(schema)
        unique[key] = schema

    collapsed = list(unique.values())
    if len(collapsed) == 1:
        return collapsed[0]
    collapsed.sort(key=_canonical)
    return {"anyOf": collapsed}


def _merge(a: dict, b: dict) -> dict:
    if not a:
        return b
    if not b:
        return a
    if _canonical(a) == _canonical(b):
        return a

    # Merge already-unioned schemas by flattening anyOf.
    if "anyOf" in a or "anyOf" in b:
        left = a.get("anyOf", [a])
        right = b.get("anyOf", [b])
        return _wrap_any_of([*left, *right])

    a_type = a.get("type")
    b_type = b.get("type")

    # integer + number => number
    if {a_type, b_type} == {"integer", "number"}:
        return {"type": "number"}

    if a_type != b_type:
        return _wrap_any_of([a, b])

    if a_type == "array":
        return {
            "type": "array",
            "items": _merge(a.get("items", {}), b.get("items", {})),
        }

    if a_type == "object":
        a_props = a.get("properties", {})
        b_props = b.get("properties", {})
        keys = sorted(set(a_props.keys()) | set(b_props.keys()))

        props: dict[str, dict] = {}
        for key in keys:
            if key in a_props and key in b_props:
                props[key] = _merge(a_props[key], b_props[key])
            elif key in a_props:
                props[key] = a_props[key]
            else:
                props[key] = b_props[key]

        a_required = set(a.get("required", []))
        b_required = set(b.get("required", []))
        required = sorted(a_required & b_required)

        result = {
            "type": "object",
            "properties": props,
            "additionalProperties": False,
        }
        if required:
            result["required"] = required
        return result

    return a


def _infer(value: object) -> dict:
    value_type = _type_of(value)

    if value_type in {"null", "boolean", "integer", "number", "string"}:
        return {"type": value_type}

    if value_type == "array":
        arr = value  # type: ignore[assignment]
        assert isinstance(arr, list)
        if not arr:
            return {"type": "array", "items": {}}

        item_schema: dict = {}
        for item in arr:
            item_schema = _merge(item_schema, _infer(item))
        return {"type": "array", "items": item_schema}

    obj = value  # type: ignore[assignment]
    assert isinstance(obj, dict)
    properties = {key: _infer(obj[key]) for key in sorted(obj.keys())}
    result = {
        "type": "object",
        "properties": properties,
        "additionalProperties": False,
    }
    if properties:
        result["required"] = sorted(properties.keys())
    return result


def _schema_for_file(path: Path) -> dict:
    data = json.loads(path.read_text(encoding="utf-8"))
    inferred = _infer(data)
    return {
        "$schema": SCHEMA_URI,
        "title": path.name,
        **inferred,
    }


def main() -> None:
    files = sorted(
        p
        for p in OUTPUT_DIR.glob("*.json")
        if p.is_file() and not p.name.endswith(".schema.json")
    )

    written = 0
    for src in files:
        schema = _schema_for_file(src)
        dst = src.with_name(f"{src.stem}.schema.json")
        write_json(dst, schema)
        written += 1
        print(f"Wrote schema -> {dst}")

    print(f"Generated {written} schema files in {OUTPUT_DIR}")


if __name__ == "__main__":
    main()
