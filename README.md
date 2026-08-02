# VELT

Local-first, keyboard-driven desktop terminal for real estate investment analysis.
Houses, apartments, and land, including overseas acquisition. HUD Fair Market Rent
is the headline metric for Section 8 / voucher-eligible properties.

Status: **Day 1 scaffold.** The underwriting engine, money primitives, provenance
tracer, connector boundary, snapshot store, and daemon are implemented and tested.
The Electron shell and React terminal UI are stubs.

## Quick start

```bash
just ci        # every doctrine gate: fmt, lint, test, openapi, drift
just daemon    # run the loopback daemon
just test      # tests, including snapshot-to-the-cent financial fixtures
```

`just` is the sole entry point (doctrine §7). There are no npm scripts, no
Makefiles, and no shell scripts invoked directly except `scripts/create-github-repo.sh`,
which runs once.

## Layout

```
crates/
  velt-money        integer minor units; f64 forbidden by clippy
  velt-provenance   Traced<T> — every number carries its derivation
  velt-engine       pure underwriting: no I/O, no clock, no randomness
  velt-connector    the only entry point for external data; Fair Housing boundary
  velt-store        SQLite WAL; immutable snapshots + current-pointer flip
  velt-daemon       axum on loopback; utoipa-generated OpenAPI
apps/
  terminal          React terminal UI          (stub)
  shell             Electron host              (stub)
packages/
  api-client        TypeScript client, generated from openapi.json
```

## How the constraints are enforced

These are not conventions. Each is a build failure.

| Doctrine | Mechanism | Verify |
|---|---|---|
| Money is integer minor units; no `f64` in financial paths | `clippy.toml` `disallowed-types` + `float_arithmetic = deny` | add an `f64` anywhere, run `just lint` |
| Engine is pure — no clock | `clippy.toml` `disallowed-methods` bans `Utc::now`, `SystemTime::now` | `just lint` |
| Every computed number carries provenance | engine returns `Traced<T>`; an untraced number is unrepresentable | `crates/velt-engine/src/tests.rs` |
| Every external datum carries source / tier / timestamp / confidence | `Datum::new` has no partial constructor | type signature |
| Fair Housing — no demographic or proxy scoring | `FairHousingFilter` runs inside `Connector::ingest`; undeclared forbidden fields are a hard error | `velt-connector` tests |
| Data rights checked before scraping | `Connector` cannot be implemented without a `RightsPosture` | trait signature |
| Immutable results, current-pointer flip | `Store::put_snapshot` inserts and flips in one transaction; no update path | `velt-store` tests |
| Rust is the single source of truth | `just drift` regenerates OpenAPI + TS client and fails on any diff | CI `gate` job |

## Numerical correctness

Doctrine §2 puts numerical correctness above profit. Financial fixtures are
asserted exactly — no tolerance windows — against values produced by an
independent 60-digit decimal implementation:

| Case | Expected |
|---|---|
| $250,000 @ 7.00% / 360mo | $1,663.26 /mo |
| $400,000 @ 6.50% / 360mo | $2,528.27 /mo |
| $100,000 @ 5.00% / 180mo | $790.79 /mo |
| Balance, $250k @ 7% after 60 payments | $235,328.71 |

Amortization compounds in i128 fixed point at 1e12 and rounds exactly once.

## Commercial model

**CMR-OPEN** (doctrine §6). No architecture here forecloses subscription,
perpetual license, or proprietary-edge. Entitlement is not yet implemented;
when it is, it goes behind a single trait with a `NullEntitlement` default.
See `docs/decisions/DR-001-scaffold.md`.
