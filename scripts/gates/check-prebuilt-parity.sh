#!/usr/bin/env bash
# Copyright (C) 2026 rezky_nightky
# SPDX-License-Identifier: GPL-3.0-only
# OS: Linux only — zylaxion is a Linux eBPF tool; no other OS is supported.
#
# ZYLAXION PREBUILT eBPF PARITY CHECK (night-improve-31, the zelynic
# NIGHT-ask-2 shape ported to zylaxion's ONE object).
#
# Guards the ebpf-prebuilt/ lane — the maintainer-built object that
# makes `cargo install zylaxion` land the full-featured binary on a
# plain stable toolchain (cargo's package walk cannot carry the
# detached ebpf/ workspace, so the registry tarball ships this object
# instead; provenance: ebpf-prebuilt/manifest.toml).
#
# What is checked, and why each row is load-bearing:
#   1. The object exists and is a structurally sane ELF (magic +
#      minimum size). A missing or truncated object would make the
#      registry lane's build.rs panic on every `cargo install`.
#   2. Every manifest [[object]] row matches the file on disk
#      (sha256 + size). A tampered or partially refreshed directory
#      fails here instead of shipping an unaccounted artifact.
#   3. The manifest's ebpf_tree_sha256 equals the sha256 over the
#      LC_ALL=C byte-sorted (NUL-separated) sha256sum listing of the
#      git-tracked ebpf/ files — the STALENESS killer: any change
#      under ebpf/ (sources, locks, pins, target spec) changes the
#      tree hash, and the gate fails until the lane is regenerated.
#      Since night-dinner-4 the refresh is automatic: every
#      ebpf-feature build runs the generator's --check verdict
#      (build/ebpf.rs refresh_prebuilt_lane) and a stale lane
#      regenerates itself, so this gate is the second line of defense
#      (dormant builds and refresh-less checkouts are what it still
#      catches). scripts/dev/regenerate-ebpf-prebuilt.sh stays the
#      one writer.
#      The shipped object can never silently fall behind the sources
#      it claims to carry. The v80.0.0-beta.1 incident (the finding
#      that commissioned this gate) rode exactly this shape: the
#      release commit touched ebpf/Cargo.toml and ebpf/Cargo.lock,
#      nothing forced the lane refresh, and the manifest pin shipped
#      describing a tree the checkout no longer matched.
#   4. No tracked file under ebpf/ hides behind an assume-unchanged
#      or skip-worktree flag — the PHANTOM-PIN killer: the tree hash
#      in row 3 reads on-disk content, so a hidden file's local churn
#      hashes into the pin while the commit stages the old bytes; the
#      pin then passes locally and fails CI's clean checkout.
#
# Deliberately NOT checked: a rebuild-and-byte-compare of the object.
# Byte reproducibility is a host property, not a contract — the
# tree-hash pin is deterministic on every Linux and is the enforcement
# surface. The same discipline as check-release-parity.sh: pin what
# is deterministic, document what is not.
#
# The manifest is generator-controlled (regenerate-ebpf-prebuilt.sh
# writes a flat-key header plus one [[object]] block per object), so
# parsing is deliberately simple awk over that exact shape.
#
# The recipe here is kept in lockstep with
# scripts/dev/regenerate-ebpf-prebuilt.sh (which is the one way the
# pin is made), so the two implementations cannot drift apart
# silently. Path sensitivity is the property that earns the shape:
# the hashed listing carries each file's path, so a content-preserving
# rename under ebpf/ still stales the pin — the pin describes the
# tree, not just its content multiset.
#
# Usage: bash scripts/gates/check-prebuilt-parity.sh
set -euo pipefail

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
cd "$REPO_ROOT"

PREBUILT_DIR="ebpf-prebuilt"
MANIFEST="${PREBUILT_DIR}/manifest.toml"
FAILED=0

fail() {
	echo "FAIL: $*"
	FAILED=$((FAILED + 1))
}

pass() {
	echo "PASS: $*"
}

# sha256 over the LC_ALL=C byte-sorted (NUL-separated) sha256sum
# listing of the git-tracked ebpf/ files. The LC_ALL=C pin is
# load-bearing: a bare `sort -z` follows the invoking shell's
# collation, and a UTF-8 desktop locale reorders the very list a C
# locale sorts by raw bytes — one identical ebpf/ tree, clean
# `git status` on both sides, hashed to two different pins. LC_ALL=C
# makes the pin deterministic on every Linux, the property the
# manifest header always claimed.
ebpf_tree_sha() {
	git ls-files -z -- ebpf/ |
		LC_ALL=C sort -z |
		xargs -0 -r sha256sum |
		sha256sum |
		awk '{print $1}'
}

# ── 1. the lane exists and its object is structurally sane ────────
if [ ! -d "$PREBUILT_DIR" ]; then
	fail "ebpf-prebuilt/ is missing — the registry lane cannot ship. Run ./scripts/dev/regenerate-ebpf-prebuilt.sh"
	echo "FAIL: 1 of 4 checks (lane missing)"
	exit 1
fi
if [ ! -f "$MANIFEST" ]; then
	fail "ebpf-prebuilt/manifest.toml is missing — regenerate with ./scripts/dev/regenerate-ebpf-prebuilt.sh"
	FAILED=$((FAILED + 1))
else
	pass "manifest present (${MANIFEST})"
fi

# One object today; the loop stays the manifest's growth path (a
# second pinned object slots in here without a rewrite).
# shellcheck disable=SC2043
for name in zylaxion-tap; do
	file="${PREBUILT_DIR}/${name}"
	if [ ! -f "$file" ]; then
		fail "${file} is missing — regenerate with ./scripts/dev/regenerate-ebpf-prebuilt.sh"
		continue
	fi
	magic="$(head -c 4 "$file" | od -An -tx1 | tr -d ' \n')"
	if [ "$magic" != "7f454c46" ]; then
		fail "${file} is not an ELF file (magic ${magic})"
		continue
	fi
	size="$(stat -c%s "$file")"
	if [ "$size" -lt 64 ]; then
		fail "${file} is only ${size} bytes — too short for an ELF64 header"
		continue
	fi
	pass "${name} is a structurally sane ELF (${size} bytes)"
	# The RELOCATABLE-SYMBOL killer: aya's loader resolves map
	# relocations and in-section function relocations; any GLOBAL
	# UNDEFINED FUNC symbol it must instead refuse with
	# "function 0xADDR not found while relocating <prog>" at LOAD
	# time — on the CI VMs, the only place the object ever loads.
	# A plain `readelf -s` over the shipped object catches the class
	# BEFORE the push: the null symbol (index 0) and pure-NOTYPE
	# locals are the ELF spec's own furniture; a GLOBAL UNDEFINED
	# symbol is a call the loader cannot make. readelf is a binutils
	# tool the GitHub runner image carries; a host without it gets an
	# honest skip note, never a false-green claim.
	if command -v readelf >/dev/null 2>&1; then
		undef_globals="$(readelf -sW "$file" 2>/dev/null |
			awk '$7 == "UND" && $5 == "GLOBAL" {print $8}' |
			grep -v '^$' || true)"
		if [ -n "$undef_globals" ]; then
			fail "${name} carries undefined GLOBAL symbols the aya loader cannot relocate: ${undef_globals}"
		else
			pass "${name} has no undefined GLOBAL symbols (the loader's relocation surface is closed)"
		fi
	else
		echo "SKIP: readelf not found — the GLOBAL-symbol row on ${name} is unverified here (CI carries binutils and runs this row)"
	fi
done

# ── 2. every manifest row matches the file on disk ───────────────
if [ -f "$MANIFEST" ]; then
	# One line per [[object]] block: "name sha256 size"
	while read -r name want_sha want_size; do
		[ -n "$name" ] || continue
		file="${PREBUILT_DIR}/${name}"
		if [ ! -f "$file" ]; then
			fail "manifest names ${name} but the file is absent"
			continue
		fi
		got_sha="$(sha256sum "$file" | awk '{print $1}')"
		got_size="$(stat -c%s "$file")"
		if [ "$got_sha" != "$want_sha" ]; then
			fail "${name} sha256 drift: manifest ${want_sha}, disk ${got_sha}"
		elif [ "$got_size" != "$want_size" ]; then
			fail "${name} size drift: manifest ${want_size}, disk ${got_size}"
		else
			pass "${name} matches its manifest row (sha256 + size)"
		fi
	done < <(
		awk '
			/^\[\[object\]\]/ {
				if (name != "") print name, sha, size
				name = ""; sha = ""; size = ""
			}
			/^name = / { gsub(/"/, "", $3); name = $3 }
			/^sha256 = / { gsub(/"/, "", $3); sha = $3 }
			/^size = / { size = $3 }
			END { if (name != "") print name, sha, size }
		' "$MANIFEST"
	)

	# ── 3. the manifest's tree pin equals the live ebpf/ tree ──────
	want_tree="$(awk -F'"' '/^ebpf_tree_sha256 = /{print $2}' "$MANIFEST")"
	got_tree="$(ebpf_tree_sha)"
	if [ -z "$want_tree" ]; then
		fail "manifest carries no ebpf_tree_sha256 — regenerate with ./scripts/dev/regenerate-ebpf-prebuilt.sh"
	elif [ "$want_tree" != "$got_tree" ]; then
		fail "ebpf/ changed after the prebuilt lane was generated (tree ${got_tree} vs manifest ${want_tree}) — run: cd ebpf && cargo build --release && ./scripts/dev/regenerate-ebpf-prebuilt.sh, then commit ebpf-prebuilt/. A locally MODIFIED tracked file under ebpf/ trips this too (a local pro build rewriting ebpf/Cargo.lock is the known shape): git status -- ebpf/ shows the churn — restore it before refreshing, or the refresh pins the churn"
	else
		pass "prebuilt objects pin the live ebpf/ tree (${got_tree:0:12}...)"
	fi

	# ── 4. no tracked file under ebpf/ hides churn from the pin ────
	# The phantom-pin guard: the tree hash in row 3 reads ON-DISK
	# content over the tracked list, so a file hidden behind
	# assume-unchanged (h) or skip-worktree (S) hashes its local churn
	# into the pin while the commit stages the old bytes — the pin
	# passes locally, then fails CI's clean checkout. The row refuses
	# that shape here, at commit time, with the unhide remedy named.
	hidden_churn="$(git ls-files -v -- ebpf/ | awk '$1 ~ /^[hS]$/ {print $2}')"
	if [ -n "$hidden_churn" ]; then
		fail "tracked ebpf/ file(s) hide behind assume-unchanged/skip-worktree flags — the tree pin may describe churn the commit cannot carry ($(echo "$hidden_churn" | tr '\n' ' ')) — unhide each (git update-index --no-assume-unchanged --no-skip-worktree <file>), restore or stage the change, and re-run ./scripts/dev/regenerate-ebpf-prebuilt.sh"
	else
		pass "no hidden-churn flags under ebpf/ (the pin describes a shippable tree)"
	fi
fi

if [ "$FAILED" -eq 0 ]; then
	echo "OK: ebpf-prebuilt lane in parity (object + manifest + ebpf/ tree pin)"
	exit 0
else
	echo "FAIL: ${FAILED} prebuilt-parity failure(s) — the registry lane is not shippable"
	exit 1
fi
