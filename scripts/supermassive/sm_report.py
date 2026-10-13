# Copyright (C) 2026 rezky_nightky
# SPDX-License-Identifier: GPL-3.0-only
"""The supermassive verdict ledger (night-master-3).

One row per probe, two transports, one source of truth: every battery
stage reports through this ledger, the ledger prints the human row and
the MASS-RESULT sentinel on the same line, and the final rollup emits
MASS-VERDICT. The sentinel lines are the CI relay (the micro-VM cannot
pass exit codes through qemu — .github/workflows/supermassive.yml
greps these lines off the serial console; scripts/ci/supermassive-init.sh
owns the same contract for its own probes). The house voice follows the
zelynic supermassive lineage: a row states the FACT it proved, not a
smiley.

Exit contract: the engine exits 0 only when every row passed. A row
that could not run prints an honest SKIP row (counts as neither pass
nor fail when the skip reason is environmental — the caller decides
via `strict_skips` whether a skip is acceptable for its lane).
"""

from __future__ import annotations

import sys
import time

# ANSI only when stdout is a terminal; the serial console and CI logs
# stay clean text.
_USE_COLOR = hasattr(sys.stdout, "isatty") and sys.stdout.isatty()
_GREEN = "\033[32m" if _USE_COLOR else ""
_RED = "\033[31m" if _USE_COLOR else ""
_YELLOW = "\033[33m" if _USE_COLOR else ""
_NC = "\033[0m" if _USE_COLOR else ""


class Ledger:
    """The battery's single verdict surface."""

    def __init__(self, title: str) -> None:
        self.title = title
        self.rows: list[tuple[str, str, str]] = []  # (kind, name, detail)
        self.failures = 0
        self.skips = 0
        self.strict_skips = False

    def pass_row(self, name: str, detail: str = "") -> None:
        self._emit("PASS", name, detail)

    def fail_row(self, name: str, detail: str = "") -> None:
        self._emit("FAIL", name, detail)

    def skip_row(self, name: str, detail: str = "") -> None:
        self._emit("SKIP", name, detail)

    def info(self, name: str, detail: str = "") -> None:
        """A provenance row: printed, never counted either way."""
        self._emit("NOTE", name, detail)

    def _emit(self, kind: str, name: str, detail: str) -> None:
        self.rows.append((kind, name, detail))
        if kind == "FAIL":
            self.failures += 1
        elif kind == "SKIP":
            self.skips += 1
        tail = f" — {detail}" if detail else ""
        if kind == "PASS":
            color, kind_out = _GREEN, "PASS"
        elif kind == "FAIL":
            color, kind_out = _RED, "FAIL"
        elif kind == "SKIP":
            color, kind_out = _YELLOW, "SKIP"
        else:
            color, kind_out = "", "NOTE"
        print(f"[{kind_out}] {name}{tail}", flush=True)
        # The MASS sentinel carries only PASS/FAIL/SKIP — a NOTE is
        # provenance for the log, never a verdict the grep should see.
        if kind in ("PASS", "FAIL", "SKIP"):
            print(f"MASS-RESULT: {name} — {kind_out}", flush=True)

    def verdict_line(self) -> str:
        if self.failures:
            return (
                f"MASS-VERDICT: FAIL ({self.failures} probe(s) failed, "
                f"{self.skips} skipped)"
            )
        if self.skips and self.strict_skips:
            return (
                f"MASS-VERDICT: FAIL ({self.skips} probe(s) skipped and "
                f"this lane accepts no skips)"
            )
        return (
            f"MASS-VERDICT: PASS ({len(self.rows)} probe(s), "
            f"{self.skips} skipped)"
        )

    def finish(self) -> int:
        """Print the verdict, return the process exit code."""
        print(self.verdict_line(), flush=True)
        if self.failures:
            return 1
        if self.skips and self.strict_skips:
            return 1
        return 0


class Step:
    """A stage banner: one line, self-evident in the serial log."""

    def __init__(self, ledger: Ledger, name: str) -> None:
        self.ledger = ledger
        self.name = name

    def __enter__(self) -> "Step":
        print(f"── {self.name} " + "─" * max(1, 62 - len(self.name)), flush=True)
        return self

    def __exit__(self, exc_type, exc, tb) -> bool:
        if exc is not None:
            self.ledger.fail_row(
                f"{self.name}: the stage crashed",
                f"{exc_type.__name__}: {exc}",
            )
            return True  # swallow: the ledger owns the verdict
        return False
