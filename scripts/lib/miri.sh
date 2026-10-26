# shellcheck shell=bash
# Copyright (C) 2026 rezky_nightky
# SPDX-License-Identifier: GPL-3.0-only
# OS: Linux only - zylaxion is a Linux keyboard-sound tool.
#
# ZYLAXION BUILD MODULE: miri (night-dragon-maintenance-1, the sibling
# zejtron's mature runner shape ported to this estate)
#
# Miri (the undefined-behavior detector) integration: the stamp file
# contract, the status banner shown on every build.sh invocation, and
# the nightly-driven runner scoped to the pure-Rust test families that
# are rootless and side-effect-free by design.
#
# One implementation, two surfaces: the runner logic lives here (the
# audited-families list, the admission law, the stamp), dispatched by
# ./scripts/build.sh miri - so a contributor runs the same command
# locally that CI runs, and the stamp file contract (target/miri-stamp)
# means CI's verdict and the local banner read from the same source of
# truth. The workflow (.github/workflows/miri.yml) only installs the
# toolchain and calls the runner with --no-install.
#
# The incident this shape guards against (night-dinner-6, this
# estate): a miri run hung past 30 minutes with 93% of its wall clock
# inside FIVE tests - two full-text README/CHANGELOG doc-assertion
# scans (~23 minutes interpreted) and three sample-loop render tests
# (~6 minutes of pure float work). All five are pure safe code with
# no pointer/borrow surface the other tests do not already cover, so
# they carry #[cfg_attr(miri, ignore = "...")] (the reason spelled out
# in each) and still run NATIVELY in every other lane.
#
# The admission law (what keeps the incident from recurring): the
# audited families below are the pointer/alloc/UB surface, not the
# docs lane and not the kernel-object lane. A test that full-text
# scans README/CHANGELOG, burns an unbounded numeric/render loop, or
# calls into the foreign libc/kernel ABI must never enter these
# families without
#   #[cfg_attr(miri, ignore = "<the reason>")]
# - it keeps running natively in every other lane (ci.yml's cargo test
# and the Supermassive VM battery own the docs assertions, the math
# loops, and the kernel surface). The families carry those five
# ignores today (verified 2026-10-26 against the tree); this law is
# what keeps it true. A future burden test that slips in anyway dies
# loudly inside the workflow's fail-fast cap instead of hanging the
# runner.
#
# Scope (substring filters against the full test path; the test tree
# is the zelynic Pattern C shape wired via #[path], so the filters
# mirror the src module paths):
#   - cli::        the clap argument pins, the suggestion ladder,
#                  the ux contract
#   - commands::   the command-surface output contracts (the two
#                  rlimit-ABI pins inside carry their own miri
#                  ignores - foreign libc is not interpretable)
#   - core::       the core math and string pins
#   - engine::     the voice-pool lifecycle math (the two one-second
#                  render loops and the pool loop carry their own
#                  miri ignores; the freeze doc-assertions carry the
#                  two README/CHANGELOG-scan ignores)
#   - info::       the Hinnant civil-clock lanes + the build report
#   - output::     the output-surface pins
#   - profiles::   the preset tuning tables and registry pins
#   - preset_tests the session preset parser/validator pins (the one
#                  pure slice of the session tree - the orphan,
#                  privilege, and kernel families read /proc, /sys,
#                  and spawn processes, shapes miri cannot run, so
#                  the session tree rides OUTSIDE the audited scope)
#   The ebpf tree rides outside too: the loader/bpf-syscall/pin
#   surface is the kernel-object estate, not the pure core (its
#   interpreter-hostile pins already carry their own miri ignores).
#
# Miri flags: -Zmiri-disable-isolation is deliberately NOT set (the
# zylaxion contract, unchanged) - the audited families must stay
# deterministic; a test needing host isolation is a test bug, and
# miri failing loudly is the contract.
#
# Verification status is cached in target/miri-stamp (key=value
# format) so a build does not re-run miri; the banner at the start of
# every build.sh invocation reflects the stamp state:
#   - VERIFIED  - stamp commit == HEAD
#   - STALE     - stamp commit != HEAD (suggests a re-run)
#   - FAILED    - the last run had violations
#   - NEVER RUN - no stamp yet
#
# Sourced by scripts/build.sh - not a standalone executable (no main,
# no argument parsing; the entry script owns both).

# The runner's own defaults (the entry script's parser fills these).
MIRI_FILTER="${MIRI_FILTER:-}"
MIRI_NO_INSTALL="${MIRI_NO_INSTALL:-0}"
MIRI_FULL="${MIRI_FULL:-0}"

readonly MIRI_STAMP_FILE="target/miri-stamp"
readonly MIRI_LOG_FILE="target/miri-log.txt"

# resolve_nightly_name - the toolchain name the runner addresses.
# Plain `nightly` wins (what CI installs and what +nightly resolves);
# a dated nightly is the fallback so a machine that never installed
# the moving tag still runs. Empty means: nothing nightly-shaped, the
# caller installs.
resolve_nightly_name() {
    rustup toolchain list 2>/dev/null | awk '$1 == "nightly" {print $1; exit}'
}

# The audited scope: the pure families (the admission law above owns
# what may enter them).
readonly MIRI_AUDIT_MODULES=(
    "cli::"
    "commands::"
    "core::"
    "engine::"
    "info::"
    "output::"
    "profiles::"
    "preset_tests"
)

# show_miri_status - print the one-line stamp banner. Called from
# build.sh on every invocation; quiet under -q.
show_miri_status() {
    [ "${QUIET:-0}" -eq 1 ] && return 0

    local head head_short
    head=$(git rev-parse HEAD 2>/dev/null || echo "")
    head_short=$(git rev-parse --short HEAD 2>/dev/null || echo "")

    if [ ! -f "${MIRI_STAMP_FILE}" ]; then
        echo "Miri: never run on this workspace. Run './scripts/build.sh miri' to verify (needs nightly, ~4 min)."
        return 0
    fi

    local stamp_commit stamp_short stamp_ts_iso stamp_status stamp_dur stamp_run stamp_fail
    stamp_commit=$(grep '^commit=' "${MIRI_STAMP_FILE}" | cut -d= -f2- || true)
    stamp_short=$(grep '^commit_short=' "${MIRI_STAMP_FILE}" | cut -d= -f2- || true)
    stamp_ts_iso=$(grep '^timestamp_iso=' "${MIRI_STAMP_FILE}" | cut -d= -f2- || true)
    stamp_status=$(grep '^status=' "${MIRI_STAMP_FILE}" | cut -d= -f2- || true)
    stamp_dur=$(grep '^duration_ms=' "${MIRI_STAMP_FILE}" | cut -d= -f2- || true)
    stamp_run=$(grep '^tests_run=' "${MIRI_STAMP_FILE}" | cut -d= -f2- || true)
    stamp_fail=$(grep '^tests_failed=' "${MIRI_STAMP_FILE}" | cut -d= -f2- || true)

    local dur_sec=""
    if [ -n "${stamp_dur}" ]; then
        dur_sec=$(awk -v ms="${stamp_dur}" 'BEGIN { printf "%.1f", ms/1000 }')
    fi

    case "${stamp_status}" in
    verified)
        if [ "${stamp_commit}" = "${head}" ]; then
            echo "Miri VERIFIED at ${stamp_short} - ${stamp_ts_iso} - ${stamp_run} tests (${stamp_fail} fail) - ${dur_sec}s"
        else
            echo "Miri STALE - verified at ${stamp_short}, HEAD is ${head_short}. Run './scripts/build.sh miri' to refresh."
        fi
        ;;
    failed)
        echo "Miri FAILED at ${stamp_short} - ${stamp_ts_iso} - ${stamp_fail} violations. See ${MIRI_LOG_FILE}."
        ;;
    skipped:*)
        local reason="${stamp_status#skipped:}"
        echo "Miri SKIPPED (${reason}) at ${stamp_short}. Run './scripts/build.sh miri' to verify."
        ;;
    *)
        echo "Miri: unknown status '${stamp_status}' at ${stamp_short}."
        ;;
    esac
}

# run_miri - run Miri verification on the pure-Rust test families and
# update the stamp file. Args (parsed by build.sh before main):
#   --filter <pat>  narrow the scope to one substring (a partial run;
#                   the stamp is NOT updated)
#   --no-install    never auto-install nightly/miri (CI's contract -
#                   the workflow installs its own pinned toolchain, so
#                   a missing component must fail loudly, not download)
#   --full          run the ENTIRE test suite (slow; the session and
#                   ebpf families that read /proc, /sys, or the kernel
#                   ABI will fail under miri)
run_miri() {
    local filter="${MIRI_FILTER}"
    local full="${MIRI_FULL}"

    # 1. rustup is required (nightly management is its job).
    if ! command -v rustup &>/dev/null; then
        echo "error: rustup not installed. Miri requires nightly. Install from https://rustup.rs" >&2
        exit 1
    fi

    # 2. Resolve (or install) the nightly toolchain the runner rides.
    local nightly_name
    nightly_name=$(resolve_nightly_name)
    if [ -z "${nightly_name}" ]; then
        nightly_name=$(rustup toolchain list 2>/dev/null | awk '$1 ~ /^nightly-/ {print $1; exit}')
    fi
    if [ -z "${nightly_name}" ]; then
        if [ "${MIRI_NO_INSTALL}" = "1" ]; then
            echo "error: nightly toolchain not installed and --no-install given. Aborting." >&2
            exit 1
        fi
        echo "+ rustup toolchain install nightly --profile minimal --component miri"
        rustup toolchain install nightly --profile minimal --component miri || {
            echo "error: failed to install nightly + miri. Try manually: rustup toolchain install nightly --component miri" >&2
            exit 1
        }
        nightly_name="nightly"
    else
        # 3. Ensure the miri component is installed on that nightly.
        if ! rustup component list --toolchain "${nightly_name}" 2>/dev/null | grep -q '^miri[-[:space:]].*installed'; then
            if [ "${MIRI_NO_INSTALL}" = "1" ]; then
                echo "error: miri component not installed on ${nightly_name} and --no-install given. Aborting." >&2
                exit 1
            fi
            echo "+ rustup component add miri --toolchain ${nightly_name}"
            rustup component add miri --toolchain "${nightly_name}" || {
                echo "error: failed to install miri component." >&2
                exit 1
            }
        fi
    fi

    local nightly_ver miri_ver
    nightly_ver=$(rustc "+${nightly_name}" --version 2>/dev/null | head -1)
    miri_ver=$(cargo "+${nightly_name}" miri --version 2>/dev/null | head -1)
    echo "Miri: ${miri_ver}"
    echo "Nightly: ${nightly_ver}"

    # 4. One-time libstd setup, non-interactive (a prompt mid-run would
    #    hang CI and a piped local run alike).
    echo "+ cargo ${nightly_name} miri setup"
    cargo "+${nightly_name}" miri setup 2>&1 | tail -3 || true

    # 5. Build the filter list. An explicit --filter rides as a partial
    #    probe (no stamp update); the default scope walks the audited
    #    families; --full walks everything (no stamp).
    local -a harness_args=()
    if [ -n "${filter}" ]; then
        harness_args=("${filter}")
        echo "Running Miri with filter: ${filter}"
    elif [ "${full}" = "1" ]; then
        echo "Running Miri on the FULL test suite (slow; the /proc, /sys, and kernel-ABI families will fail under miri)..."
    else
        read -r -a harness_args <<<"${MIRI_AUDIT_MODULES[*]}"
        echo "Scope: ${#harness_args[@]} families (${harness_args[*]})"
    fi

    # 6. Run Miri. --locked: the lockfile-freeze contract (Cargo.lock is
    #    committed; an interpreter build must not silently re-resolve
    #    the dependency graph). Miri flags stay empty by default - see
    #    the isolation contract in the header. The audited families
    #    remain the feature-independent pure-Rust core - the eBPF
    #    loader is not miri's subject.
    #    Test filters ride the harness argv (after --); the cargo
    #    options stay before it.
    #    tee opens its output the moment the pipeline starts, before
    #    cargo has materialized target/ on a cold cache - so on a
    #    cache miss tee cannot open the log, streams to stdout only,
    #    and exits 1 at EOF owning the pipeline status (the red run
    #    pattern: every suite interpreted clean, the job red anyway,
    #    and the artifact upload found nothing). The mkdir closes that
    #    race before the pipeline exists.
    local start_ms end_ms duration_ms
    start_ms=$(date +%s%3N 2>/dev/null || date +%s)

    mkdir -p target
    rm -f "${MIRI_LOG_FILE}"
    local miri_exit=0
    MIRIFLAGS="${MIRIFLAGS:-}" \
        cargo "+${nightly_name}" miri test --locked -- "${harness_args[@]}" 2>&1 | tee "${MIRI_LOG_FILE}" || miri_exit=1

    end_ms=$(date +%s%3N 2>/dev/null || date +%s)
    duration_ms=$((end_ms - start_ms))

    # 7. Parse the result counts (|| true guards a compile-failure log
    #    that holds no summary).
    local tests_run=0 tests_failed=0
    tests_run=$(grep -oE '[0-9]+ passed' "${MIRI_LOG_FILE}" 2>/dev/null | awk '{s+=$1} END {print s+0}' || true)
    tests_failed=$(grep -oE '[0-9]+ failed' "${MIRI_LOG_FILE}" 2>/dev/null | awk '{s+=$1} END {print s+0}' || true)

    # 8. The verdict. A verified run that matched ZERO tests is a drift
    #    smell, not a pass - the scope list names module paths, and a
    #    rename would otherwise turn the gate green while checking
    #    nothing.
    local status
    if [ "${miri_exit}" = "0" ] && [ "${tests_failed}" = "0" ]; then
        status="verified"
        if [ "${tests_run}" = "0" ] && [ "${full}" != "1" ]; then
            echo "warning: Miri matched 0 tests - a scope family may have drifted. Check MIRI_AUDIT_MODULES against the module wiring." >&2
        fi
        echo "Miri verification PASSED (${tests_run} tests, 0 fail, ${duration_ms}ms)"
    else
        status="failed"
        echo "Miri verification FAILED (${tests_failed} failures, ${duration_ms}ms). See ${MIRI_LOG_FILE}." >&2
    fi

    # 9. Stamp file. Partial runs (--filter/--full) do not update it -
    #    they do not represent the audited scope.
    if [ -n "${filter}" ] || [ "${full}" = "1" ]; then
        echo "Partial run - stamp file not updated. Run './scripts/build.sh miri' (no flags) to refresh."
    else
        local head head_short ts_iso ts_unix
        head=$(git rev-parse HEAD 2>/dev/null || echo "unknown")
        head_short=$(git rev-parse --short HEAD 2>/dev/null || echo "unknown")
        ts_unix=$(date +%s)
        ts_iso=$(date -u +%Y-%m-%dT%H:%M:%SZ)

        local modules
        modules=$(IFS=',' && echo "${MIRI_AUDIT_MODULES[*]}")

        cat >"${MIRI_STAMP_FILE}" <<STAMP_EOF
# zylaxion Miri verification stamp (generated by scripts/build.sh miri)
# Format: key=value. Parse with grep '^key=' | cut -d= -f2-
commit=${head}
commit_short=${head_short}
timestamp_unix=${ts_unix}
timestamp_iso=${ts_iso}
status=${status}
duration_ms=${duration_ms}
tests_run=${tests_run}
tests_failed=${tests_failed}
modules=${modules}
miri_version=${miri_ver}
nightly_version=${nightly_ver}
STAMP_EOF
        echo "Stamp written to ${MIRI_STAMP_FILE}"
        echo "Log saved to ${MIRI_LOG_FILE}"
    fi

    if [ "${miri_exit}" != "0" ] || [ "${tests_failed}" != "0" ]; then
        exit 1
    fi
}
