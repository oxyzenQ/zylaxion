#!/usr/bin/env python3
# SPDX-FileCopyrightText: 2025-2026 rezky_nightky (oxyzenQ)
# SPDX-License-Identifier: AGPL-3.0-only OR Commercial
# PLATFORM: UNIX-only (Linux). zylaxion is a Linux-only tool.
#
# ZYLAXION SECURITY SWEEP - the Sunday supply-chain bot
#
"""The maintenance lane's security classification pass (Dragon Guard
- Dependency Maintenance).

The owner's contract for the Sunday run is a smart bot, not a bare
gate: the lane updates every tracked lockfile first (night-zen-1,
scripts/dev/update-deps.sh), and the sweep answers the two scenarios
that matter to a high-value tree whose owner is asleep at run time:

  1. an advisory exists and an in-range release clears it - the
     updater already landed the bump, so the run stays GREEN and the
     job summary reports the fix as an urgent auto-update (the tree
     was exposed before the sweep, it is clean now);
  2. an advisory exists and nothing in the current requirement set
     reaches a patch - the run FAILS after the update commit lands
     (fixable work is never blocked by unfixable findings), and the
     summary says CALL THE OWNER, with the reason per finding: a
     patched release exists but is unreachable (move the manifest
     requirements deliberately), or no patched release is published
     at all (watch upstream, or remove/replace the dependency).

The mechanism is a pre/post diff over `cargo audit --json --file` on
every tracked lockfile: `pre` snapshots the HEAD tree before the
update, `post` re-sweeps after it, classifies, writes the job summary
(GITHUB_STEP_SUMMARY) and the verdict file, and always exits 0 - the
`verdict` subcommand owns the only exit-1 point, run from the final
step of the job (`if: always()`), so the commit step keeps its chance
to land the fixable bumps even when unfixable findings remain. A
scanner that cannot run is a RED verdict, never a silent green: a
sweep that cannot see the tree must not certify it.

The bar is the canonical lane's bar (audit.yml, `--deny warnings`):
remaining WARNINGS (unmaintained, unsound, yanked) fail the run the
same way they fail the push lane, classified as owner review.

Usage:
  security-sweep.py pre      # snapshot the HEAD tree (never exits 1)
  security-sweep.py post     # re-sweep, classify, write summary+verdict
  security-sweep.py verdict  # exit 1 iff the verdict is RED
"""

from __future__ import annotations

import sys

sys.dont_write_bytecode = True  # the root-owned pycache guard (night-cafe-1)

import json
import os
import shutil
import subprocess
import tempfile
from dataclasses import dataclass
from pathlib import Path

SWEEP_DIR = Path(
    os.environ.get("ZYLAXION_SWEEP_DIR")
    or Path(tempfile.gettempdir()) / "zylaxion-security-sweep"
)
PRE_DIR = SWEEP_DIR / "pre"
POST_DIR = SWEEP_DIR / "post"
VERDICT_FILE = SWEEP_DIR / "verdict"
COMMIT_NOTE_FILE = SWEEP_DIR / "commit-note.txt"
AUDIT_TIMEOUT_SECONDS = 180

ECHO = "security-sweep:"


@dataclass(frozen=True)
class Vuln:
    """One vulnerability entry from the cargo-audit JSON report (the
    0.23 shape: vulnerabilities are top-level, warnings are a map)."""

    lockfile: str
    package: str
    version: str
    advisory_id: str
    title: str
    patched: tuple[str, ...]
    aliases: tuple[str, ...] = ()


@dataclass(frozen=True)
class AdvisoryWarning:
    """One cargo-audit warning (unmaintained, unsound, yanked, ...)."""

    lockfile: str
    package: str
    version: str
    kind: str
    advisory_id: str
    title: str


@dataclass(frozen=True)
class FixedVuln:
    """An advisory the pre-update tree carried and the bump cleared."""

    lockfile: str
    package: str
    old_version: str
    new_version: str
    advisory_id: str
    title: str
    aliases: tuple[str, ...] = ()


@dataclass
class LockSweep:
    """The classified result of auditing one lockfile."""

    lockfile: str
    vulns: list[Vuln]
    warnings: list[AdvisoryWarning]
    error: str | None = None


# ── cargo audit invocation and parsing ──────────────────────────────────


def tracked_lockfiles() -> list[str]:
    """Every tracked lockfile, null-delimited end to end (night-zen-2)."""
    proc = subprocess.run(
        ["git", "ls-files", "-z", "--", "*Cargo.lock"],
        check=True,
        capture_output=True,
        text=True,
    )
    locks = sorted(part for part in proc.stdout.split("\x00") if part)
    return locks


def as_list(value: object) -> list:
    return value if isinstance(value, list) else []


def as_dict(value: object) -> dict:
    return value if isinstance(value, dict) else {}


def walk_list(doc: dict, *keys: str) -> list:
    """Walk a key path and return the list at its end ([] on any miss)."""
    node: object = doc
    for key in keys:
        node = as_dict(node).get(key, [])
    return as_list(node)


def parse_sweep(lockfile: str, doc: dict) -> LockSweep:
    """The cargo-audit 0.23 JSON shape (the scanner version the workflow
    pins): `vulnerabilities` is top-level and `warnings` is a map keyed
    by warning kind. Defensive on every field: a schema drift degrades
    values to unknown, it never crashes the sweep (the verdict must
    stay honest either way)."""
    vulns: list[Vuln] = []
    warnings: list[AdvisoryWarning] = []
    for item in walk_list(doc, "vulnerabilities", "list"):
        advisory = as_dict(item.get("advisory"))
        package = as_dict(item.get("package"))
        patched = tuple(
            str(v)
            for v in walk_list(item, "versions", "patched")
            if isinstance(v, str)
        ) or tuple(
            str(v)
            for v in as_list(advisory.get("patched"))
            if isinstance(v, str)
        )
        vulns.append(
            Vuln(
                lockfile=lockfile,
                package=str(package.get("name") or advisory.get("package") or "unknown"),
                version=str(package.get("version") or "unknown"),
                advisory_id=str(advisory.get("id") or "unknown-advisory"),
                title=str(advisory.get("title") or ""),
                patched=patched,
                aliases=tuple(
                    str(a) for a in as_list(advisory.get("aliases")) if isinstance(a, str)
                ),
            )
        )
    for kind, entries in sorted(as_dict(doc.get("warnings")).items()):
        for entry in as_list(entries):
            if not isinstance(entry, dict):
                continue
            package = as_dict(entry.get("package"))
            advisory = as_dict(entry.get("advisory"))
            warnings.append(
                AdvisoryWarning(
                    lockfile=lockfile,
                    package=str(package.get("name") or advisory.get("package") or "unknown"),
                    version=str(package.get("version") or "unknown"),
                    kind=str(entry.get("kind") or kind),
                    advisory_id=str(advisory.get("id") or "-"),
                    title=str(advisory.get("title") or ""),
                )
            )
    return LockSweep(lockfile=lockfile, vulns=vulns, warnings=warnings)


def audit_lock(lockfile: str) -> LockSweep:
    """One cargo-audit pass on one lockfile. Exit 1 means findings (data,
    not a verdict here); anything else - broken tool, dead fetch, timeout,
    unparsable output - is a scan error, and a scan error is a RED
    verdict downstream, never a silent pass. The flag is --format json
    (cargo-audit 0.23 replaced the old bare --json with it) and the
    workflow pins the scanner version to keep this contract still."""
    try:
        proc = subprocess.run(
            ["cargo", "audit", "--format", "json", "--file", lockfile],
            capture_output=True,
            text=True,
            timeout=AUDIT_TIMEOUT_SECONDS,
        )
    except (OSError, subprocess.TimeoutExpired) as exc:
        return LockSweep(lockfile, [], [], error=f"cargo audit could not run: {exc}")
    if proc.returncode not in (0, 1):
        stderr_lines = [line for line in (proc.stderr or "").strip().splitlines() if line]
        detail = stderr_lines[-1] if stderr_lines else "no stderr"
        return LockSweep(lockfile, [], [], error=f"cargo audit exited {proc.returncode}: {detail}")
    try:
        doc = json.loads(proc.stdout)
    except json.JSONDecodeError as exc:
        return LockSweep(lockfile, [], [], error=f"cargo audit printed unparsable JSON: {exc}")
    return parse_sweep(lockfile, doc)


# ── lockfile version lookup (the was -> now column) ─────────────────────


def lockfile_versions(lockfile: str) -> dict[str, list[str]]:
    """Map package name -> the version(s) the lockfile currently pins."""
    try:
        import tomllib
    except ModuleNotFoundError:
        return lockfile_versions_fallback(lockfile)
    try:
        with open(lockfile, "rb") as handle:
            doc = tomllib.load(handle)
    except (OSError, ValueError):
        return lockfile_versions_fallback(lockfile)
    versions: dict[str, list[str]] = {}
    for package in as_list(doc.get("package")):
        name = package.get("name")
        version = package.get("version")
        if isinstance(name, str) and isinstance(version, str):
            versions.setdefault(name, []).append(version)
    return versions


def lockfile_versions_fallback(lockfile: str) -> dict[str, list[str]]:
    """A minimal line parser for the [[package]] tables, for interpreters
    without tomllib (< 3.11)."""
    versions: dict[str, list[str]] = {}
    current_name: str | None = None
    in_package = False
    with open(lockfile, encoding="utf-8") as handle:
        for raw in handle:
            line = raw.strip()
            if line.startswith("[["):
                in_package = line == "[[package]]"
                current_name = None
                continue
            if not in_package or line.startswith("#") or "=" not in line:
                continue
            key, _, value = line.partition("=")
            key = key.strip().strip('"')
            value = value.strip().strip('"')
            if key == "name":
                current_name = value
            elif key == "version" and current_name:
                versions.setdefault(current_name, []).append(value)
    return versions


# ── snapshot storage (pre vs post) ──────────────────────────────────────


def encode_sweep(sweep: LockSweep) -> dict:
    return {
        "lockfile": sweep.lockfile,
        "error": sweep.error,
        "vulns": [vars(v) for v in sweep.vulns],
        "warnings": [vars(w) for w in sweep.warnings],
    }


def decode_sweep(doc: dict) -> LockSweep:
    """Explicit reconstruction (never **kwargs): an unexpected stored key
    degrades, it does not crash, and `patched` returns as a tuple."""
    vulns = [
        Vuln(
            lockfile=str(raw.get("lockfile") or ""),
            package=str(raw.get("package") or ""),
            version=str(raw.get("version") or ""),
            advisory_id=str(raw.get("advisory_id") or ""),
            title=str(raw.get("title") or ""),
            patched=tuple(str(p) for p in as_list(raw.get("patched"))),
            aliases=tuple(str(a) for a in as_list(raw.get("aliases"))),
        )
        for raw in as_list(doc.get("vulns"))
        if isinstance(raw, dict)
    ]
    warnings = [
        AdvisoryWarning(
            lockfile=str(raw.get("lockfile") or ""),
            package=str(raw.get("package") or ""),
            version=str(raw.get("version") or ""),
            kind=str(raw.get("kind") or "informational"),
            advisory_id=str(raw.get("advisory_id") or "-"),
            title=str(raw.get("title") or ""),
        )
        for raw in as_list(doc.get("warnings"))
        if isinstance(raw, dict)
    ]
    return LockSweep(
        lockfile=str(doc.get("lockfile") or ""),
        error=doc.get("error"),
        vulns=vulns,
        warnings=warnings,
    )


def sweep_path(directory: Path, lockfile: str) -> Path:
    return directory / (lockfile.replace("/", "__") + ".json")


def load_snapshots(directory: Path, locks: list[str]) -> dict[str, LockSweep]:
    snapshots: dict[str, LockSweep] = {}
    if not directory.is_dir():
        return snapshots
    for lock in locks:
        path = sweep_path(directory, lock)
        if not path.is_file():
            continue
        try:
            snapshots[lock] = decode_sweep(json.loads(path.read_text(encoding="utf-8")))
        except (OSError, json.JSONDecodeError):
            continue
    return snapshots


def store_sweeps(directory: Path, sweeps: list[LockSweep]) -> None:
    shutil.rmtree(directory, ignore_errors=True)
    directory.mkdir(parents=True)
    for sweep in sweeps:
        sweep_path(directory, sweep.lockfile).write_text(
            json.dumps(encode_sweep(sweep), indent=2), encoding="utf-8"
        )


def sweep_sweep_all(locks: list[str]) -> list[LockSweep]:
    sweeps = [audit_lock(lock) for lock in locks]
    for sweep in sweeps:
        if sweep.error:
            print(f"{ECHO} {sweep.lockfile}: SCAN ERROR - {sweep.error}")
        else:
            print(
                f"{ECHO} {sweep.lockfile}: {len(sweep.vulns)} vulnerability(ies), "
                f"{len(sweep.warnings)} warning(s)"
            )
            for vuln in sweep.vulns:
                print(f"{ECHO}   VULN {vuln.package} {vuln.version} {vuln.advisory_id}")
            for warning in sweep.warnings:
                print(f"{ECHO}   WARNING {warning.package} {warning.version} [{warning.kind}]")
    return sweeps


# ── classification (the pre/post diff) ──────────────────────────────────


def classify(
    pre: dict[str, LockSweep], post: dict[str, LockSweep], locks: list[str]
) -> list[FixedVuln]:
    """Advisories the pre-update tree carried that the post-update tree
    no longer does. A lock whose post scan errored contributes nothing:
    an unreadable post state must never be counted as a fix."""
    fixed: list[FixedVuln] = []
    post_versions = {lock: lockfile_versions(lock) for lock in locks}
    for lock in locks:
        post_sweep = post.get(lock)
        if post_sweep is None or post_sweep.error:
            continue
        post_ids = {v.advisory_id for v in post_sweep.vulns}
        pre_sweep = pre.get(lock)
        for vuln in (pre_sweep.vulns if pre_sweep else []):
            if vuln.advisory_id in post_ids:
                continue
            now = post_versions.get(lock, {}).get(vuln.package, [])
            new_version = ", ".join(sorted(set(now))) or "(gone from the lockfile)"
            fixed.append(
                FixedVuln(
                    lockfile=lock,
                    package=vuln.package,
                    old_version=vuln.version,
                    new_version=new_version,
                    advisory_id=vuln.advisory_id,
                    title=vuln.title,
                    aliases=vuln.aliases,
                )
            )
    return fixed


def advisory_label(advisory_id: str, aliases: tuple[str, ...]) -> str:
    """The advisory id with its CVE alias when one exists - the owner
    reads CVE ids, the RustSec id is the canonical reference."""
    cve = next((a for a in aliases if a.startswith("CVE-")), None)
    return f"{advisory_id} ({cve})" if cve else advisory_id


def stuck_reason(vuln: Vuln) -> str:
    if vuln.patched:
        reachable = ", ".join(vuln.patched)
        return (
            f"a patched release exists ({reachable}) but no requirement in this "
            "tree reaches it - move the manifest requirement(s) deliberately"
        )
    return "no patched release is published yet - watch upstream, or remove/replace the dependency"


# ── the job summary and the verdict ─────────────────────────────────────


def markdown_table(headers: list[str], rows: list[list[str]]) -> list[str]:
    lines = ["| " + " | ".join(headers) + " |", "| " + " | ".join(["---"] * len(headers)) + " |"]
    for row in rows:
        lines.append("| " + " | ".join(cell.replace("|", "\\|") for cell in row) + " |")
    return lines


def write_summary(text: str) -> None:
    summary_path = os.environ.get("GITHUB_STEP_SUMMARY")
    if summary_path:
        with open(summary_path, "a", encoding="utf-8") as handle:
            handle.write(text)
    else:
        print(text, end="")


def cmd_pre() -> int:
    """Snapshot the HEAD tree. Findings are data here, never a verdict:
    this subcommand never exits 1 on advisories (the updater and the
    post sweep still get their chance)."""
    locks = tracked_lockfiles()
    if not locks:
        print(f"{ECHO} no tracked Cargo.lock found", file=sys.stderr)
        return 1
    print(f"{ECHO} pre-update snapshot over {len(locks)} lockfile(s): {', '.join(locks)}")
    store_sweeps(PRE_DIR, sweep_sweep_all(locks))
    return 0


def cmd_post() -> int:
    """Re-sweep, classify against the pre snapshot, write the summary and
    the verdict. Always exits 0: the verdict subcommand owns the exit-1
    point so the commit step can still land the fixable bumps."""
    locks = tracked_lockfiles()
    if not locks:
        print(f"{ECHO} no tracked Cargo.lock found", file=sys.stderr)
        return 1
    pre = load_snapshots(PRE_DIR, locks)
    post_list = sweep_sweep_all(locks)
    store_sweeps(POST_DIR, post_list)
    post = {sweep.lockfile: sweep for sweep in post_list}

    post_vulns = [v for sweep in post_list for v in sweep.vulns]
    post_warnings = [w for sweep in post_list for w in sweep.warnings]
    post_errors = [sweep for sweep in post_list if sweep.error]
    pre_errors = [sweep for sweep in pre.values() if sweep.error]
    fixed = classify(pre, post, locks)

    red = bool(post_vulns or post_warnings or post_errors)
    VERDICT_FILE.write_text("red" if red else "green", encoding="utf-8")

    if fixed:
        note_lines = [f"security sweep: {len(fixed)} urgent dependency fix(es) landed:"]
        note_lines += [
            f"- {f.lockfile}: {f.package} {f.old_version} -> {f.new_version} cleared {f.advisory_id}"
            for f in fixed
        ]
        COMMIT_NOTE_FILE.write_text("\n".join(note_lines) + "\n", encoding="utf-8")
    else:
        COMMIT_NOTE_FILE.unlink(missing_ok=True)

    write_summary(build_report(locks, pre, post_list, fixed, red))
    print(f"{ECHO} fixed this run: {len(fixed)}; remaining findings: "
          f"{len(post_vulns)} vulnerability(ies), {len(post_warnings)} warning(s), "
          f"{len(post_errors)} scan error(s)")
    print(f"{ECHO} verdict: {'RED - CALL THE OWNER' if red else 'GREEN'}")
    return 0


def build_report(
    locks: list[str],
    pre: dict[str, LockSweep],
    post_list: list[LockSweep],
    fixed: list[FixedVuln],
    red: bool,
) -> str:
    lines: list[str] = []
    lines.append("## Security sweep - the Sunday supply-chain bot")
    lines.append("")
    lines.append(
        f"Swept {len(locks)} tracked lockfile(s): {', '.join(f'`{lock}`' for lock in locks)}."
    )
    lines.append("")
    pre_vulns = sum(len(sweep.vulns) for sweep in pre.values())
    pre_warnings = sum(len(sweep.warnings) for sweep in pre.values())
    pre_errors = sum(1 for sweep in pre.values() if sweep.error)
    if not pre:
        lines.append(
            "The pre-update snapshot is missing - the fixed-this-run list below "
            "cannot be trusted for completeness (the post-update verdict below "
            "is unaffected)."
        )
        lines.append("")
    post_vulns = [v for sweep in post_list for v in sweep.vulns]
    post_warnings = [w for sweep in post_list for w in sweep.warnings]
    post_errors = [sweep for sweep in post_list if sweep.error]
    lines.append(
        f"Pre-update tree: {pre_vulns} vulnerable package(s), "
        f"{pre_warnings} owner-review warning(s), {pre_errors} scan error(s)."
    )
    lines.append(
        f"Post-update tree: {len(post_vulns)} vulnerable package(s), "
        f"{len(post_warnings)} owner-review warning(s), {len(post_errors)} scan error(s)."
    )
    lines.append("")

    lines.append(f"### Fixed this run ({len(fixed)} urgent auto-update(s))")
    lines.append("")
    if fixed:
        rows = [
            [
                f.lockfile,
                f.package,
                f.old_version,
                f.new_version,
                advisory_label(f.advisory_id, f.aliases),
                f.title,
            ]
            for f in fixed
        ]
        lines += markdown_table(
            ["lockfile", "package", "was", "now", "advisory", "title"], rows
        )
    else:
        lines.append(
            "None - the pre-update tree carried no advisory an in-range bump could clear."
        )
    lines.append("")

    lines.append(f"### CALL THE OWNER ({len(post_vulns)} dangerous dependency finding(s))")
    lines.append("")
    if post_vulns:
        rows = [
            [
                v.lockfile,
                f"{v.package} {v.version}",
                advisory_label(v.advisory_id, v.aliases),
                v.title,
                stuck_reason(v),
            ]
            for v in post_vulns
        ]
        lines += markdown_table(
            ["lockfile", "package version", "advisory", "title", "why the bot is stuck"], rows
        )
    else:
        lines.append("None.")
    lines.append("")

    lines.append(
        f"### Owner review ({len(post_warnings)} warning(s) - the same bar the push lane fails on)"
    )
    lines.append("")
    if post_warnings:
        rows = [
            [w.lockfile, f"{w.package} {w.version}", w.kind, w.advisory_id, w.title]
            for w in post_warnings
        ]
        lines += markdown_table(
            ["lockfile", "package version", "kind", "advisory", "title"], rows
        )
    else:
        lines.append("None.")
    lines.append("")

    lines.append(f"### Scan errors ({len(post_errors)} - a scanner that cannot run is a red verdict)")
    lines.append("")
    if post_errors:
        rows = [[sweep.lockfile, sweep.error or ""] for sweep in post_errors]
        lines += markdown_table(["lockfile", "detail"], rows)
    else:
        lines.append("None.")
    lines.append("")

    if red:
        lines.append(
            f"**Verdict: RED - CALL THE OWNER** - {len(post_vulns)} vulnerable package(s), "
            f"{len(post_warnings)} warning(s) and {len(post_errors)} scan error(s) remain after "
            "the updater's best effort. The update commit (if any) already landed; finish the "
            "rest deliberately. Details above."
        )
    elif fixed:
        lines.append(
            f"**Verdict: GREEN** - the tree was exposed before the sweep ({pre_vulns} vulnerable "
            f"package(s) on the pre-update tree), and the urgent auto-update(s) above cleared "
            "every one of them. No finding remains."
        )
    else:
        lines.append("**Verdict: GREEN** - no advisory on any tracked lockfile.")
    return "\n".join(lines) + "\n"


def cmd_verdict() -> int:
    """The only exit-1 point of the sweep, run from the job's final step
    (`if: always()`): a missing verdict file means the sweep never
    completed, and an incomplete sweep is RED - it must never read as green."""
    if not VERDICT_FILE.is_file():
        print(
            f"{ECHO} verdict file missing - the sweep did not complete; "
            "treat the run as RED (call the owner)",
            file=sys.stderr,
        )
        return 1
    verdict = VERDICT_FILE.read_text(encoding="utf-8").strip()
    if verdict == "green":
        print(f"{ECHO} verdict: GREEN - clean or fully auto-fixed; see the job summary")
        return 0
    print(
        f"{ECHO} verdict: RED - CALL THE OWNER; the update commit already landed, "
        "finish the rest deliberately (see the job summary)"
    )
    return 1


def main(argv: list[str]) -> int:
    commands = {"pre": cmd_pre, "post": cmd_post, "verdict": cmd_verdict}
    if len(argv) != 1 or argv[0] not in commands:
        print("usage: security-sweep.py {pre|post|verdict}", file=sys.stderr)
        return 2
    SWEEP_DIR.mkdir(parents=True, exist_ok=True)
    return commands[argv[0]]()


if __name__ == "__main__":
    raise SystemExit(main(sys.argv[1:]))
