#!/usr/bin/env bash
set -euo pipefail

# Why: One simple, dependable script to generate macOS (.icns) and Windows (.ico)
# icons from the canonical Icon.png used by this project. Keep it small and clear.

# Run from anywhere; operate relative to this script.
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
ICONS_DIR="${SCRIPT_DIR}/../icons"

INPUT="${ICONS_DIR}/Icon.png"
ICNS_OUT="${ICONS_DIR}/icon.icns"
ICO_OUT="${ICONS_DIR}/icon.ico"
ICONSET_DIR="${ICONS_DIR}/App.iconset"

if [[ ! -f "${INPUT}" ]]; then
  echo "Error: '${INPUT}' not found." >&2
  echo "Place your source PNG at: ${ICONS_DIR}/Icon.png" >&2
  exit 1
fi

echo "Generating icons from: ${INPUT}"

produced_any=0

# --- .icns (macOS) via sips + iconutil ---
if command -v sips >/dev/null 2>&1 && command -v iconutil >/dev/null 2>&1; then
  rm -rf "${ICONSET_DIR}"
  mkdir -p "${ICONSET_DIR}"

  # Keep the canonical size set including Retina (@2x) variants up to 1024.
  for size in 16 32 128 256 512; do
    sips -z "${size}" "${size}" "${INPUT}" --out "${ICONSET_DIR}/icon_${size}x${size}.png" >/dev/null
    sips -z $((size * 2)) $((size * 2)) "${INPUT}" --out "${ICONSET_DIR}/icon_${size}x${size}@2x.png" >/dev/null
  done

  iconutil -c icns "${ICONSET_DIR}" -o "${ICNS_OUT}"
  echo "Wrote ${ICNS_OUT}"
  produced_any=1
else
  echo "Skipping .icns: requires macOS tools 'sips' and 'iconutil'."
fi

# --- .ico (Windows) via ImageMagick ---
IM_CMD=""
if command -v magick >/dev/null 2>&1; then
  IM_CMD="magick"
elif command -v convert >/dev/null 2>&1; then
  IM_CMD="convert"
fi

if [[ -n "${IM_CMD}" ]]; then
  "${IM_CMD}" "${INPUT}" -background none -define icon:auto-resize=256,128,64,48,32,16 "${ICO_OUT}"
  echo "Wrote ${ICO_OUT}"
  produced_any=1
else
  echo "Skipping .ico: ImageMagick not found."
  echo "Install: macOS -> brew install imagemagick | Ubuntu -> sudo apt-get install imagemagick"
fi

# Clean up temporary iconset (only used for .icns generation)
rm -rf "${ICONSET_DIR}"

if [[ "${produced_any}" -eq 0 ]]; then
  echo "No outputs produced (missing dependencies)." >&2
  exit 1
fi

echo "Done."
