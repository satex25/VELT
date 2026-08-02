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
ci-full: ci mutants coverage
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

# Install every tool the gates above depend on.
setup:
    cargo install cargo-binstall
    cargo binstall -y cargo-nextest cargo-insta cargo-mutants cargo-deny \
                      bacon cargo-llvm-cov cargo-hakari cargo-machete sccache
    pnpm install

clean:
    cargo clean
    rm -rf node_modules apps/*/node_modules packages/*/node_modules .turbo target/coverage
