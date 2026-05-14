#!/usr/bin/env bash
# Build the Surface website with Genereto, then post-process generated
# HTML so doc-internal links work on the web (the markdown sources
# carry .md-style links for in-repo reading).
#
# Requires `genereto` on $PATH. Pass --bin to point at a specific
# binary (useful when running from a build dir).

set -euo pipefail

GENERETO=genereto
PROJECT_PATH="$(cd "$(dirname "$0")" && pwd)"
REPO_URL_BASE="https://github.com/"  # override via $SURFACE_REPO_URL

if [ -n "${SURFACE_REPO_URL:-}" ]; then
  REPO_URL_BASE="${SURFACE_REPO_URL%/}/blob/main"
fi

for arg in "$@"; do
  case "$arg" in
    --bin=*) GENERETO="${arg#--bin=}";;
    --bin)   shift; GENERETO="$1";;
  esac
done

rm -rf "$PROJECT_PATH/output"

"$GENERETO" --project-path "$PROJECT_PATH"

cd "$PROJECT_PATH/output"

# (1) Rewrite doc-internal .md hrefs to .html so cross-page nav works.
for f in *.html; do
  # href="something.md"  →  href="something.html"
  sed -i -E 's|href="([^"]+)\.md"|href="\1.html"|g' "$f"
  # href="something.md#anchor" → href="something.html#anchor"
  sed -i -E 's|href="([^"#]+)\.md#|href="\1.html#|g' "$f"
done

# (2) Rewrite repo-relative paths that escape the site into the repo browser.
# These come from the canonical docs/*.md files (e.g. ../TODO.md, ../examples/).
for f in *.html; do
  sed -i -E "s|href=\"\.\./TODO\.html\"|href=\"${REPO_URL_BASE}/TODO.md\"|g" "$f"
  sed -i -E "s|href=\"\.\./examples/\"|href=\"${REPO_URL_BASE}/examples/\"|g" "$f"
  sed -i -E "s|href=\"\.\./examples/([^\"]+)\"|href=\"${REPO_URL_BASE}/examples/\1\"|g" "$f"
done

echo "Built: $PROJECT_PATH/output/"
