# DR-003 — No-sudo toolchain bootstrap; SSH for GitHub

**Status:** accepted
**Date:** 2026-08-02
**Supersedes:** nothing. Corrects an unstated assumption in DR-001 and DR-002.

## Context

DR-001 and DR-002 specified the toolchain but never specified how it gets onto
a developer's machine. The repository implicitly assumed Homebrew — the README
said `brew install gh`, and `scripts/create-github-repo.sh` exits early unless
`gh` is present.

That assumption is false on the only machine VELT currently has to run on.
Attempting to install Homebrew there produced:

```
Sorry, user ciaragrace may not run sudo on Kens-MacBook.
```

The account is not in the sudoers file. Homebrew installs into `/opt/homebrew`,
which requires it. So the documented path to a working toolchain did not exist,
and consequently nothing in this repository had ever been compiled on the
target machine — every green test result came from a disposable Linux
container.

A second, unrelated failure compounded this. `git push` was attempted with an
account password:

```
remote: Invalid username or token. Password authentication is not supported.
fatal: Authentication failed for 'https://github.com/satex25/VELT.git'
```

GitHub disabled password authentication for git over HTTPS on 2021-08-13. The
error message is misleading — the password is read correctly, it is simply no
longer an accepted credential — so it invites retrying, which can never work.

## Decision

**1. Homebrew is not a dependency of this project.** Every tool VELT needs
installs into the user's home directory without elevation:

| Tool | Installs to | Mechanism |
|---|---|---|
| Rust | `~/.rustup`, `~/.cargo` | `rustup` shell installer |
| Node | `~/.local/node` | official tarball, SHA-256 verified against `SHASUMS256.txt` |
| pnpm | `~/Library/pnpm` | `corepack`, which ships inside Node |
| `just` | `~/.cargo/bin` | `cargo binstall` |
| `cargo-*` gate tools | `~/.cargo/bin` | `cargo binstall`, already in `just setup` |

`scripts/bootstrap-macos.sh` performs this and is idempotent.

**2. `scripts/bootstrap-macos.sh` is the second permitted exception to "just is
the sole task runner" (§7).** The justification is not stylistic: `just` is one
of the things the script installs, so the rule cannot apply to its own
precondition. The exception is bounded — the script installs a toolchain and
stops. It builds nothing, tests nothing, and is never invoked by another
recipe.

**3. GitHub authentication is SSH, not HTTPS.** `scripts/push-to-github.sh`
generates an ed25519 key if none exists, prints the public half for pasting
into GitHub's web UI, and rewrites `origin` from HTTPS to SSH. A personal
access token over HTTPS would also work; SSH is chosen because it does not
expire, so it cannot silently break a workflow months from now.

**4. `scripts/create-github-repo.sh` is retained but is no longer the
documented path.** It remains correct on a machine with `gh` installed and
authenticated, and deleting a working script to make a point is not an
improvement.

## Consequences

- The toolchain is now installable on the target machine. The four Definition
  of Done gates that were unconfirmed there become reachable.
- Node is pinned by checksum verification at install time, which is marginally
  stronger than what Homebrew would have given.
- The Xcode Command Line Tools remain a hard prerequisite — Rust cannot link
  without a system linker. These normally install without admin rights via
  `xcode-select --install`. If this machine blocks that too, it is the single
  thing that must be escalated to whoever administers it. The bootstrap script
  detects this and stops with that message rather than failing later inside
  `cargo build` with a confusing linker error.
- `~/.zshrc` gains a delimited `# >>> VELT toolchain >>>` block. Delimited so
  re-running the script cannot append duplicates, and so it can be removed in
  one edit.

## What this does not change

No architectural decision in §7 is affected. Rust remains the source of truth,
`just` remains the task runner for all real work, the monorepo layout is
untouched, and no commercial model is foreclosed — a bootstrap script is not a
distribution mechanism (§6, T4 has not fired).
