#!/usr/bin/env bash
# Creates the VELT GitHub repository and pushes this scaffold to it.
#
# Run this once, on a machine where you are signed in to the GitHub CLI.
# It is idempotent: if the remote already exists it just pushes.
#
#   ./scripts/create-github-repo.sh              # private repo named VELT
#   ./scripts/create-github-repo.sh myorg/VELT   # explicit owner/name

set -euo pipefail

REPO="${1:-VELT}"
VISIBILITY="${VELT_REPO_VISIBILITY:-private}"

cd "$(dirname "$0")/.."

if ! command -v gh >/dev/null 2>&1; then
  echo "error: GitHub CLI (gh) not found."
  echo "  macOS:  brew install gh"
  echo "  then:   gh auth login"
  exit 1
fi

if ! gh auth status >/dev/null 2>&1; then
  echo "error: not signed in. Run: gh auth login"
  exit 1
fi

if [ ! -d .git ]; then
  git init -b main
  git add -A
  git commit -m "VELT Day 1 scaffold"
fi

if git remote get-url origin >/dev/null 2>&1; then
  echo "remote 'origin' already set: $(git remote get-url origin)"
else
  echo "creating $VISIBILITY repository: $REPO"
  gh repo create "$REPO" \
    --"$VISIBILITY" \
    --source=. \
    --remote=origin \
    --description "Local-first, keyboard-driven terminal for real estate investment analysis." \
    --disable-wiki
fi

git push -u origin main
echo
echo "✓ pushed. Repository: $(gh repo view --json url -q .url)"
