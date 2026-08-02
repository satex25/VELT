#!/usr/bin/env bash
# Push VELT to GitHub without the GitHub CLI.
#
# Why this exists: scripts/create-github-repo.sh needs `gh`, `gh` is normally
# installed with Homebrew, and Homebrew needs administrator rights this machine
# does not have. This script needs nothing but `git`, which ships with the
# Xcode Command Line Tools.
#
# It also fixes the actual cause of the earlier failure. GitHub disabled
# password authentication for git over HTTPS on 2021-08-13. Typing your account
# password at the prompt cannot work and never will; the error it produces
# ("Invalid username or token") is misleading because the password *is* being
# read, it is just no longer an accepted credential.
#
# Two credentials do work:
#
#   SSH key            — recommended. No expiry, nothing to paste on each push.
#   Personal access token — an HTTPS password replacement, with an expiry date.
#
# Usage:  bash scripts/push-to-github.sh [owner/repo]
# Default owner/repo comes from the existing origin remote.

set -euo pipefail

cd "$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"

ok()   { printf '  \033[32m✓\033[0m %s\n' "$1"; }
warn() { printf '  \033[33m!\033[0m %s\n' "$1"; }
die()  { printf '\033[31m✗ %s\033[0m\n' "$1" >&2; exit 1; }
hdr()  { printf '\n\033[1m%s\033[0m\n' "$1"; }

command -v git >/dev/null 2>&1 || die "git not found"
[ -d .git ] || die "not a git repository: $(pwd)"

# ---------------------------------------------------------------------------
# Work out where we are pushing.
# ---------------------------------------------------------------------------

if [ $# -ge 1 ]; then
  SLUG="$1"
elif git remote get-url origin >/dev/null 2>&1; then
  SLUG="$(git remote get-url origin \
    | sed -E 's#^(https://github\.com/|git@github\.com:)##; s#\.git$##')"
else
  die "No origin remote and no owner/repo argument. Try: bash scripts/push-to-github.sh youruser/VELT"
fi

OWNER="${SLUG%%/*}"
NAME="${SLUG##*/}"
BRANCH="$(git symbolic-ref --short HEAD 2>/dev/null || echo main)"

hdr "Target"
ok "repository  $OWNER/$NAME"
ok "branch      $BRANCH"
ok "commits     $(git rev-list --count HEAD)"

# ---------------------------------------------------------------------------
hdr "Step 1 — does the repository exist on GitHub?"
# ---------------------------------------------------------------------------

HTTP="$(curl -s -o /dev/null -w '%{http_code}' "https://api.github.com/repos/$OWNER/$NAME" || echo 000)"
case "$HTTP" in
  200) ok "$OWNER/$NAME exists and is public (or you are seeing a cached public view)" ;;
  404)
    warn "$OWNER/$NAME does not exist, or exists and is private."
    cat <<EOF

  If you have not created it yet, do that first — it takes about twenty
  seconds and needs no tooling:

    1. Open  https://github.com/new
    2. Owner:            $OWNER
       Repository name:  $NAME
       Visibility:       Private        <- VELT handles private deal flow
    3. Do NOT tick "Add a README", ".gitignore", or "license".
       This repository already has commits; an initialising commit on the
       remote would create a divergent history you would have to reconcile.
    4. Click "Create repository", then re-run this script.

  If it already exists as a private repository, ignore the above and carry on.

EOF
    ;;
  *) warn "GitHub API returned HTTP $HTTP — could not check. Continuing." ;;
esac

# ---------------------------------------------------------------------------
hdr "Step 2 — credential"
# ---------------------------------------------------------------------------

if ssh -o StrictHostKeyChecking=accept-new -o ConnectTimeout=10 -T git@github.com 2>&1 | grep -q "successfully authenticated"; then
  ok "SSH key already authorised with GitHub"
  METHOD=ssh
elif ls "$HOME"/.ssh/id_ed25519.pub >/dev/null 2>&1; then
  warn "An SSH key exists but GitHub has not accepted it."
  printf '\n  Add this public key at https://github.com/settings/ssh/new\n\n'
  printf '\033[36m%s\033[0m\n\n' "$(cat "$HOME/.ssh/id_ed25519.pub")"
  printf '  Then re-run this script.\n'
  exit 1
else
  warn "No SSH key found."
  printf '\n  Generating one now (no administrator rights required).\n'
  printf '  Press Enter three times to accept the default path and no passphrase.\n\n'
  ssh-keygen -t ed25519 -C "$OWNER@velt" -f "$HOME/.ssh/id_ed25519"
  printf '\n  Add this public key at https://github.com/settings/ssh/new\n'
  printf '  Give it any title, leave the type as "Authentication Key".\n\n'
  printf '\033[36m%s\033[0m\n\n' "$(cat "$HOME/.ssh/id_ed25519.pub")"
  printf '  Then re-run this script.\n'
  exit 1
fi

# ---------------------------------------------------------------------------
hdr "Step 3 — remote"
# ---------------------------------------------------------------------------

TARGET_URL="git@github.com:$OWNER/$NAME.git"
CURRENT_URL="$(git remote get-url origin 2>/dev/null || echo '')"

if [ "$CURRENT_URL" = "$TARGET_URL" ]; then
  ok "origin already set to SSH"
elif [ -n "$CURRENT_URL" ]; then
  git remote set-url origin "$TARGET_URL"
  ok "origin switched from HTTPS to SSH"
  printf '      was: %s\n      now: %s\n' "$CURRENT_URL" "$TARGET_URL"
else
  git remote add origin "$TARGET_URL"
  ok "origin added: $TARGET_URL"
fi

# ---------------------------------------------------------------------------
hdr "Step 4 — push"
# ---------------------------------------------------------------------------

if git push -u origin "$BRANCH"; then
  printf '\n\033[32m\033[1m✓ pushed.\033[0m  https://github.com/%s/%s\n' "$OWNER" "$NAME"
else
  die "push failed — the message above says why"
fi
