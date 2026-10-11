#!/usr/bin/env bash
# Copyright (C) 2026 rezky_nightky
# SPDX-License-Identifier: GPL-3.0-only
# OS: Linux only — zylaxion is a Linux keyboard-sound tool; no other OS is supported.
#
# ZYLAXION SPDX HEADER CHECK
#
# Scans all core/code/config/script/doc files for the required license
# header. Fails if any included file is missing the header or carries
# the wrong license identifier.
#
# Required (both lines, within the first 10 lines of the file):
#   Copyright (C) 2026 rezky_nightky
#   SPDX-License-Identifier: GPL-3.0-only
# Rejected: any other SPDX-License-Identifier value (the project is
# GPL-3.0-only licensed).
#
# Every *.sh ADDITIONALLY requires the OS line within the first 10
# lines — `# OS: Linux only` — the owner's rule, enforced as a failure
# (never a warning): a .sh without it never reaches CI.
#
# Included file types: *.rs, *.c, *.h, *.py, *.sh, *.toml, *.yml, *.yaml, *.md
# Scope: git-tracked files PLUS untracked-but-present files (respecting
#   .gitignore), so a new source file missing its header fails the check
#   BEFORE it can be committed (pre-commit proxy parity).
# Excluded: CHANGELOG.md — a frozen historical record, never rewritten
#   (the same exclusion policy as every other gate).
#
# Usage: bash scripts/gates/check-headers.sh
#
# Platform: UNIX-only (uses `git`, `head`, `grep`). Not for Windows cmd.exe.

set -euo pipefail

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"

RED='\033[0;31m'
NC='\033[0m'

MISSING=0
WRONG=0
CHECKED=0

EXPECTED_COPYRIGHT="Copyright (C) 2026 rezky_nightky"
EXPECTED_LICENSE_NAME="GPL-3.0-only"

while IFS= read -r -d '' file; do
	CHECKED=$((CHECKED + 1))

	if ! head -10 "$file" | grep -q "SPDX-License-Identifier"; then
		echo -e "${RED}MISSING SPDX header: ${file}${NC}"
		MISSING=$((MISSING + 1))
		continue
	fi

	if ! head -10 "$file" | grep -q "SPDX-License-Identifier: ${EXPECTED_LICENSE_NAME}"; then
		echo -e "${RED}WRONG LICENSE (expected ${EXPECTED_LICENSE_NAME}): ${file}${NC}"
		WRONG=$((WRONG + 1))
		continue
	fi

	if ! head -10 "$file" | grep -q "${EXPECTED_COPYRIGHT}"; then
		echo -e "${RED}MISSING copyright line: ${file}${NC}"
		MISSING=$((MISSING + 1))
	fi

	if [[ "$file" == *.sh ]]; then
		if ! head -10 "$file" | grep -q '^# OS: Linux only'; then
			echo -e "${RED}MISSING OS line (os linux only): ${file}${NC}"
			MISSING=$((MISSING + 1))
		fi
	fi
done < <(
	git ls-files --cached --others --exclude-standard 2>/dev/null |
		grep -E '\.(rs|c|h|py|sh|toml|yml|yaml|md)$' |
		grep -v -E '^CHANGELOG\.md$' |
		while IFS= read -r line; do
			printf '%s\0' "${REPO_ROOT}/${line}"
		done
)

TOTAL_FAIL=$((MISSING + WRONG))

if [[ "$TOTAL_FAIL" -eq 0 ]]; then
	echo "OK: $CHECKED files checked, all carry the license header"
	exit 0
else
	echo -e "${RED}FAIL: $MISSING missing, $WRONG wrong license (of $CHECKED checked)${NC}"
	exit 1
fi
