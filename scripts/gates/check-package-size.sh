#!/usr/bin/env bash
# SPDX-FileCopyrightText: 2025-2026 rezky_nightky (oxyzenQ)
# SPDX-License-Identifier: AGPL-3.0-only OR Commercial
# OS: Linux only — zylaxion is a Linux keyboard-sound tool; no other OS is supported.
#
# ZYLAXION PACKAGE SIZE CHECK (night-cafe-mclaren-5)
#
# The gate the 413 incident lacked. On 2026-11-27 a publish attempt
# died at the crates.io upload endpoint with the server's own verdict:
#
#   status 413 Payload Too Large: max upload size is: 10485760
#
# The tarball had crossed the cap while every gate that existed stayed
# green — size was nobody's axis. The structural fix that landed is
# night-dinner-11's publish allowlist (include-mode in Cargo.toml);
# this gate is the detector that axis was missing: the publish
# tarball's byte count must stay under the crates.io upload cap,
# measured on the real cargo package output — locally after any
# manifest or asset change, and in the publish workflow BEFORE the
# upload step. The law is strictly smaller-than: the server's message
# names 10485760 as the maximum the endpoint accepts, so a tarball at
# exactly the cap fails here instead of gambling on the server.
#
# Failure diagnostics: the gate lists the ten largest members of the
# tarball, so the next 413-class incident names its own culprit in the
# log instead of sending the maintainer hunting with tar by hand.
#
# Usage: bash scripts/gates/check-package-size.sh
#
# Platform: UNIX-only (uses `stat`, `tar`, `awk`). Not for Windows cmd.exe.

set -euo pipefail

# The root-owned __pycache__ wall, shell layer (night-cafe-1): any
# interpreter this script spawns inherits dont-write-bytecode, so no
# repo-local bytecode lands beside the scripts at any privilege level.
export PYTHONDONTWRITEBYTECODE=1

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
cd "$REPO_ROOT"

# The crates.io upload cap, verbatim from the server's 413 response
# (the incident log, 2026-11-27): "max upload size is: 10485760".
# If crates.io ever changes the limit, this constant changes with it —
# the gate follows the server's truth, never a private number.
CAP=10485760

VERSION="$(grep '^version = ' Cargo.toml | head -1 | sed 's/version = "\(.*\)".*/\1/')"
if [ -z "$VERSION" ]; then
    echo "FAIL: could not parse version from Cargo.toml"
    exit 1
fi

# The measured artifact, packaged exactly like the publish lane
# packages it: --locked (the committed dependency set, never a fresh
# resolve), --no-verify (the size is the file walk's verdict, not a
# build's — the compile lanes own compilation), --allow-dirty (the
# gate also runs pre-push locally, where the tree is by definition
# not clean; on a CI checkout it is a no-op).
CRATE="target/package/zylaxion-${VERSION}.crate"
if ! cargo package --locked --no-verify --allow-dirty --quiet; then
    echo "FAIL: cargo package could not build the publish tarball"
    exit 1
fi
if [ ! -f "$CRATE" ]; then
    echo "FAIL: expected tarball ${CRATE} is missing after cargo package"
    exit 1
fi

SIZE="$(stat -c '%s' "$CRATE")"

if [ "$SIZE" -ge "$CAP" ]; then
    echo "FAIL: publish tarball ${CRATE} is ${SIZE} bytes, the crates.io cap is ${CAP}"
    echo "      the upload would die with 413 Payload Too Large — trim the tarball."
    echo "      The ten largest members:"
    tar -tzvf "$CRATE" | awk '{print $3, $NF}' | sort -rn | head -10 | sed 's/^/        /'
    exit 1
fi

HEADROOM=$((CAP - SIZE))
PERCENT=$((SIZE * 100 / CAP))
echo "OK: publish tarball ${CRATE} is ${SIZE} bytes (${PERCENT}% of the ${CAP}-byte crates.io cap, ${HEADROOM} bytes of headroom)"
exit 0
