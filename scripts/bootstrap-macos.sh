#!/usr/bin/env bash
# VELT toolchain bootstrap for macOS — no administrator rights required.
#
# Doctrine §7 makes `just` the sole task runner. This script is the one
# permitted exception, for the obvious reason: you cannot run `just` before
# `just` exists. It installs the toolchain and then hands over. Everything
# after this is `just`. See docs/decisions/DR-003-no-sudo-bootstrap.md.
#
# Homebrew is deliberately NOT used. Homebrew writes to /opt/homebrew and
# requires sudo on a managed Mac. Every tool VELT needs installs into your home
# directory instead:
#
#   rustup  -> ~/.rustup, ~/.cargo
#   node    -> ~/.local/node
#   pnpm    -> ~/Library/pnpm
#   just    -> ~/.cargo/bin   (via cargo)
#
# Usage:  bash scripts/bootstrap-macos.sh
# Safe to re-run; every step is idempotent.

set -euo pipefail

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
NODE_PREFIX="$HOME/.local/node"
STEP=0

step()  { STEP=$((STEP + 1)); printf '\n\033[1m[%d] %s\033[0m\n' "$STEP" "$1"; }
ok()    { printf '  \033[32m✓\033[0m %s\n' "$1"; }
warn()  { printf '  \033[33m!\033[0m %s\n' "$1"; }
die()   { printf '  \033[31m✗ %s\033[0m\n' "$1" >&2; exit 1; }

printf '\033[1mVELT toolchain bootstrap\033[0m\n'
printf 'Repository: %s\n' "$REPO_ROOT"
printf 'No sudo required. Nothing is written outside your home directory.\n'

# ---------------------------------------------------------------------------
step "Checking prerequisites"
# ---------------------------------------------------------------------------

case "$(uname -s)" in
  Darwin) ok "macOS $(sw_vers -productVersion 2>/dev/null || echo '')" ;;
  *) die "This script is macOS-only. Detected: $(uname -s)" ;;
esac

ARCH="$(uname -m)"
case "$ARCH" in
  arm64)  NODE_ARCH="darwin-arm64" ;;
  x86_64) NODE_ARCH="darwin-x64" ;;
  *) die "Unsupported architecture: $ARCH" ;;
esac
ok "architecture $ARCH"

# Rust needs a linker. On macOS that comes from the Xcode Command Line Tools.
# These install without admin rights on most machines; if this Mac is managed
# and blocks it, that is the one thing that must be requested from whoever
# administers it.
if xcode-select -p >/dev/null 2>&1; then
  ok "Xcode Command Line Tools present ($(xcode-select -p))"
else
  warn "Xcode Command Line Tools missing — Rust cannot link without them."
  warn "A GUI installer will open. Complete it, then re-run this script."
  xcode-select --install || true
  exit 1
fi

# ---------------------------------------------------------------------------
step "Installing Rust (rustup)"
# ---------------------------------------------------------------------------

if [ -x "$HOME/.cargo/bin/rustup" ]; then
  ok "rustup already installed"
else
  # --no-modify-path: we manage the PATH block ourselves at the end, so
  # re-running never appends a duplicate line to the shell profile.
  curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs \
    | sh -s -- -y --no-modify-path --default-toolchain none
  ok "rustup installed"
fi

export PATH="$HOME/.cargo/bin:$PATH"
command -v rustup >/dev/null 2>&1 || die "rustup not on PATH after install"

# rust-toolchain.toml pins the exact version and components. Installing from
# inside the repo makes rustup honour that pin rather than pulling `stable`,
# which is the whole point of the pin.
( cd "$REPO_ROOT" && rustup show active-toolchain >/dev/null 2>&1 ) || true
( cd "$REPO_ROOT" && rustup toolchain install )
ok "toolchain: $(cd "$REPO_ROOT" && rustc --version 2>/dev/null || echo 'pending')"

# ---------------------------------------------------------------------------
step "Installing Node.js"
# ---------------------------------------------------------------------------

if [ -x "$NODE_PREFIX/bin/node" ]; then
  ok "node already installed ($("$NODE_PREFIX/bin/node" --version))"
else
  TMP="$(mktemp -d)"
  trap 'rm -rf "$TMP"' EXIT

  # Resolve the current LTS from the official index rather than hardcoding a
  # version that goes stale. package.json requires >=20.
  #
  # Downloaded to a file rather than piped into a parser. Node is the thing
  # being installed, so it does not exist yet on a first run; piping into it
  # kills the pipe and makes curl report a write failure (error 56) before the
  # fallback parser ever runs. The download is fine, the error is noise, and
  # noise in a bootstrap is indistinguishable from a real fault.
  echo "  resolving current Node LTS from nodejs.org…"
  curl -fsSL https://nodejs.org/dist/index.json -o "$TMP/index.json" \
    || die "could not reach nodejs.org"

  # python3 is guaranteed here: it ships with the Xcode Command Line Tools,
  # which step 1 has already confirmed are installed.
  NODE_VER="$(python3 -c '
import json, sys
with open(sys.argv[1]) as fh:
    releases = json.load(fh)
print(next(r["version"] for r in releases if r["lts"]))
' "$TMP/index.json")" || die "could not parse the Node release index"

  [ -n "$NODE_VER" ] || die "could not resolve a Node LTS version"
  echo "  installing Node $NODE_VER ($NODE_ARCH)"

  TARBALL="node-${NODE_VER}-${NODE_ARCH}.tar.gz"
  curl -fsSL "https://nodejs.org/dist/${NODE_VER}/${TARBALL}" -o "$TMP/$TARBALL"

  # Verify against the official checksum file before extracting anything.
  curl -fsSL "https://nodejs.org/dist/${NODE_VER}/SHASUMS256.txt" -o "$TMP/SHASUMS256.txt"
  EXPECTED="$(grep " ${TARBALL}\$" "$TMP/SHASUMS256.txt" | awk '{print $1}')"
  ACTUAL="$(shasum -a 256 "$TMP/$TARBALL" | awk '{print $1}')"
  [ -n "$EXPECTED" ] || die "no checksum published for $TARBALL"
  [ "$EXPECTED" = "$ACTUAL" ] || die "checksum mismatch for $TARBALL — refusing to install"
  ok "checksum verified"

  mkdir -p "$NODE_PREFIX"
  tar -xzf "$TMP/$TARBALL" -C "$NODE_PREFIX" --strip-components=1
  ok "node $NODE_VER installed to $NODE_PREFIX"
fi

export PATH="$NODE_PREFIX/bin:$PATH"

# ---------------------------------------------------------------------------
step "Installing pnpm"
# ---------------------------------------------------------------------------

# corepack ships with Node and reads the "packageManager" field in
# package.json, so pnpm lands on the exact pinned version rather than whatever
# is newest. That pin is a reproducibility guarantee, same as the Rust one.
if command -v corepack >/dev/null 2>&1; then
  corepack enable --install-directory "$NODE_PREFIX/bin" >/dev/null 2>&1 \
    || corepack enable >/dev/null 2>&1 || true
  ( cd "$REPO_ROOT" && corepack prepare --activate >/dev/null 2>&1 ) || true
  ok "pnpm via corepack: $(cd "$REPO_ROOT" && pnpm --version 2>/dev/null || echo 'activates on first use')"
else
  warn "corepack not found; falling back to the standalone pnpm installer"
  curl -fsSL https://get.pnpm.io/install.sh | sh -
  export PNPM_HOME="$HOME/Library/pnpm"
  export PATH="$PNPM_HOME:$PATH"
  ok "pnpm installed"
fi

# ---------------------------------------------------------------------------
step "Installing just"
# ---------------------------------------------------------------------------

if command -v just >/dev/null 2>&1; then
  ok "just already installed ($(just --version))"
else
  # cargo-binstall fetches a prebuilt binary instead of compiling from source:
  # seconds rather than minutes, and it is needed by `just setup` regardless.
  cargo install cargo-binstall --locked 2>/dev/null || true
  if command -v cargo-binstall >/dev/null 2>&1; then
    cargo binstall -y just
  else
    cargo install just --locked
  fi
  ok "just installed ($(just --version))"
fi

# ---------------------------------------------------------------------------
step "Wiring your shell PATH"
# ---------------------------------------------------------------------------

PROFILE="$HOME/.zshrc"
MARKER="# >>> VELT toolchain >>>"

if [ -f "$PROFILE" ] && grep -qF "$MARKER" "$PROFILE"; then
  ok "PATH block already present in ~/.zshrc"
else
  cat >> "$PROFILE" <<'PATHBLOCK'

# >>> VELT toolchain >>>
export PATH="$HOME/.cargo/bin:$PATH"
export PATH="$HOME/.local/node/bin:$PATH"
export PNPM_HOME="$HOME/Library/pnpm"
export PATH="$PNPM_HOME:$PATH"
# <<< VELT toolchain <<<
PATHBLOCK
  ok "appended PATH block to ~/.zshrc"
fi

# ---------------------------------------------------------------------------
step "Verifying"
# ---------------------------------------------------------------------------

FAILED=0
for tool in rustc cargo node just; do
  if command -v "$tool" >/dev/null 2>&1; then
    ok "$(printf '%-7s' "$tool") $($tool --version 2>&1 | head -1)"
  else
    warn "$tool NOT on PATH"
    FAILED=1
  fi
done

if command -v pnpm >/dev/null 2>&1; then
  ok "$(printf '%-7s' pnpm) $(pnpm --version 2>&1 | head -1)"
else
  warn "pnpm activates on first use inside the repo (corepack)"
fi

printf '\n'
if [ "$FAILED" -eq 0 ]; then
  printf '\033[32m\033[1mBootstrap complete.\033[0m\n\n'
  printf 'Open a NEW terminal window (so the PATH takes effect), then:\n\n'
  printf '    cd %s\n' "$REPO_ROOT"
  printf '    just setup     # installs the cargo-* gate tools + pnpm install\n'
  printf '    just ci        # runs every doctrine gate\n\n'
  printf 'A green `just ci` is the first time the Definition of Done (§9) has\n'
  printf 'ever been confirmed on this machine.\n'
else
  printf '\033[33mBootstrap finished with warnings.\033[0m Open a new terminal and re-run\n'
  printf 'this script; the PATH block only takes effect in a fresh shell.\n'
fi
