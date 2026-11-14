# Copyright (C) 2026 rezky_nightky
# SPDX-License-Identifier: AGPL-3.0-only
"""The kernel matrix leg (night-frontier-2B).

The tap's kernel contract — parse, the measured CO-RE posture, the
relocation path, the fentry target lookup — verified against REAL BTF
snapshots of the LTS floor lines (5.15 / 6.1 / 6.6), extracted from
sha256-pinned distro kernel images. The shipped driver
(scripts/audit/zactrix-kernel-matrix.sh) owns the pin table, the
extraction chain, and the harness invocation; this leg audits its
verdict so the claims ledger carries the matrix. A red here names the
kernel whose BTF the contract does not resolve on.
"""

import re

from sm_env import run
from sm_report import Ledger, Step

#: Cold downloads (~300 MB across the three pinned images) ride the
#: workflow cache; the guard covers a cold lane plus three harness
#: invocations (each compiles the audit harness once).
SCRIPT_TIMEOUT_S = 1500

#: The pinned LTS lines the matrix must cover — the README's "5.15+"
#: floor and the two newer LTS lines.
EXPECTED_KERNELS = ("5.15", "6.1", "6.6")


def battery(ledger: Ledger, binary: str, tree_root: str, root_lane: bool) -> None:
    """Audit the kernel matrix against the pinned LTS BTF snapshots."""
    with Step(ledger, "kernel matrix — the tap's contract on the LTS BTF snapshots"):
        script = f"{tree_root}/scripts/audit/zactrix-kernel-matrix.sh"
        rc, out = run(["bash", script], timeout=SCRIPT_TIMEOUT_S)
        passed = re.findall(r"KERNEL-MATRIX PASS \[([^\]]+)\]", out)
        lines = [ln for ln in passed if ln.startswith(EXPECTED_KERNELS)]
        if rc == 0 and lines:
            ledger.pass_row(
                "the tap's kernel contract resolves on every pinned LTS BTF snapshot",
                f"{len(lines)} snapshots verified: {', '.join(lines)}",
            )
        else:
            tail = "\n".join(out.splitlines()[-6:])
            ledger.fail_row(
                "the tap's kernel contract resolves on every pinned LTS BTF snapshot",
                f"rc={rc}; snapshots verified={lines}; tail: {tail}",
            )
