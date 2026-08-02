# VELT

Local-first, keyboard-driven desktop terminal for real estate investment
analysis. Houses, apartments, and land, including overseas acquisition. HUD Fair
Market Rent is the headline metric for Section 8 / voucher-eligible properties.

Rust edition 2024, toolchain pinned to 1.97.1. MIT licensed.

---

## Where this actually stands

VELT is an engine, not yet a product. The distinction is load-bearing, so it is
stated plainly rather than buried:

| Area | Status |
|---|---|
| Money primitives, underwriting engine, amortization | **Done and verified** |
| Provenance tracing on every computed number | **Done** |
| Fair Housing boundary, trust tiers, rights posture | **Done** — no live source wired to it |
| Snapshot store (SQLite WAL, pointer flip) | **Implemented and tested, called by nothing** |
| HTTP daemon (`/health`, `/underwrite`) | **Serves** — but persists nothing |
| Live data (HUD FMR, listings, comps) | **Not started** |
| Electron shell, React terminal UI | **Not started** — both are README stubs |

In one line: *a correct, verified engine with no face and no data.* You cannot
currently use VELT to look at a property.

`SESSION_STATE.md` is the long-form version and is kept honest.

## Verification

Every gate below runs on `just ci`, and all of them pass:

| Signal | Result |
|---|---|
| Tests | **65 / 65** passing |
| Mutation testing | **86 viable mutants, 84 detected** — both survivors have a written proof of equivalence |
| Clippy | clean at `-D warnings`, with `f64` banned outright in financial paths |
| Dependency policy | advisories, licences, bans and sources all clean; zero unused dependencies |
| OpenAPI ↔ TypeScript | regenerated and diffed on every run; no drift |
| Financial fixtures | asserted exactly, against an independent 60-digit decimal implementation |

Mutation testing is the one that matters most. Snapshot tests prove the engine
matches its fixtures; they do not prove the fixtures constrain the engine.
`cargo mutants` closes that gap, and a surviving mutant is treated as a bug
report against the test suite rather than a curiosity.

## Quick start

First time on a machine — installs Rust, Node, pnpm and `just` into your home
directory. **No administrator rights required; Homebrew is not used** (DR-003):

```bash
bash scripts/bootstrap-macos.sh
```

Then, in a new terminal:

```bash
just setup     # install every tool the gates depend on (once)
just ci        # every doctrine gate: fmt, lint, test, deps, openapi, drift
just daemon    # run the loopback daemon
just watch     # bacon — background compiler, keyboard-driven
```

`just` is the sole entry point (doctrine §7). There are no npm scripts and no
Makefiles. Three shell scripts are invoked directly, each once and each for a
reason `just` cannot cover: `bootstrap-macos.sh` installs `just` itself,
`push-to-github.sh` sets up an SSH credential, and `create-github-repo.sh` is
the legacy `gh`-based path kept for machines that have it.

### The gates

| Command | What it does |
|---|---|
| `just doctor` | Which tools this shell can actually see. Run first on any "command not found". |
| `just ci` | The Definition of Done (§9). This is the gate. |
| `just ci-full` | `ci` plus mutation testing and coverage. Run before a release. |
| `just test-engine` | Tight loop on the two crates where being wrong is fatal. |
| `just mutants` | Mutation testing. A surviving mutant is a bug in the *tests*. |
| `just deps` | Advisories, licence allow-list, banned crates, unused deps. |
| `just review` | Interactive insta snapshot review. |
| `just coverage` | HTML coverage report over the workspace. |

### Toolchain

| Tool | Role |
|---|---|
| `cargo-nextest` | One process per test — enforces engine purity, not just speed |
| `cargo-insta` | Snapshot-to-the-cent fixtures |
| `cargo-mutants` | Proves the fixtures actually constrain the engine |
| `cargo-deny` | Advisories + licence policy (a §6 optionality gate) |
| `cargo-machete` | Unused-dependency sweep |
| `bacon` | Background compiler, keyboard-first |
| `cargo-llvm-cov` | Coverage floor |
| `cargo-hakari` | Workspace feature unification |
| `sccache` | CI compilation cache |

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
| Tests actually constrain the engine | `cargo mutants` on `velt-engine` + `velt-money` | CI `mutants` job |
| No copyleft crate can silently foreclose a commercial model (§6) | `deny.toml` permissive-only allow-list | `just deps` |
| Builds are reproducible | toolchain pinned in `rust-toolchain.toml`; `wildcards = "deny"` | `just deps` |

## Fair Housing

Doctrine §5 forbids demographic neighborhood scoring and any proxy metric
derived from a protected characteristic. This is enforced at the only place
external data can enter — `Connector::ingest` — by a filter that matches
forbidden field markers case-insensitively as substrings, so
`neighborhood_crime_index_2025` is caught by the `crime` entry.

The filter **errors rather than silently stripping**. A source that starts
shipping a crime index should stop the pipeline and force an explicit decision,
not quietly lose a column. Crime indices, school ratings, composite
"desirability" scores and ZIP-granularity median household income are all
refused, each with a recorded reason.

Note the limitation honestly: a substring denylist is not a complete defence. A
determined source could ship an unlisted proxy. The list is a floor, not a
proof.

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

Why this is not paranoia: a $200,000 loan at 7% over 360 months has an exact
payment of $1,330.60499…, which rounds to **$1,330.60**. A float implementation
can land on $1,330.61 — and a cent per month, compounded across a portfolio and
a 30-year hold, is the kind of quiet drift doctrine §2 calls worse than useless.

## Commercial model

**CMR (§6).** VELT is MIT licensed, which grants anyone who receives the code
the right to use, modify and sell it. That is a deliberate change from the
original `UNLICENSED` posture and it does narrow §6: a perpetual-licence or
proprietary-edge model no longer has the code itself as leverage. Subscription,
hosted service, and proprietary modules built *on top* of VELT remain fully
open, and copyright is undivided, so relicensing future work is still possible.

`deny.toml` remains the standing gate on the dependency graph: a permissive-only
allow-list, so no third-party crate can impose obligations MIT does not.

Licence audit 2026-08-02: 153 resolved packages, zero crates whose only
available licence is copyleft. `r-efi` is tri-licensed and taken under MIT.

Entitlement is not implemented; when it is, it goes behind a single trait with a
`NullEntitlement` default.

## Licence

MIT — see [`LICENSE`](LICENSE).

## Design records

- [`docs/decisions/DR-001-scaffold.md`](docs/decisions/DR-001-scaffold.md)
- [`docs/decisions/DR-002-toolchain.md`](docs/decisions/DR-002-toolchain.md)
- [`docs/decisions/DR-003-no-sudo-bootstrap.md`](docs/decisions/DR-003-no-sudo-bootstrap.md)
