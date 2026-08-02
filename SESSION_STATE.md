# VELT — session state

**Updated:** 2026-08-02 (session 2)
**Phase:** Day 1 scaffold complete, toolchain modernised. Build mode (doctrine §3).

---

## Where things stand

### Done and verified by execution

| Crate | State | Tests |
|---|---|---|
| `velt-money` | integer minor units, `Bps`, one rounding policy | 10 |
| `velt-provenance` | `Traced<T>`, `Trace` tree, source rollup, renderer | 4 |
| `velt-engine` | full underwrite + fixed-point amortization | 27 |
| `velt-connector` | trust tiers, `Datum`, Fair Housing filter, rights posture | 7 |
| `velt-store` | SQLite WAL, immutable snapshots, pointer flip | 2 |
| `velt-daemon` | axum on loopback, utoipa OpenAPI, `--openapi` | serving |

**50 tests, 0 failures. Clippy clean at `-D warnings`. Re-verified under Rust
1.97.1 / edition 2024.**

### Session 2 — toolchain modernisation (DR-002)

| Change | Verified |
|---|---|
| Edition 2021 → **2024**, resolver 2 → **3**, MSRV 1.83 → **1.85** | `cargo fix --edition` produced a zero-line diff |
| Toolchain pinned to **1.97.1** (was floating `stable`) | builds clean |
| rustfmt `style_edition = "2024"` | `cargo fmt --check` clean after reformat |
| `deny.toml` — advisories, permissive-only licences, banned crates, source pinning | TOML valid; licence audit run against `cargo metadata` |
| `bacon.toml` — background compiler, engine-only fast job | TOML valid |
| `.config/nextest.toml` — CI profile, JUnit, zero-retry on financial paths | TOML valid |
| `.cargo/mutants.toml` — mutation scope limited to engine + money | TOML valid |
| `.cargo/config.toml` — aliases; mold/wild left commented (Linux-only) | TOML valid |
| Dev profile: deps at `opt-level = 2`, `debug = 1` | builds clean |
| `justfile` — `deps`, `mutants`, `coverage`, `snapshots`, `setup`, `ci-full` | — |
| CI — nextest + deny + machete + a separate mutants job; actions/node/pnpm bumped | YAML valid |

### Definition of Done (§9) — status

| Gate | Status |
|---|---|
| Compiles clean, zero warnings, clippy included | ✅ verified |
| Tests pass, snapshot-to-the-cent on financial paths | ✅ 50/50, exact assertions |
| New external data has rights posture + trust tier | ✅ enforced by trait signature; no live connector yet |
| New computed values carry provenance | ✅ engine returns `Traced` only |
| OpenAPI ↔ TypeScript drift check passes | ✅ regenerated, no drift |
| It runs | ✅ daemon exercised over HTTP (session 1) |
| Committed to the repo | ✅ — **push to `satex25/VELT` still pending, see below** |

### Not done — declared, not disguised

- **Electron shell** and **React terminal UI** are `README` stubs.
- **No live connector exists.** The framework is built; HUD FMR, listing
  sources, and assessor data are not wired. Each needs a written rights posture
  first (doctrine §5).
- **`cargo deny`, `cargo machete`, `cargo mutants` have not been executed.**
  The binaries are installed on the Mac, not in the environment where this tree
  was edited. Configs are syntactically valid and the licence audit was done
  directly against `cargo metadata`. First `just ci` on the Mac confirms.
- **`pnpm install` has not run here**, so `tsc` has not typechecked the
  generated client.

---

## Pick up here

1. `cd ~/Developer/VELT && just setup && just ci` — confirm every gate green on
   your machine. Expect `just deps` to be the first thing that has ever run.
2. `git push -u origin main` — the remote is wired, nothing is on GitHub yet.
3. `just mutants` — first mutation run on the engine. Every survivor is a
   missing test. Fix them before building anything on top.
4. First real connector. HUD FMR is the headline metric, so it is the obvious
   target. **Write the rights posture before the connector code** (§5).
5. Then the terminal UI against the generated client.

---

## The gap you should know about

**The 27-document v2 blueprint is not in this project's knowledge base.** The
`docs/` and `files/` folders synced from the VELT project are empty; only the
custom instructions and `memory.md` came across. This scaffold was built from
doctrine §5, §7 and §9 plus first-principles underwriting. Nothing was invented
and labelled as blueprint.

You chose to proceed without recovering it. If that changes, drop the documents
in `docs/blueprint/` — the repo is now the durable home for them, not a chat
transcript.

---

## CMR (§6)

**CMR-OPEN.** Triggers T1–T5 have not fired. No auth, billing, entitlement,
multi-tenant storage, or paid data contract exists.

**T6 (Day 90) starts 2026-08-02 → fires 2026-10-31.**

Licence audit 2026-08-02: 153 resolved packages, zero crates whose only
available licence is copyleft. `r-efi` is tri-licensed and taken under MIT.
**No foreclosure has occurred; A, B and C all remain live.** `deny.toml` is now
the standing gate that keeps it that way.
