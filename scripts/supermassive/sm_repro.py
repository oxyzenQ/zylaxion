# Copyright (C) 2026 rezky_nightky
# SPDX-License-Identifier: AGPL-3.0-only
"""The reproducible-build attestation leg (night-frontier-2B).

Two independent pro-alias builds from one commit, SOURCE_DATE_EPOCH
pinned, two target directories — the shipped script
(scripts/dev/verify-reproducible-build.sh) owns the law and the
measurement; this leg audits its verdict so the claims ledger carries
the attestation. The binary verdict (byte-identical or not) is
RECORDED, never swallowed: the stamps are the gate, the full binary
equality is the stronger property the toolchain may or may not grant.
"""

import re

from sm_env import run
from sm_report import Ledger, Step

#: The two-build proof compiles the whole dependency tree twice; the
#: guard is generous, the instrument's own wall time has been ~2-3 min
#: warm on the claims lane.
SCRIPT_TIMEOUT_S = 900


def battery(ledger: Ledger, binary: str, tree_root: str, root_lane: bool) -> None:
    """Audit the reproducible-build attestation on this tree."""
    with Step(ledger, "reproducible build — the two-build stamp attestation"):
        script = f"{tree_root}/scripts/dev/verify-reproducible-build.sh"
        rc, out = run(["bash", script], timeout=SCRIPT_TIMEOUT_S)
        stamps_pass = "REPRODUCIBLE-BUILD ATTESTATION: PASS" in out
        binary_identical = "BINARY: byte-identical" in out
        if rc == 0 and stamps_pass:
            detail = "two builds, one commit, one lock, SOURCE_DATE_EPOCH pinned"
            if binary_identical:
                detail += "; binaries byte-identical (the full reproducible-builds property)"
            else:
                detail += "; stamps identical, binaries differ (recorded, not hidden)"
            ledger.pass_row(
                "the provenance chain renders the same bytes on two independent builds",
                detail,
            )
        else:
            tail = "\n".join(out.splitlines()[-6:])
            ledger.fail_row(
                "the provenance chain renders the same bytes on two independent builds",
                f"rc={rc}; tail: {tail}",
            )
