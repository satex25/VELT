# VELT — where this project actually stands

**Updated:** 2026-08-02 (session 3)
**Repository:** `~/Desktop/velt` — this folder is the whole project. Nothing
lives anywhere else.
**Phase:** Core Rust engine written and tested. Toolchain not yet installed on
this Mac. UI not started.

---

## Read this first

If you have lost the thread, this section is the answer.

**What exists:** six Rust crates, 2,853 lines, 50 tests. The money primitives,
the underwriting engine, the provenance tracer, the Fair Housing connector
boundary, the snapshot store, and an HTTP daemon. This is the hard half of the
product and it is real code, not scaffolding.

**What does not exist:** any user interface. The Electron shell and the React
terminal are two README files that say "not implemented." No live data source
is connected — no HUD figures, no listings, no comps. VELT cannot currently
show you a property.

**What is blocking you:** the Rust toolchain is not installed on this Mac, so
nothing here can be compiled or run locally. That is fixable in about ten
minutes and does not require administrator rights. See "Start here" below.

**Honest one-line summary:** a correct, well-built engine with no face and no
data, on a machine that cannot yet compile it.

---

## Start here

Run these in Terminal, in order.

```bash
cd ~/Desktop/velt
bash scripts/bootstrap-macos.sh   # installs Rust, Node, pnpm, just — no sudo
```

Then open a **new** Terminal window and run:

```bash
cd ~/Desktop/velt
just setup                        # cargo-nextest, cargo-deny, bacon, pnpm install
just ci                           # every doctrine gate, on this machine
```

A green `just ci` is the first time the Definition of Done (§9) will have been
confirmed on hardware you own. Until then, treat the test results below as
inherited, not observed.

To put the code on GitHub:

```bash
bash scripts/push-to-github.sh
```

---

## Verification status — read the two columns separately

The distinction below matters and was previously blurred. Everything in this
repository was built and tested inside a Linux container, not on this Mac. The
container is gone. Its build output has been deleted from `target/` because it
was `aarch64-unknown-linux-gnu` and macOS `cargo` cannot use a byte of it.

| Crate | Purpose | Tests | Verified in Linux container | Verified on this Mac |
|---|---|---:|:---:|:---:|
| `velt-money` | integer minor units, `Bps`, one rounding policy | 10 | ✅ | ⬜ |
| `velt-provenance` | `Traced<T>`, trace tree, source rollup | 4 | ✅ | ⬜ |
| `velt-engine` | underwriting + fixed-point amortization | 27 | ✅ | ⬜ |
| `velt-connector` | trust tiers, Fair Housing filter, rights posture | 7 | ✅ | ⬜ |
| `velt-store` | SQLite WAL, immutable snapshots, pointer flip | 2 | ✅ | ⬜ |
| `velt-daemon` | axum on loopback, utoipa OpenAPI | — | ✅ | ⬜ |
| | | **50** | | |

### Independently re-verified 2026-08-02

The financial fixtures were recomputed from scratch against a 60-digit decimal
implementation, with no reference to the Rust code. All four match to the cent:

| Case | Expected | Recomputed |
|---|---|---|
| $250,000 @ 7.00% / 360mo | $1,663.26 | $1,663.26 ✅ |
| $400,000 @ 6.50% / 360mo | $2,528.27 | $2,528.27 ✅ |
| $100,000 @ 5.00% / 180mo | $790.79 | $790.79 ✅ |
| Balance, $250k @ 7% after 60 payments | $235,328.71 | $235,328.71 ✅ |

Static audit of the same date: no `f64` or `f32` in any financial path, no
`todo!`, no `unimplemented!`, no `panic!`, no TODO or FIXME comments, and every
`unwrap()`/`expect()` in the tree is inside a `#[cfg(test)]` module. The
`#[test]` count is exactly 50, matching the claim.

### Definition of Done (§9)

| Gate | Status |
|---|---|
| Compiles clean, zero warnings, clippy included | ⚠️ passed in the container; **unconfirmed on this Mac** |
| Tests pass, snapshot-to-the-cent on financial paths | ⚠️ 50/50 in the container; fixtures independently re-verified |
| External data has rights posture + trust tier | ✅ enforced by trait signature — no live connector exists yet |
| Computed values carry provenance | ✅ the engine can only return `Traced` |
| OpenAPI ↔ TypeScript drift check passes | ⚠️ generated in the container; re-run `just drift` here |
| It runs | ❌ **not on this machine.** No toolchain installed. |
| Committed to the repo | ⚠️ committed locally, **never pushed** |

Four of seven gates are unconfirmed on this Mac. That is the honest count, and
`bash scripts/bootstrap-macos.sh` followed by `just ci` closes all four.

---

## Not done — declared, not disguised

- **No user interface.** `apps/shell` and `apps/terminal` are README stubs.
- **No live connector.** The framework is built; HUD FMR, listing sources, and
  assessor data are not wired. Each needs a written rights posture first (§5).
- **`cargo deny`, `cargo machete`, `cargo mutants` have never executed.** The
  configs are syntactically valid and the licence audit was done directly
  against `cargo metadata`, but the tools themselves have not run. `just deps`
  is the first real test of that.
- **`pnpm install` has never run**, so `tsc` has never typechecked the generated
  TypeScript client. `node_modules/` was deleted: it had been installed by npm
  rather than the pinned pnpm, and contained a Linux turbo binary.
- **No lockfile for the JavaScript side.** `pnpm-lock.yaml` does not exist, so
  JS builds are not yet reproducible the way Cargo builds are. `just setup`
  creates it — commit it.
- **Nothing is on GitHub.** The remote points at `satex25/VELT`. The earlier
  push failed because GitHub disabled password authentication over HTTPS in
  August 2021; an account password cannot work. `scripts/push-to-github.sh`
  sets up an SSH key instead.

---

## Next, in order

1. `bash scripts/bootstrap-macos.sh`, then `just setup && just ci`. Nothing else
   matters until every gate is green on hardware you control.
2. `bash scripts/push-to-github.sh` — get the work off a single laptop.
3. `just mutants` — the first mutation run on the engine. Every surviving
   mutant is a missing test. Fix them before building anything on top.
4. First real connector. HUD Fair Market Rent is the headline metric, so it is
   the obvious target. **Write the rights posture before the connector code**
   (§5).
5. The terminal UI, against the generated client. This is where VELT stops
   being an engine and starts being a product you can look at.

---

## CMR (§6)

**CMR-OPEN.** Triggers T1–T5 have not fired. No auth, billing, entitlement,
multi-tenant storage, or paid data contract exists.

**T6 (Day 90) runs from 2026-08-02 → fires 2026-10-31.**

Licence audit 2026-08-02: 153 resolved packages, zero crates whose only
available licence is copyleft. `r-efi` is tri-licensed and taken under MIT.
No foreclosure has occurred; models A, B and C all remain live. `deny.toml` is
the standing gate that keeps it that way, and `just deps` is how it is checked.
