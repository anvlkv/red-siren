#!/usr/bin/env bash
set -euo pipefail

# ====== CONFIG (edit as desired) ======
REPO="anvlkv/red-siren"          # Your GitHub repo
KEYSTORE_FILE="red-siren-test-release.jks"
ALIAS="redSirenTest"             # ANDROID_KEY_ALIAS
STOREPASS="changeit_store"       # ANDROID_KEYSTORE_PASSWORD
KEYPASS="changeit_key"           # ANDROID_KEY_PASSWORD (can be same as STOREPASS)
DNAME="CN=Red Siren Test,O=Red Siren,L=Remote,ST=NA,C=US"
VALID_DAYS=10000
KEYALG="RSA"
KEYSIZE=2048

# ====== ARG PARSING ======
# -f : force regeneration (overwrite existing keystore file)
FORCE=0
while getopts ":f" opt; do
  case "$opt" in
    f) FORCE=1 ;;
    *) echo "Usage: $0 [-f]"; exit 1 ;;
  esac
done

# ====== 1. Generate keystore (self-signed) ======
# keytool comes with Java (JDK). Ensure 'keytool -help' works first.
echo "Preparing keystore ${KEYSTORE_FILE} ..."
if [[ -f "${KEYSTORE_FILE}" ]]; then
  if [[ $FORCE -eq 1 ]]; then
    echo "-f supplied: removing existing keystore ${KEYSTORE_FILE}"
    rm -f "${KEYSTORE_FILE}"
  else
    echo "Keystore ${KEYSTORE_FILE} already exists. Use -f to force regeneration."
    echo "Re-using existing keystore; skipping generation step."
  fi
fi

if [[ ! -f "${KEYSTORE_FILE}" ]]; then
  echo "Generating keystore ${KEYSTORE_FILE} ..."
  keytool -genkeypair \
    -storetype PKCS12 \
    -keystore "${KEYSTORE_FILE}" \
    -alias "${ALIAS}" \
    -storepass "${STOREPASS}" \
    -keypass "${KEYPASS}" \
    -keyalg "${KEYALG}" \
    -keysize "${KEYSIZE}" \
    -validity "${VALID_DAYS}" \
    -dname "${DNAME}"

  echo "Keystore generated:"
  keytool -list -v -keystore "${KEYSTORE_FILE}" -storepass "${STOREPASS}" -alias "${ALIAS}" | grep -E 'Alias name:|Entry type:|Valid from:'
else
  echo "Keystore metadata (existing):"
  keytool -list -v -keystore "${KEYSTORE_FILE}" -storepass "${STOREPASS}" -alias "${ALIAS}" | grep -E 'Alias name:|Entry type:|Valid from:' || true
fi

# ====== 2. Base64 encode keystore for GitHub secret ======
# Ensure no line breaks on all platforms
if base64 --help 2>&1 | grep -q -- "-w "; then
  B64_CONTENT="$(base64 -w0 "${KEYSTORE_FILE}")"
else
  B64_CONTENT="$(base64 < "${KEYSTORE_FILE}" | tr -d '\n')"
fi

# ====== 3. Create / update GitHub secrets with gh ======
# Requires: gh auth login (with repo rights)
echo "Pushing secrets to GitHub repo ${REPO} ..."
printf "%s" "${B64_CONTENT}" | gh secret set ANDROID_KEYSTORE_BASE64 -R "${REPO}" --body -
gh secret set ANDROID_KEYSTORE_PASSWORD -R "${REPO}" --body "${STOREPASS}"
gh secret set ANDROID_KEY_ALIAS -R "${REPO}" --body "${ALIAS}"
gh secret set ANDROID_KEY_PASSWORD -R "${REPO}" --body "${KEYPASS}"

echo "Secrets set. (GitHub won’t show values back for security.)"

# ====== 4. (Optional) Local test of decoding (mirrors workflow step) ======
echo "Testing decode round-trip..."
echo "${B64_CONTENT}" | base64 --decode > /tmp/roundtrip.jks
cmp "${KEYSTORE_FILE}" /tmp/roundtrip.jks && echo "Round-trip OK."

cat <<EOF

Done.

Tip: Force regenerate locally:
  ./scripts/keystore.sh -f

EOF
