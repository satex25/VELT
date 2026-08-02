# VELT — session state

**Updated:** 2026-08-02
**Phase:** Day 1 scaffold complete. Build mode (doctrine §3).

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

**50 tests, 0 failures. `cargo clippy --workspace --all-targets -- -D warnings`
is clean.** The daemon was run and exercised: `/health` returns ok,
`POST /underwrite` returns the fixture figures exactly, a zero price returns 422.

The `f64` ban was verified negatively — inserting an `f64` into the engine fails
the build with the doctrine §5 citation, then was reverted.

### Definition of Done (§9) — status

| Gate | Status |
|---|---|
| Compiles clean, zero warnings, clippy included | ✅ verified |
| Tests pass, snapshot-to-the-cent on financial paths | ✅ 50/50, exact assertions |
| New external data has rights posture + trust tier | ✅ enforced by trait signature; no live connector yet |
| New computed values carry provenance | ✅ engine returns `Traced` only |
| OpenAPI ↔ TypeScript drift check passes | ⚠️ script written and run; not yet CI-verified (no `pnpm install` here) |
| It runs | ✅ daemon exercised over HTTP |
| Committed to the repo | ✅ git initialized, committed locally |

### Not done — declared, not disguised

- **Electron shell** and **React terminal UI** are `README` stubs.
- **No live connector exists.** The framework is built; HUD FMR, listing
  sources, and assessor data are not wired. Each needs a written rights posture
  first (doctrine §5).
- **`pnpm install` has not run** in this environment, so `tsc` has not typechecked
  the generated client. It is syntactically generated from the real spec.

---

## The gap you should know about

**The 27-document v2 blueprint is not in this project's knowledge base.** The
`docs/` and `files/` folders synced from the VELT project are empty; only the
custom instructions and `memory.md` came across.

This scaffold was therefore built from the doctrine in the project instructions
— which is complete on architecture (§7), constraints (§5), and Definition of
Done (§9) — plus first-principles underwriting. Nothing was invented and
labelled as blueprint. Where the blueprint specifies something this scaffold got
wrong, the blueprint wins on domain scope; doctrine §3 says reality wins on
implementation detail.

**Action for you:** upload the 27 documents to the VELT project knowledge, or
drop them in `docs/blueprint/` in this repo.

---

## Pick up here

1. `cd velt && just ci` — confirm all gates green on your machine.
2. `./scripts/create-github-repo.sh` — creates the repo and pushes (needs `gh auth login`).
3. First real connector. HUD FMR is the headline metric, so it is the obvious
   first target. Write the rights posture before the connector code.
4. Then the terminal UI against the generated client.

## CMR (§6)

**CMR-OPEN.** Triggers T1–T5 have not fired. No auth, billing, entitlement,
multi-tenant storage, or paid data contract exists. **T6 (Day 90) starts
2026-08-02 → fires 2026-10-31.**
