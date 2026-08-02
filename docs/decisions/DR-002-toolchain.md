# DR-002 — Toolchain modernisation and verification gates

**Status:** accepted
**Date:** 2026-08-02
**Supersedes:** nothing. Extends DR-001.

## Context

DR-001 built the scaffold and proved it correct against hand-verified fixtures.
That establishes the engine matches its fixtures. It does not establish that the
fixtures *constrain* the engine, and it does not establish anything about the
dependency graph, which is where both the legal risk (§5) and the commercial-model
risk (§6) actually live.

This record covers the toolchain decisions taken to close those two gaps, plus
the edition migration.

## Decisions

**D1 — Rust edition 2024, resolver 3, MSRV 1.85.** `cargo fix --edition` was run
across the workspace under 1.97.1 and produced a **zero-line diff** — the tree
contained no 2021-era idioms, so the migration carried no behavioural risk.
Resolver 3 is MSRV-aware: a transitive dependency that raises its own
`rust-version` above ours is now rejected at resolve time rather than surfacing
as a confusing compile error. Reversal cost: one line.

**D2 — The toolchain is pinned to an exact version, not `stable`.**
`rust-toolchain.toml` pins `1.97.1`. A terminal that is wrong about money is
worth less than zero (§2), and "the build changed because someone ran `rustup
update`" is not an acceptable explanation for a changed figure. Bumping is now a
reviewable commit. Cost: manual bumps. Accepted.

**D3 — nextest replaces `cargo test` as the workspace runner.** This is a
correctness decision, not a speed one. §5 requires the computation engine to be
pure — no I/O, no clock, no ambient state. Under `cargo test`, every test in a
binary shares one process, so a test that passes only because a previous test
left state behind still goes green; the purity constraint is asserted but not
checked. nextest runs one process per test, which makes leaked state fail.
Doctests are not run by nextest and get a second pass in `just test`.

**D4 — cargo-mutants gates the financial paths in CI.** Snapshot tests prove the
engine matches its fixtures; mutation testing proves the fixtures constrain the
engine. It replaces function bodies with plausible wrong values — a `Money` that
returns zero, a comparison that flips — and reports any mutant the suite fails to
kill. Scoped to `velt-engine` and `velt-money` via `.cargo/mutants.toml`, because
mutating the daemon or store produces mutants killed by integration behaviour
rather than by unit tests, which is noise. A surviving mutant is a bug report
against the tests, not against the code.

**D5 — cargo-deny is a blocking gate, and the licence allow-list is a §6
artefact.** ⚑ Relevant to CMR. A copyleft crate reaching the statically linked
daemon binary would foreclose commercial model B (perpetual proprietary licence)
*silently* — precisely the unflagged foreclosure §6 forbids. The allow-list is
therefore permissive-only, and adding an entry is a commercial-model decision
rather than a build fix.

Audited against the resolved graph on 2026-08-02: **153 packages, zero crates
whose only available licence is copyleft.** `r-efi@5.3.0` and `r-efi@6.0.0` carry
`MIT OR Apache-2.0 OR LGPL-2.1-or-later` and are taken under MIT; both are
UEFI-target dependencies not linked on macOS or Linux. **No foreclosure has
occurred. CMR remains open on A, B, and C.**

**D6 — `panic = "abort"` was considered and rejected for the release profile.**
Clippy already denies `panic` in workspace code, so a panic in release means a
bug in a dependency. On a local-first terminal that should surface as a 500 from
the axum catch-panic layer, not as the daemon vanishing mid-session while the
user has unsaved analysis open. Recorded because it is the kind of decision that
otherwise gets relitigated every six months.

**D7 — Dependencies compile at `opt-level = 2` in the dev profile; workspace
crates stay at 0.** Dependencies change roughly monthly, workspace crates change
every minute, so the optimisation cost is paid once and cached. This is what
makes proptest runs and the bundled-SQLite tests tolerable in the inner loop.
Dev `debug` drops to `1` (line tables): backtraces and panic locations still
resolve, full DWARF does not, and linking gets materially faster.

**D8 — Fast linkers are deferred, not adopted.** `mold` and `wild` are
Linux-only. The development machine is macOS, which already uses Apple's parallel
linker. Configuration is present but commented in `.cargo/config.toml`. Per §7,
adopting one requires a measured trigger, not an anticipated one.

## Consequences

`just ci` now runs: fmt → lint → test (nextest + doctests) → dependency policy →
openapi → drift. `just ci-full` adds mutation testing and coverage. CI runs the
gate job and a separate mutation job so slow adversarial checks never block fast
feedback.

## Verification

Executed on this tree, not inspected:

| Gate | Result |
|---|---|
| `cargo build --workspace` | clean |
| `cargo clippy --workspace --all-targets -- -D warnings` | clean |
| `cargo fmt --all -- --check` | clean under style edition 2024 |
| `cargo test --workspace` | 50 passed, 0 failed |
| OpenAPI regenerated from Rust | no drift |
| TypeScript client regenerated | no drift, 13 schemas / 2 operations |
| All config TOML + CI YAML parsed | valid |

Not verified in this environment: `cargo deny check`, `cargo machete` and
`cargo mutants` were not executed — the binaries are installed on the
development machine, not in the environment where this tree was edited. The
licence audit above was performed directly against `cargo metadata`, which is
the same data cargo-deny reads. **First `just ci` on the Mac is the real
confirmation.**
