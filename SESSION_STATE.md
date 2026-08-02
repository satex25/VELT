# VELT — where this project actually stands

**Updated:** 2026-08-02 (session 4)
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

**What exists:** six Rust crates, 3,433 lines, 65 tests — all 65 passing on
this machine. The money primitives, the underwriting engine, the provenance
tracer, the Fair Housing connector boundary, the snapshot store, and an HTTP
daemon. This is the hard half of the product and it is real code, not
scaffolding.

**`just ci` is green end to end as of session 4** — every gate, including the
two that had never been reached. The daemon has now served a real underwrite
over HTTP on this machine.

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

The code is on GitHub at `satex25/VELT`, **public**, pushed over SSH in session
4. `git push` is all that is needed now; `scripts/push-to-github.sh` remains for
setting up a credential on a fresh machine.

---

## Verification status — read the two columns separately

The distinction below matters and was previously blurred. Everything in this
repository was built and tested inside a Linux container, not on this Mac. The
container is gone. Its build output has been deleted from `target/` because it
was `aarch64-unknown-linux-gnu` and macOS `cargo` cannot use a byte of it.

**Superseded 2026-08-02, session 3: the workspace now builds and tests green on
this Mac.** `cargo nextest` reported *50 tests run: 50 passed, 0 skipped* across
6 binaries, after a clean 38.69s compile under 1.97.1 / aarch64-apple-darwin.
The Mac column below is no longer inherited. (Session 4 took the suite to 65;
the table records the current counts, not the session-3 ones.)

| Crate | Purpose | Tests | Linux container | This Mac |
|---|---|---:|:---:|:---:|
| `velt-money` | integer minor units, `Bps`, one rounding policy | 19 | ✅ | ✅ |
| `velt-provenance` | `Traced<T>`, trace tree, source rollup | 4 | ✅ | ✅ |
| `velt-engine` | underwriting + fixed-point amortization | 33 | ✅ | ✅ |
| `velt-connector` | trust tiers, Fair Housing filter, rights posture | 7 | ✅ | ✅ |
| `velt-store` | SQLite WAL, immutable snapshots, pointer flip | 2 | ✅ | ✅ |
| `velt-daemon` | axum on loopback, utoipa OpenAPI | — | ✅ | serves |
| | | **65** | | **65/65** |

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
| Tests pass, snapshot-to-the-cent on financial paths | ✅ **65/65 on this Mac**; fixtures also independently re-verified |
| External data has rights posture + trust tier | ✅ enforced by trait signature — no live connector exists yet |
| Computed values carry provenance | ✅ the engine can only return `Traced` |
| OpenAPI ↔ TypeScript drift check passes | ✅ **green** — `just drift` regenerates both and reports no diff |
| It runs | ✅ **green** — daemon served `/health` and a real `/underwrite` over loopback |
| Committed to the repo | ✅ **green** — pushed to `satex25/VELT` (public); remote `main` verified at the same SHA as local, nothing unpushed |

**Every gate in `just ci` now passes on this machine (session 4).** The two that
had never been reached were blocked behind `deps`, which `just ci` orders first;
once `deps` was fixed in session 3 both turned out to pass on the first run.

`/underwrite` was exercised with a $250k / 20% down / 7% / 360mo deal and checked
by hand: NOI $19,170.00, cap rate 767bps, DSCR 1.2006, CFBT $3,202.80, annual
debt service $15,967.20. The monthly payment behind that figure is
$1,330.60499…, which rounds to $1,330.60 — a case where a float implementation
would plausibly have returned $1,330.61. Every figure carries its full
provenance tree in the response.

The one gate that is *not* in `just ci`: CI also runs `cargo test --workspace
--doc`, and the tree contains zero doctests. That step passes vacuously.

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

### The daemon does not persist

`cargo machete` ran for the first time on 2026-08-02 and reported six unused
dependencies. Five were leftovers. The sixth is a finding.

**`velt-daemon` did not use `velt-store` or `velt-connector` at all.**
`post_underwrite` computes a result, returns it over HTTP, and forgets it. The
immutable-snapshot and current-pointer machinery in `velt-store` is
implemented, tested, and called by nothing. `AppState` even documents
`engine_version` as "stamped onto every snapshot" — there are no snapshots.

Nothing VELT computes currently survives the response.

All six were removed rather than added to a `cargo-machete` ignore list.
Reserving the dependency slot would have hidden this from the only tool that
found it. `velt-store` and `velt-connector` get added back in the same commit
that first calls them.

The other five, with the design reason each removal is safe:

| Crate | Removed | Why it was safe |
|---|---|---|
| `velt-store` | `velt-engine` | `put_snapshot` is generic over `T: Serialize` and writes an opaque JSON column — decoupled from engine types on purpose |
| `velt-store` | `utoipa` | store types are not part of the HTTP contract |
| `velt-connector` | `chrono` | `Datum::fetched_at` is an RFC-3339 `String` supplied by the caller; a datetime crate here invites `Utc::now()` next to the engine, which §5 forbids |
| `velt-daemon` | `serde_json` | only ever reached through `axum::Json`, which carries its own copy |

Verified before removal: zero references to any of the six anywhere under
`crates/*/src`, including test modules and derive attributes.

### Mutation testing — first run, and what it found

`just mutants` ran for the first time on 2026-08-02: **123 mutants, 69 caught,
28 missed, 25 unviable, 1 timeout.** 70% of viable mutants caught.

The raw score was the least useful part. Triage of the 28 survivors:

| Class | Count | Action |
|---|---:|---|
| Provably equivalent — no test can kill them | 8 | Documented, not chased |
| Untested predicates (`is_zero`, `is_negative`, `Bps::is_positive`) | 13 | Tests added |
| Untested divide-by-zero guards (`Money::div_int`, `Bps::div_int`) | 2 | Tests added |
| Untested sign handling in `amort::div_round_half_away` | 3 | Killed by deletion — see below |
| Untested vacancy-rate lower bound | 2 | Tests added |

Two of those are unkillable by construction and remain so: `numer <= 0` vs
`numer < 0` is unreachable because `rem != 0` implies `numer != 0`, and
`denom <= 0` vs `denom < 0` is unreachable because `denom == 0` already
returned. Chasing them would mean writing tests that cannot fail.

**The rest of that "equivalent" classification was wrong, and session 4 corrected
it.** Three of the eight were `Currency` match arms, filed as dead because every
supported currency has exponent 2. That is true of the *reachability*, but it
buried the fact that the arms were also **wrong** — the same defect recorded two
paragraphs below as a separate finding. A mutant that survives because the
function has a latent bug is not an equivalent mutant; it is the bug reporting
itself. All three are now gone.

The count itself does not carry over: the amort dedup removed eleven mutants, so
session 4 works from 112 rather than 123, and the first confirming run found six
survivors rather than eight. Three were the `Currency` defect, one was a `pow_fp`
guard that turned out to be killable outright, and two are the genuinely
equivalent pair.

**The finding: `div_round_half_away` was implemented twice.** Identical logic in
`velt-money/src/lib.rs` and `velt-engine/src/amort.rs`, while velt-money's copy
carried the doc comment *"The one rounding primitive in VELT."* That was false.
velt-money's copy had sign tests; amort's had none — which is exactly why three
mutants survived in one twin and zero in the other.

Doctrine §5 wants rounding applied once so a change is a one-line diff. It was a
two-crate edit that could silently diverge. Fixed by making velt-money's version
`pub` with an `op` label and reducing amort's 26-line copy to a 3-line
delegation. The three mutants are gone by deletion, not by testing.

**Two other real defects surfaced:**

- **A negative vacancy rate was unproven.** The guard existed and was correct,
  but nothing tested it, so nothing would have stopped a refactor removing it.
  A negative vacancy pushes the occupancy complement above 100% — effective
  gross income would exceed scheduled rent.
- **`Currency::minor_units_per_major` had a latent wrong answer.** `3 => 1_000`
  and `_ => 1_000` were the same value, so the catch-all also returned 1,000 for
  exponent 4 or more — silently wrong by a factor of ten. Harmless while nothing
  has exponent ≠ 2; a real bug the day a currency is added. **Fixed in session
  4** — see below.

Tests 50 → 60. Every new assertion was verified against each mutation before
committing; all 22 targeted mutants are killed.

### Session 4 — the mutation run confirmed, and the `Currency` fix

Three runs, same 112 mutants, on this Mac:

| Run | Caught | Missed | Unviable | Timeout |
|---|---:|---:|---:|---:|
| Confirming session 3's tests | 80 | 6 | 25 | 1 |
| After the `Currency` fix | 82 | 3 | 26 | 1 |
| After the `pow_fp` test | **83** | **2** | 26 | 1 |

**86 viable mutants, 84 detected, and both survivors carry a one-line proof of
equivalence** (the `numer`/`denom` pair above). 83 were caught outright; the
84th is the `while exp > 0` guard in `pow_fp` turned into an infinite loop,
which the suite catches on the timeout — a detection, not a hole. Session 3's
prediction of "8 survivors, all equivalent" was close on the count and wrong on
the reason.

**The `Currency` fix closed the domain rather than patching the arm.** Changing
`_ => 1_000` to `_ => 10_000` would have moved the wrong answer from exponent 4
to exponent 5. The cause was that `exponent()` returned a `u32`, so a five-entry
lookup had to map four billion inputs and therefore needed a catch-all — and a
catch-all is a wrong answer waiting for its first caller.

`exponent()` now returns a closed `Exponent` enum, `Zero`..`Four` — ISO 4217 uses
0, 2, 3 and 4, and 1 completes the ladder rather than leaving a hole a future
currency could fall into. The scale table is exhaustive over it, so there is no
fallback to be wrong, and deleting an arm stops compiling instead of silently
returning a neighbour's scale — which is why one of the three mutants came back
`unviable` rather than `caught`. The currency-to-scale data still lives in
exactly one place, so adding JPY or KWD remains a one-line edit.

Three tests pin it, each failing independently when the old defect is
reintroduced: the `10^digits` identity across every variant, distinctness of all
scales (the exact property `3` and `_` violated), and the two values the old
catch-all got wrong.

**One more mutant turned out to be killable.** `pow_fp` guards its final
squaring, whose result is never read. That guard is load-bearing at the top of
the domain: squaring `(1+r)^256` overflows `i128` while `(1+r)^256` itself is
fine, so dropping the guard shrinks the rate-and-term range the engine can
underwrite. It is not observable through `monthly_payment` — the payment formula
multiplies principal by growth and overflows on that product first for any
realistic loan — so the test calls `pow_fp` directly.

**`mutants.out/` is no longer tracked.** It is build output: a full run rewrites
the whole directory, and tracking it made every mutation run a ~9,500-line diff
across 130 files. It accounted for 130 of the repository's 180 tracked files and
buried the real change in commit `f417ec6`. CI still keeps the report by
uploading the directory as a workflow artifact.

---

## Not done — declared, not disguised

- **No user interface.** `apps/shell` and `apps/terminal` are README stubs.
- **No live connector.** The framework is built; HUD FMR, listing sources, and
  assessor data are not wired. Each needs a written rights posture first (§5).
- **Nothing is persisted.** See "The daemon does not persist" above. This is the
  largest gap in the Rust half and it is invisible from the outside, because
  `/underwrite` returns a correct answer either way.
- **`cargo deny`, `cargo machete` and `cargo mutants` have all now run**, and all
  three found real problems. **All are fixed**, including the `Currency`
  exponent catch-all, and the mutation result is confirmed over three runs.
- **`pnpm install` completed on 2026-08-02.** `pnpm-lock.yaml` now exists and is
  a build input — keep it committed. The `drift` gate now runs and passes, so
  the generated client is regenerated and diffed on every `just ci`.
- **There are no doctests.** CI runs `cargo test --workspace --doc` and it
  passes over an empty set. Not a defect, but it is not evidence of anything
  either, and the crate-level docs are good enough to be worth executing.
- **`Display for Money` is wrong for a 0-exponent currency.** Found while
  reviewing the `Currency` fix; same latent class, different site. `width`
  becomes 0, so the format renders a trailing separator with no fraction
  digits — `1234.0 JPY` where it should be `1234 JPY`. Unreachable today, and
  deliberately **not fixed here**, because it is also untestable today: no
  `Currency` has exponent 0, so a fix could not be proven by a test, and this
  project does not ship unverified changes to a financial path. Fix it in the
  same commit that adds the first non-2-exponent currency, which is the commit
  that can test it. Note `Display` is excluded from mutation testing by
  `exclude_re` in `.cargo/mutants.toml`, so mutants will not surface this.
- **pnpm reports 9.12.3 → 11.18.0 available.** Do not take that upgrade
  casually: the version is pinned in `package.json` as a reproducibility
  guarantee, so bumping it is a deliberate commit, not a prompt to accept.
- **GitHub CI has never passed, and both jobs failed for reasons that have
  nothing to do with the code.** Read from the Actions API once the repository
  went public; two runs, both `failure`. Flagged here rather than assumed green,
  which is exactly why it was worth flagging.

  **`doctrine gates` died at step 6 of 16.** `pnpm/action-setup@v4` was given
  `version: 10` while `package.json` pins `packageManager: pnpm@9.12.3`; the
  action refuses a conflicting pair. Every gate after it — fmt, lint, tests,
  deps, drift — was **skipped**, so CI has never actually verified anything on
  this repository. The `version:` line is removed; the action now reads
  `package.json`, which is also the only value the committed lockfile works
  with. Fixed, but unverified until a run goes green.

  **`mutation testing` cannot pass as written.** `cargo mutants` exits 3 when
  any mutant survives, and two survive by construction — the `numer`/`denom`
  pair with a written proof that no test can kill them. Confirmed locally: exit
  3 on all three session-4 runs, including the final one. The job is therefore
  red permanently, which is worse than absent, because a gate that is always
  red is a gate nobody reads. **Not yet fixed** — it needs a decision about what
  the gate should assert, and the honest options are to compare against a known
  survivor set, or to make the job informational and let `just ci-full` be the
  real check before a release.

---

## Next, in order

1. **Get CI green.** It has never passed. The pnpm setup break is fixed but
   unverified; the mutation job still needs a decision about what it should
   assert, since two provably-equivalent survivors make `cargo mutants` exit
   non-zero forever. Until a run is green, no gate has ever been enforced by
   anything except a human running `just ci` locally.
2. **Persist something.** `velt-store` is implemented, tested, and called by
   nothing; `/underwrite` still forgets its answer the moment it responds. This
   is the biggest gap in the Rust half and the one the outside cannot see.
3. First real connector. HUD Fair Market Rent is the headline metric, so it is
   the obvious target. **Write the rights posture before the connector code**
   (§5).
4. The terminal UI, against the generated client. This is where VELT stops
   being an engine and starts being a product you can look at.

`just ci` is the gate for all of the above and it currently passes; keep it that
way. Three of the last four commits before session 4 were made with `fmt-check`
failing, which is how a green gate quietly stops being one.

---

## CMR (§6)

**CMR-OPEN.** Triggers T1–T5 have not fired. No auth, billing, entitlement,
multi-tenant storage, or paid data contract exists.

**T6 (Day 90) runs from 2026-08-02 → fires 2026-10-31.**

Licence audit 2026-08-02: 153 resolved packages, zero crates whose only
available licence is copyleft. `r-efi` is tri-licensed and taken under MIT.
`deny.toml` is the standing gate on the dependency graph, and `just deps` is how
it is checked.

### Licence decision, session 4 — VELT is MIT

`Cargo.toml` declared `license = "UNLICENSED"` and there was no `LICENSE` file.
Both are now MIT, and `LICENSE` is committed.

**This is a real narrowing of §6 and is recorded rather than glossed.** MIT
grants anyone who receives the code the right to use, modify, sublicense and
sell it. Model A (subscription / hosted) is unaffected. Model B (perpetual
licence) loses the code itself as leverage, since MIT already conveys perpetual
use to any recipient. Model C (proprietary edge) survives only for modules kept
outside this repository. The earlier claim that "no foreclosure has occurred"
was true when written and is no longer true; it has been removed from the README
rather than left standing.

One thing preserves the remaining optionality: copyright is undivided — a single
holder — so future work can be licensed differently. MIT cannot be retracted for
commits already published.

**The repository was made public on 2026-08-02, and that is the step that spent
the rest.** This file previously noted that while the repository stayed private
the grant bound nobody, because MIT runs only to people who receive the code —
and that publishing, not the LICENSE file, would be the decision that mattered.
That decision has now been taken deliberately: the plan is for VELT to be public
regardless. Anyone may now use, modify and sell this code. Recorded as fact, not
as a caveat.

The copyright line reads `Copyright (c) 2026 satex25` — the GitHub account that
owns the repository, chosen deliberately over the `git config user.name` value
of `col`. Now that the repository is public this is the attribution MIT requires
every downstream copy to preserve, so it is worth replacing with a legal name or
registered company: a handle is weak if the copyright ever has to be enforced or
assigned. Changing it later is a one-line commit and does not affect copies
already taken.

**Author email is public.** All commits carry `col <gracecolin1@gmail.com>` in
both author and committer fields, and that is now world-readable. Setting a
GitHub `@users.noreply.github.com` address fixes it for future commits; the
existing 18 would need a history rewrite, which changes every SHA and is
probably not worth it.
