"""
Shared helpers for Red Siren data-preparation scripts.

Provides:
  - repo_path()   — resolve a path relative to the repository root
  - parse_float() — robust float parsing that returns None on failure
  - write_json()  — deterministic JSON output with stable formatting
  - normalize_name() — lower-case, punctuation-stripped name for cross-CSV matching
"""

import json
import re
import unicodedata
from pathlib import Path

# The repository root is two directories above this file:
#   scripts/data_prep/data_prep_utils.py  ->  ../../  == repo root
_REPO_ROOT = Path(__file__).resolve().parents[2]


def repo_path(*parts: str) -> Path:
    """Return an absolute Path inside the repository root.

    Example::

        repo_path("src-tauri", "resources", "medium-and-material")
    """
    return _REPO_ROOT.joinpath(*parts)


def parse_float(value: object) -> float | None:
    """Convert *value* to float; return ``None`` for blank, ``'N/A'``, or
    anything that cannot be parsed as a finite number.
    """
    if value is None:
        return None
    s = str(value).strip()
    if not s or s.lower() in {"n/a", "na", "-", "—", "null", "none"}:
        return None
    try:
        f = float(s)
    except ValueError:
        return None
    import math
    if not math.isfinite(f):
        return None
    return f


def parse_friction_range(value: object) -> float | None:
    """Parse a friction coefficient value that may be a range or a single number.

    Examples::

        parse_friction_range("0.5 - 0.8")  # → 0.65  (midpoint)
        parse_friction_range("0.4")         # → 0.4
        parse_friction_range("")            # → None
        parse_friction_range("0.3 - ")      # → 0.3   (partial range, use available end)

    Returns the midpoint for a ``"lo - hi"`` range, or the single value
    otherwise.  Returns ``None`` when the value is blank or unparseable.
    """
    import re
    import math

    if value is None:
        return None
    s = str(value).strip()
    if not s or s.lower() in {"n/a", "na", "-", "—", "null", "none"}:
        return None

    # Match a range like "0.5 - 0.8" or "0.5-0.8"
    range_match = re.match(
        r"^\s*([0-9]*\.?[0-9]+)\s*[-–]\s*([0-9]*\.?[0-9]+)\s*$", s
    )
    if range_match:
        lo = parse_float(range_match.group(1))
        hi = parse_float(range_match.group(2))
        if lo is not None and hi is not None:
            mid = (lo + hi) / 2.0
            return mid if math.isfinite(mid) else None
        if lo is not None:
            return lo
        if hi is not None:
            return hi
        return None

    # Match a trailing-dash partial range like "0.3 - "
    partial_match = re.match(r"^\s*([0-9]*\.?[0-9]+)\s*[-–]\s*$", s)
    if partial_match:
        return parse_float(partial_match.group(1))

    return parse_float(s)


def write_json(path: Path, data: object) -> None:
    """Write *data* as indented JSON to *path*, creating parent directories as
    needed.  Overwrites any existing file so repeated runs are deterministic.

    Keys are sorted to keep diffs stable across reruns.
    """
    path.parent.mkdir(parents=True, exist_ok=True)
    text = json.dumps(data, indent=2, sort_keys=True, ensure_ascii=False)
    path.write_text(text + "\n", encoding="utf-8")


def normalize_name(name: str) -> str:
    """Return a canonical lower-case token string suitable for fuzzy name
    matching across different CSV sources.

    Steps:
    1. Unicode-normalise (NFKD) and strip combining characters.
    2. Lower-case.
    3. Remove parentheses and their contents (e.g. ``(Al2O3)``).
    4. Replace hyphens/underscores with spaces.
    5. Collapse runs of whitespace.
    6. Strip leading/trailing whitespace.
    """
    # Step 1: unicode normalise
    nfkd = unicodedata.normalize("NFKD", name)
    ascii_only = "".join(c for c in nfkd if not unicodedata.combining(c))
    # Step 2: lower
    s = ascii_only.lower()
    # Step 3: remove parenthetical suffixes
    s = re.sub(r"\(.*?\)", "", s)
    # Step 4: hyphens/underscores → space
    s = s.replace("-", " ").replace("_", " ")
    # Step 5-6: collapse whitespace
    return " ".join(s.split())
