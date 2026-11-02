<!-- Copyright (C) 2026 rezky_nightky -->
<!-- SPDX-License-Identifier: AGPL-3.0-only -->

# The night-dragon-maintenance-xenric-1 CLI output audit (2026-11-02)

The task: a total master-depth audit of zylaxion's CLI output — the
clean (non-verbose) surfaces and the verbose/diagnostic story —
measured against the family reference, `github.com/oxyzenQ/zejtron`,
whose `src/output/` group is the mature implementation of the same
law. Honest verdicts only: every divergence names whether it is a
drift to fix or a documented decision to keep, every fix names its
before/after bytes, and everything this session deliberately did NOT
change says why (the anti-over-engineering section at the end).

## Method and sandbox honesty

The repo is MEDIUM weight at this tree: `src/` 12,734 lines of Rust
across 89 files, `ebpf/` 391, `build.rs` + `build/` 2,096, `test/`
5,616, plus the doc mass (CHANGELOG 158 KB, README 35 KB). The whole
CLI/output estate was read at source level: `src/output/`,
`src/cli/` (mod, styles, ux, suggestion), `main.rs`, and the command
handlers behind every printed byte (`doctor`, `overview`, `status`,
`stats`, `attach`, `update`, `help`, `player`, `sound/*`), compared
line-by-line against zejtron's `src/output/{mod,verbose,spinner,
capability}.rs` and `src/cli/`. The sandbox: non-root uid, no sudo,
no bpffs mount, no audio device; the toolchain was solved without
root (rustup user-level, `alsa-lib 1.2.16` built into a local prefix
for `PKG_CONFIG_PATH`, `scripts/dev/bootstrap-ebpf.sh` for the dated
nightly + bpf-linker + b3sum — all three prerequisites green, so the
eBPF source-build lane compiles here, which the previous audit's
sandbox could not carry).

Weight strategy for the session: targeted reads only, no full-tree
scan; every behavioral claim below is a byte-level source citation,
not a summary.

## The clean (non-verbose) surfaces — verdicts

| surface | verdict | evidence |
|---------|---------|----------|
| `-V` / `--version` | PASS — family parity | brand header lines each carry their own purple wrap (the single-line law, `output::brand`), the body is plain and grep-able; zejtron's `-V` keeps the same two-shape contract |
| `--help` (curated) | PASS — the single-tier law | clap auto-help disabled; one reference, grey grammar tier, no second help wall; identical doctrine in zejtron's help surface |
| clap fatal errors | PASS — the ux bridge | one-tip contract, canonical footer exactly once, exit 2, same capability gate as every other lane |
| error/warning lanes | PASS | GNU `zylaxion: error:` labels, the five-color palette, suggestion-line classification (grey passive / green runnable), broken-pipe-safe emitters on both streams; the same tier constants zejtron pins (`#FF5A5A`, `#FFEB3C`, `#50FA7B`, `#A855F7`, `#8B8B8B`) |
| report frame | PASS | the `━━━ title ━━━` bold-purple banner is shared verbatim with zejtron's `banner()` law — doctor, overview, status sections, update check, telemetry all open with it |
| stdout/stderr anchoring | PASS | reports speak stdout through `print_stdout_line` (SIGPIPE-safe), diagnostics stderr; the pipe law holds (`status | head` cannot kill the process) |
| doctor / overview | PASS | both ride `kv()` — two-space indent, 11-column keys — the zelynic geometry, byte-identical law |
| exit codes | PASS | 2 usage, 1 runtime refusal, 0 success/version — matches the ux contract |

### FINDING F2 — the two posture reports drifted on geometry (FIXED)

`zylaxion` (the no-subcommand overview) and `zylaxion status` are the
family's two posture reports. The overview renders its rows through
`output::kv()`; `status` carried its own manual padding
(`"kernel:   "`, `"tap:      "`, ...) — the same `kernel:` key landed
at a different column on sibling surfaces, and the kv() doc contract
("every report surface's values align at the same column") was false
for one of its two flagship users. No test or doc pinned the old
bytes. Fixed: every `status` row (kernel, tap, sound, preset, volume,
player, the fix/telemetry continuations, the introspection tip) now
rides `kv()`, continuations use the empty-key continuation form, so
both posture reports and the doctor align at one column.

## The verbose / diagnostic story — verdicts

zejtron's law (night-hunter-4): one global `-v/--verbose`, one
`vlog!` macro, one bracketed `[tag] message` stderr lane, stdout
stays ledger-clean; no `log` crate, no env_logger anywhere in the
tree. zylaxion's story measured against it:

### FINDING F1 — the panic hook's context line was dead on the journal (FIXED)

`main.rs`'s panic hook (the dragonzen audit S3 stream-death
diagnostic) reported through `log::error!` — but the journal is
wired per-verb, so a panic BEFORE a verb wired env_logger rendered
nothing through the facade (the chained default handler still
printed, the hook's own context line did not). Fixed: the hook now
speaks through `output::error` — `zylaxion: error: panic at ...` —
the lane that needs no wiring, so the context line survives every
dispatch order and carries the family label. The detached player's
stderr stays nulled (below), unchanged.

### FINDING F5 — three copies of one journal builder (FIXED)

`attach.rs`, `sound/mod.rs`, and `player.rs` each carried a verbatim
copy of the same env_logger builder closure (the hand-rolled UTC
stamp, the level/target columns). Three copies of one format drift
silently. Fixed: `src/output/journal.rs` is now the one door —
`journal::init(default_filter)` — and the three verbs keep only
their loudness decision at their call sites (`attach` info by
design; sound verbs and player warn, night-hunter-6). `RUST_LOG`
still overrides; the per-verb rationale moved into the module doc
where the next reader will find it.

### The global `-v` divergence — DOCUMENTED, NOT PORTED (decision)

zejtron arms a global `-v`; zylaxion does not, and porting it verbatim
is not available: `status --verbose` already owns the `--verbose`
name on a subcommand (clap refuses a global arg of the same name —
a build-time panic, not a warning), and the documented user contract
(`status --verbose` = the telemetry deep read, printed across help,
README, and QA) must not break. The engine's diagnostic lane here is
the journal: `RUST_LOG=info zylaxion _play` in the foreground is the
documented full-story shape, and the parent's failure messages
already name it. A future family-wide unification would either
rename status's flag (a breaking CLI change, owner's call) or move
the 71 `log::` call sites onto a zejtron-style `vlog!` lane — the
map is: attach-chain, orchestrate, and player lifecycle traces to
`[ebpf]`/`[player]` tags; audio negotiation to `[audio]`; user-
impacting warnings (stream death, watcher failure) keep a
default-visible seat via `output::warning`. Recorded here so the
next session starts from the map, not from a re-audit.

### FINDING F4 — the player's stderr dies at spawn: BY DESIGN (kept)

The detached player spawns with `stdin/stdout/stderr → Stdio::null()`
(night-hunter-6: the detached player owns no terminal bytes — a
preset switch an hour later must never dirty the shell that ran
`on`). This audit initially flagged the player's 71 log sites as
unreachable noise; the source comment chain proves the design is
deliberate and the debugging path is documented (foreground
`zylaxion _play`). Kept as-is. The panic-hook fix above is what
makes the foreground diagnostic run honest.

### The capability ladder divergence — DOCUMENTED, NOT PORTED (decision)

zejtron resolves a four-rung capability ladder (Mono / Color16 /
Color256 / TrueColor) and pins explicit 16/256-color escape slots;
zylaxion gates a boolean (TTY + `NO_COLOR` + `CLICOLOR`) and emits
truecolor RGB everywhere, relying on the terminal's own
approximation. zylaxion's `cli/styles.rs` documents this as a
conscious depth contract ("zylaxion has no --color-mode ladder ...
if that surface ever lands, mirror zelynic's clap_styles_16 here").
Not a drift — a shallower rung the owner chose. Recommendation on
record: when the ladder lands, port zejtron's `capability.rs`
ladder-as-data shape wholesale; the helper signatures in
`output/mod.rs` gate internally, so call sites would not move.

## What changed in this commit (the fix set)

1. F1: panic hook → `output::error` (main.rs), the pre-init gap
   closed, family label carried.
2. F2: `status` rows → `output::kv()` (sound/status.rs), the two
   posture reports and the doctor align at one column.
3. F5: `output/journal.rs` — one builder door, three loudness call
   sites (attach info, sound warn, player warn), `RUST_LOG` contract
   unchanged.

No user-facing byte changed except the status geometry alignment and
the panic line's label; no preset, map, pin, spawn, or exit-code
behavior moved.

## What this session deliberately did NOT change (anti-over-engineering)

- No global `-v` (blocked by the status `--verbose` name contract and
  clap's arg-name uniqueness; conversion map above).
- No capability ladder port (documented shallower rung; port plan
  above).
- No spinner port (zejtron's spinner narrates intentional multi-
  second waits; zylaxion's verbs are one-shot map writes with no wait
  to narrate — a port would be decoration, not information).
- No default-loudness change on `attach` (the info chain is the
  privileged verb's product feedback, not leakage).
- No env_logger removal (zejtron runs no log crate; zylaxion's
  journal with the trimmed no-time-crate feature set is the owner's
  invested design — one door now, not three).
- No benchmark run: zylaxion carries no benchmark suite (no
  criterion, no bench target, no density/entropy metrics — those
  belong to a sibling project), so per the maintenance rules the A/B
  step is skipped honestly; the verification below is compile + live
  runs + gates.

## Verification

- `cargo check --all-targets` green on the patched tree (eBPF
  source-build lane included — nightly 2026-09-18 + bpf-linker
  0.11.1 + b3sum 1.8.7 via the bootstrap script).
- Live runs of the built binary: `-V`, `--help`, bare overview,
  `status`, `status --verbose`, `doctor`, `list-presets`,
  `list-backends`, the honest refusals (`on` without the tap,
  `attach` without root) — byte shapes as pinned, exit codes as
  contracted.
- Gates: rustfmt, clippy `-D warnings`, check-headers,
  check-language, check-loc, check-permissions.
