# VELT task runner.
# Doctrine §7: `just` is the sole task runner. No npm scripts as entry points,
# no shell scripts invoked directly, no Makefiles. Every gate below is also a
# CI step, so a green `just ci` locally means a green pipeline.

set shell := ["bash", "-uc"]

_default:
    @just --list

# ---------------------------------------------------------------------------
# Gates
# ---------------------------------------------------------------------------

# The Definition of Done (doctrine §9) in executable form. This is the gate.
ci: fmt-check lint test deps openapi drift
    @echo "✓ all gates passed"

# Everything in `ci`, plus the slow adversarial checks. Run before a release or
# after any change to the underwriting engine.
ci-full: ci mutants-check coverage
    @echo "✓ all gates passed, including mutation and coverage"

# ---------------------------------------------------------------------------
# Build & test
# ---------------------------------------------------------------------------

build:
    cargo build --workspace
    pnpm -r build

# nextest, not `cargo test`: one process per test. The engine is required to be
# pure (doctrine §5), and shared-process tests can pass on leaked state.
# nextest does not run doctests, so those get a second pass.
test:
    cargo nextest run --workspace
    cargo test --workspace --doc

# Tight inner loop — the two crates where being wrong about money is fatal.
test-engine:
    cargo nextest run -p velt-engine -p velt-money

# Review pending insta snapshots interactively.
review:
    cargo insta review

# Reject snapshots no test references any more, so a deleted test cannot leave
# a stale fixture behind that still looks like coverage.
snapshots:
    cargo insta test --unreferenced=reject

lint:
    cargo clippy --workspace --all-targets -- -D warnings

fmt:
    cargo fmt --all

fmt-check:
    cargo fmt --all -- --check

# Background compiler. Leave this running in a split pane.
watch:
    bacon

# ---------------------------------------------------------------------------
# Dependency policy (doctrine §5 — legal integrity; §6 — CMR optionality)
# ---------------------------------------------------------------------------

# Advisories, licence allow-list, duplicate/banned crates, source pinning, plus
# unused-dependency sweep. A copyleft crate reaching the daemon binary silently
# forecloses commercial model B, so this gate protects optionality as much as
# it protects security.
deps:
    cargo deny check
    cargo machete

# Refresh the RustSec advisory database, then re-check.
audit:
    cargo deny fetch
    cargo deny check advisories

# ---------------------------------------------------------------------------
# Adversarial verification
# ---------------------------------------------------------------------------

# Mutation testing on the financial paths. A surviving mutant means the tests
# do not actually constrain the engine — it is a bug report against the suite,
# not against the code.
mutants:
    cargo mutants --no-shuffle -j4

# The gate form of `mutants`, and what CI runs. Judged against the survivors
# proven unkillable in .config/mutants-expected-survivors.txt rather than
# against zero, because `cargo mutants` exits non-zero whenever anything
# survives and two survive by construction. A job that can never pass is a job
# nobody reads.
#
# Fails when the survivor set changes in either direction. A new survivor is a
# missing test. A survivor that disappeared means the expected file is stale and
# should shrink — that is good news, but it still has to be recorded.
mutants-check jobs="2":
    #!/usr/bin/env bash
    set -uo pipefail
    expected=".config/mutants-expected-survivors.txt"

    # The run's own exit code is not the signal: it is non-zero in the normal
    # case. A missing outcomes.json is the signal, because it means the run
    # never completed — a baseline build failure rather than a survivor.
    cargo mutants --no-shuffle -j{{jobs}} || true
    if [[ ! -f mutants.out/outcomes.json ]]; then
        echo "✗ the mutation run did not complete — no mutants.out/outcomes.json" >&2
        exit 1
    fi

    actual="$(sed -E 's#:[0-9]+:[0-9]+: #: #' mutants.out/missed.txt 2>/dev/null | sort)"
    want="$(grep -vE '^[[:space:]]*(#|$)' "$expected" | sort)"

    if [[ "$actual" == "$want" ]]; then
        echo "✓ surviving mutants match the known-equivalent set ($(grep -c . <<<"$want") of them)"
        exit 0
    fi

    echo "✗ the set of surviving mutants changed" >&2
    diff --label "expected ($expected)" --label "actual (mutants.out/missed.txt)" \
         -u <(echo "$want") <(echo "$actual") >&2 || true
    echo >&2
    echo "  A new survivor is a missing test — write the test." >&2
    echo "  If you have proved one unkillable, add it to $expected with the proof." >&2
    exit 1

mutants-all:
    cargo mutants --no-shuffle -j4 --examine-globs 'crates/**/*.rs'

coverage:
    cargo llvm-cov nextest --workspace --html --output-dir target/coverage
    @echo "→ target/coverage/html/index.html"

# ---------------------------------------------------------------------------
# Contract (doctrine §7 — Rust is the single source of truth)
# ---------------------------------------------------------------------------

openapi:
    cargo run -q -p velt-daemon -- --openapi > openapi.json

client:
    node scripts/gen-client.mjs

# Blocking drift gate: the checked-in contract and client must match the Rust.
# If this fails, run `just openapi client` and commit the result.
drift: openapi client
    @git diff --exit-code -- openapi.json packages/api-client/src/generated.ts \
      || (echo "✗ OpenAPI/TypeScript drift — run 'just openapi client' and commit" && exit 1)
    @echo "✓ no drift"

# ---------------------------------------------------------------------------
# Run
# ---------------------------------------------------------------------------

daemon:
    cargo run -p velt-daemon

dev:
    pnpm --filter @velt/terminal dev

# ---------------------------------------------------------------------------
# Housekeeping
# ---------------------------------------------------------------------------

# Report which required tools are reachable from a recipe shell. Run this first
# when anything fails with "command not found": recipes inherit the PATH of the
# shell that invoked `just`, so a tool installed into a profile file the current
# session never re-read is invisible here while looking fine everywhere else.
doctor:
    #!/usr/bin/env bash
    set -uo pipefail
    printf '%-16s %s\n' TOOL WHERE
    printf '%-16s %s\n' ---- -----
    missing=0
    for t in cargo rustc just node pnpm; do
      if command -v "$t" >/dev/null 2>&1; then
        printf '%-16s %s\n' "$t" "$(command -v "$t")"
      else
        printf '%-16s \033[31mNOT FOUND\033[0m\n' "$t"
        missing=1
      fi
    done
    echo
    for t in cargo-nextest cargo-deny cargo-machete cargo-insta cargo-mutants bacon; do
      if command -v "$t" >/dev/null 2>&1; then
        printf '%-16s %s\n' "$t" ok
      else
        printf '%-16s \033[33mmissing — run: just setup\033[0m\n' "$t"
      fi
    done
    if [ "$missing" -eq 1 ]; then
      printf '\n\033[33mSomething core is missing from this shell.\033[0m\n'
      printf 'If the bootstrap has already run, this shell predates it:\n\n'
      printf '    source ~/.zshrc        # this shell, right now\n'
      printf '    (or just open a new terminal window)\n\n'
      printf 'If that does not fix it: bash scripts/bootstrap-macos.sh\n'
      exit 1
    fi
    printf '\n\033[32mAll required tools reachable.\033[0m\n'

# Install every tool the gates above depend on.
setup:
    #!/usr/bin/env bash
    set -uo pipefail

    echo "→ cargo tooling"
    cargo install cargo-binstall --locked 2>/dev/null || true
    cargo binstall -y cargo-nextest cargo-insta cargo-mutants cargo-deny \
                      bacon cargo-llvm-cov cargo-hakari cargo-machete sccache || exit 1

    echo
    echo "→ javascript workspace"

    # corepack ships inside Node and can activate the pnpm version pinned in
    # package.json without a separate install. Try that before giving up, so a
    # missing shim self-heals rather than stopping the recipe.
    if ! command -v pnpm >/dev/null 2>&1 && command -v corepack >/dev/null 2>&1; then
      corepack enable >/dev/null 2>&1 || true
      corepack prepare --activate >/dev/null 2>&1 || true
    fi

    if ! command -v pnpm >/dev/null 2>&1; then
      echo >&2
      echo "  pnpm is not reachable from this shell." >&2
      echo >&2
      if [ -x "$HOME/.local/node/bin/node" ]; then
        echo "  Node IS installed at ~/.local/node, so the bootstrap worked." >&2
        echo "  This shell was opened before the PATH block was written to" >&2
        echo "  ~/.zshrc and has never re-read it. Fix with either:" >&2
        echo >&2
        echo "      source ~/.zshrc      # this shell, right now" >&2
        echo "      (or open a new terminal window)" >&2
      else
        echo "  Node is not installed. Run:" >&2
        echo >&2
        echo "      bash scripts/bootstrap-macos.sh" >&2
      fi
      echo >&2
      echo "  Then check with: just doctor" >&2
      echo >&2
      exit 127
    fi

    pnpm install
    echo
    echo "✓ setup complete — pnpm-lock.yaml is a build input, commit it."

clean:
    cargo clean
    rm -rf node_modules apps/*/node_modules packages/*/node_modules .turbo target/coverage
