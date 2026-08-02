# VELT task runner.
# Doctrine §7: `just` is the sole task runner. No npm scripts as entry points,
# no shell scripts invoked directly, no Makefiles. Every gate below is also a
# CI step, so a green `just ci` locally means a green pipeline.

set shell := ["bash", "-uc"]

_default:
    @just --list

# Full gate. This is the Definition of Done (doctrine §9) in executable form.
ci: fmt-check lint test openapi drift
    @echo "✓ all gates passed"

# Build everything.
build:
    cargo build --workspace
    pnpm -r build

# Run the test suite, including snapshot tests on financial paths.
test:
    cargo test --workspace --all-targets

# Clippy at the doctrine settings. `-D warnings` makes the f64 ban a hard gate.
lint:
    cargo clippy --workspace --all-targets -- -D warnings

fmt:
    cargo fmt --all

fmt-check:
    cargo fmt --all -- --check

# Regenerate the OpenAPI contract from the Rust source of truth (doctrine §7).
openapi:
    cargo run -q -p velt-daemon -- --openapi > openapi.json

# Regenerate the TypeScript client from openapi.json.
client:
    node scripts/gen-client.mjs

# Blocking drift gate: the checked-in contract and client must match the Rust.
# If this fails, run `just openapi client` and commit the result.
drift: openapi client
    @git diff --exit-code -- openapi.json packages/api-client/src/generated.ts \
      || (echo "✗ OpenAPI/TypeScript drift — run 'just openapi client' and commit" && exit 1)
    @echo "✓ no drift"

# Run the daemon on loopback.
daemon:
    cargo run -p velt-daemon

# Run the terminal UI against a running daemon.
dev:
    pnpm --filter @velt/terminal dev

# Remove build artifacts.
clean:
    cargo clean
    rm -rf node_modules apps/*/node_modules packages/*/node_modules .turbo
