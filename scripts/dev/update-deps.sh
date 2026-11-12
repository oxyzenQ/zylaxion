#!/usr/bin/env bash
# Copyright (C) 2026 rezky_nightky
# SPDX-License-Identifier: AGPL-3.0-only
# OS: Linux only — zylaxion is a Linux eBPF tool; no other OS is
# supported (the recipe is git + cargo + coreutils).
#
# update-deps.sh (night-zen-1) — refresh EVERY Cargo.lock in the repo,
# not just the root one. The weekly lane (Dragon Guard - Dependency
# Maintenance) used to run a bare `cargo update` at the root, which
# only touches the root lock: fuzz/, ebpf/, and scripts/audit/harness/
# each carry their own lock and silently drifted. This script
# auto-detects every tracked manifest (git ls-files, so generated and
# target/ manifests are excluded by construction) and runs
# `cargo update` in each workspace directory.
#
# The eBPF caveat (night-zen-1): ebpf/Cargo.lock is part of the
# git-tracked ebpf/ tree, and the prebuilt lane pins that tree —
# ebpf-prebuilt/manifest.toml's ebpf_tree_blake3 (the
# regenerate-ebpf-prebuilt.sh recipe). An ebpf/ lock refresh therefore
# stales the pin by construction, so when the tree moved this script
# hands the refresh to the ONE WRITER (regenerate-ebpf-prebuilt.sh)
# before exiting: the rebuilt object and the rewritten manifest ride
# the same maintenance commit, and check-prebuilt-parity.sh stays
# green without a second implementation of the recipe. The regen lane
# needs the ebpf build prerequisites (the dated nightly pin,
# bpf-linker, b3sum) — run scripts/dev/bootstrap-ebpf.sh first (the
# maintenance workflow does exactly that, before this script).
#
# Usage: scripts/dev/update-deps.sh
# Exit codes: 0 all locks refreshed (and the prebuilt lane in sync);
# 1 a cargo update failed, or the prebuilt lane could not be
# regenerated after the ebpf tree moved.

set -euo pipefail
cd "$(dirname "$0")/../.." # repo root

# The toolchains live outside the default PATH in some invocation
# contexts (cron runs non-login shells): the bootstrap installs cargo
# tools into ~/.cargo/bin and bpf-linker into ~/.local/bin — the same
# contract regenerate-ebpf-prebuilt.sh applies to its own invocations.
for bin_dir in "${HOME}/.cargo/bin" "${HOME}/.local/bin"; do
	[ -d "${bin_dir}" ] && PATH="${bin_dir}:${PATH}"
done
export PATH

# Every tracked manifest is its own lock surface (the root workspace,
# fuzz/, ebpf/, scripts/audit/harness/). git ls-files is the source of
# truth: only manifests the repo actually tracks get refreshed, and
# build outputs never leak into the list.
MANIFESTS=$(git ls-files -z -- '*Cargo.toml' | LC_ALL=C sort -z | tr '\0' '\n')

if [ -z "$MANIFESTS" ]; then
	echo "update-deps: no tracked Cargo.toml found" >&2
	exit 1
fi

for manifest in $MANIFESTS; do
	dir=$(dirname "$manifest")
	echo "update-deps: cargo update in ${dir}"
	(cd "$dir" && cargo update)
done

# The prebuilt lane must never ship stale behind an ebpf/ refresh:
# when anything under ebpf/ moved (the lock in practice), the tree pin
# is stale by construction — regenerate through the one writer, so the
# rebuilt object and the rewritten manifest ride the same commit.
if ! git diff --quiet -- ebpf/; then
	echo "update-deps: the ebpf tree moved — regenerating the prebuilt lane"
	bash scripts/dev/regenerate-ebpf-prebuilt.sh
	bash scripts/dev/regenerate-ebpf-prebuilt.sh --check
else
	# The tree did not move, but drift can pre-date this run: surface a
	# stale lane loudly instead of letting the parity gate catch it later.
	if ! bash scripts/dev/regenerate-ebpf-prebuilt.sh --check >/dev/null 2>&1; then
		echo "update-deps: prebuilt lane already stale BEFORE this run" >&2
		echo "  refresh by hand: ./scripts/dev/regenerate-ebpf-prebuilt.sh" >&2
		exit 1
	fi
fi

echo "update-deps: all locks refreshed, prebuilt lane in sync"
