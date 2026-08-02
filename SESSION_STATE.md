# VELT — where this project actually stands

**Updated:** 2026-08-02 (session 3)
**Repository:** `~/Desktop/velt` — this folder is the whole project. Nothing
lives anywhere else.
**Phase:** Core Rust engine builds and tests green on this Mac. Toolchain
installed. UI not started.

**This folder is the project.** `~/Desktop/velt` and nowhere else. No second
copy, no other working directory. If a stale `VELT.zip` is still sitting on the
Desktop, delete it — two copies of a project is how they diverge.

---

## Read this first

If you have lost the thread, this section is the answer.

**What exists:** six Rust crates, 2,853 lines, 50 tests — all 50 passing on
this machine. The money primitives, the underwriting engine, the provenance
tracer, the Fair Housing connector boundary, the snapshot store, and an HTTP
daemon. This is the hard half of the product and it is real code, not
scaffolding.

**What does not exist:** any user interface. The Electron shell and the React
terminal are two README files that say "not implemented." No live data source
is connected — no HUD figures, no listings, no comps. VELT cannot currently
show you a property.

**What is blocking you:** nothing structural any more. The toolchain is
installed, the workspace compiles clean, and the tests pass. What remains is
building things that do not exist yet.

**Honest one-line summary:** a correct, verified engine with no face and no
data.

---

## Start here

The toolchain is already installed (session 3). If you are on a fresh machine,
run `bash scripts/bootstrap-macos.sh` first — no administrator rights needed.

```bash
cd ~/Desktop/velt
source ~/.zshrc   # only needed in a terminal opened before the bootstrap ran
just doctor       # confirms this shell can see cargo, node, pnpm, just
just setup        # finishes the JS half; creates pnpm-lock.yaml
just ci           # every doctrine gate
```

**Run the two `just` commands separately, not as `just setup && just ci`.** With
`&&`, a failure in `setup` silently skips `ci`, which reads as though `ci` were
the thing that failed.

`just setup` did not reach its `pnpm install` step on the first attempt —
`pnpm` was not on the PATH of that shell. Recipes inherit the PATH of the shell
that invoked `just`, and the bootstrap writes its PATH block to `~/.zshrc`,
which an already-open session never re-reads. `~/.cargo/bin` was present from an
earlier rustup install, which is why `cargo` worked and `node`/`pnpm` did not.
`just doctor` diagnoses exactly this, and `setup` now prints the fix instead of
`command not found`.

Until `pnpm install` completes, `node_modules/` and `pnpm-lock.yaml` do not
exist, the TypeScript client has never been typechecked, and the JS side of the
build is not reproducible. Commit `pnpm-lock.yaml` when it appears.

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

**Superseded 2026-08-02, session 3: the workspace now builds and tests green on
this Mac.** `cargo nextest` reported *50 tests run: 50 passed, 0 skipped* across
6 binaries, after a clean 38.69s compile under 1.97.1 / aarch64-apple-darwin.
The Mac column below is no longer inherited.

| Crate | Purpose | Tests | Linux container | This Mac |
|---|---|---:|:---:|:---:|
| `velt-money` | integer minor units, `Bps`, one rounding policy | 10 | ✅ | ✅ |
| `velt-provenance` | `Traced<T>`, trace tree, source rollup | 4 | ✅ | ✅ |
| `velt-engine` | underwriting + fixed-point amortization | 27 | ✅ | ✅ |
| `velt-connector` | trust tiers, Fair Housing filter, rights posture | 7 | ✅ | ✅ |
| `velt-store` | SQLite WAL, immutable snapshots, pointer flip | 2 | ✅ | ✅ |
| `velt-daemon` | axum on loopback, utoipa OpenAPI | — | ✅ | builds |
| | | **50** | | **50/50** |

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
| Compiles clean, zero warnings, clippy included | ✅ **confirmed on this Mac** — `fmt-check` and `clippy -D warnings` both green |
| Tests pass, snapshot-to-the-cent on financial paths | ✅ **50/50 on this Mac**; fixtures also independently re-verified |
| External data has rights posture + trust tier | ✅ enforced by trait signature — no live connector exists yet |
| Computed values carry provenance | ✅ the engine can only return `Traced` |
| OpenAPI ↔ TypeScript drift check passes | ⬜ not yet reached — `deps` fails before `drift` runs |
| It runs | ⬜ daemon compiles; not yet exercised over HTTP here |
| Committed to the repo | ⚠️ committed locally, **never pushed** |

Two gates went green on this machine in session 3. Two remain unreached because
`just ci` orders `deps` before `openapi` and `drift`, and `deps` was failing.

### The `cargo deny` failure and its fix

First real run of `just deps` on 2026-08-02 returned:

```
advisories ok, bans FAILED, licenses ok, sources ok
```

**Cause.** `[workspace.dependencies]` declared the internal crates with `path`
and no `version`. A path dependency without a version carries an implicit
requirement of `*`, and `wildcards = "deny"` — a rule aimed at external floats
like `serde = "*"` — cannot distinguish that from a local path dep, which
resolves to a directory on disk and cannot float. The gate fired on its own
workspace, not on a real reproducibility problem.

**Fixed by** stating `version = "0.1.0"` on each internal dependency, plus
`allow-wildcard-paths = true` as a scoped backstop for private crates. External
wildcards are still denied.

Also cleared in the same pass: five `license-not-encountered` warnings, by
removing allow-list entries no crate in the graph uses — including `MPL-2.0`,
which is weak copyleft and would have been a §6 question had it ever arrived.
The allow-list is now seven entries, all of them observed in the resolved graph.

**Not fixed, deliberately:** `syn` resolves at both 2.0.119 and 3.0.3. The 3.x
copy comes via `serde_derive` and `utoipa-gen`, the 2.x copy via
`tracing-attributes`, which has not migrated. Nothing here can change that, and
it clears itself upstream. Left as a warning rather than silenced with a `skip`,
because a `skip` would also hide a future duplicate that mattered.

---

## Not done — declared, not disguised

- **No user interface.** `apps/shell` and `apps/terminal` are README stubs.
- **No live connector.** The framework is built; HUD FMR, listing sources, and
  assessor data are not wired. Each needs a written rights posture first (§5).
- **`cargo deny` has now run** (and found the wildcard problem above, which is
  fixed). **`cargo machete` and `cargo mutants` still have not** — `deps` failed
  before reaching machete, and mutants is a separate recipe.
- **`pnpm install` has never completed**, so `tsc` has never typechecked the
  generated TypeScript client. The earlier `node_modules/` was deleted because
  it had been installed by npm rather than the pinned pnpm and contained a Linux
  turbo binary; the replacement has not been created yet.
- **No lockfile for the JavaScript side.** `pnpm-lock.yaml` does not exist, so
  JS builds are not reproducible the way Cargo builds are. `just setup`
  creates it — commit it when it appears.
- **Nothing is on GitHub.** The remote points at `satex25/VELT`. The earlier
  push failed because GitHub disabled password authentication over HTTPS in
  August 2021; an account password cannot work. `scripts/push-to-github.sh`
  sets up an SSH key instead.

---

## Next, in order

1. `just setup && just ci`. Two gates are green; drive the remaining ones green
   before building anything new on top.
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
