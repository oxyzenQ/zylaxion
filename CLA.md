<!-- Copyright (C) 2026 rezky_nightky -->
<!-- SPDX-License-Identifier: GPL-3.0-only -->

# Contributor License Agreement (CLA)

Thank you for your interest in contributing to **zylaxion**, maintained
by **rezky_nightky (oxyzenQ)** ("the Owner").

By submitting a pull request, patch, issue comment containing code, or
any other contribution (collectively, a "Contribution") to this
repository, you agree to the following terms.

## 1. Grant of Rights

You grant the Owner a **perpetual, worldwide, non-exclusive,
royalty-free, irrevocable** license to:

- Use, reproduce, modify, and distribute your Contribution.
- **Relicense your Contribution under any license**, including but not
  limited to GPL-3.0-only, commercial licenses, or any future license
  the Owner chooses.

This grant is necessary because zylaxion is dual-licensed
(GPL-3.0-only + Commercial). Without it, the Owner cannot legally
offer commercial licenses for code that includes your Contribution.

## 2. Ownership

You **retain full copyright** to your Contribution. This CLA does not
transfer ownership — it only grants the Owner the rights described in
Section 1.

## 3. Original Work

You confirm that:

- Your Contribution is your **original work**, or you have the legal
  right to submit it.
- Your Contribution does **not** violate any third-party copyright,
  patent, trademark, or trade secret.
- If your employer has rights over work you create, you have received
  permission to submit the Contribution, or your employer has waived
  such rights.

## 4. No Warranty

Your Contribution is provided **"as is"**, without any warranty,
express or implied, including but not limited to warranties of
merchantability or fitness for a particular purpose.

## 5. Acceptance

There is no separate document to sign. Acceptance is automatic and
verifiable by any of:

- The DCO-style `Signed-off-by:` line on your commit
  (`git commit -s` appends it from your configured git identity).
- A comment on the pull request stating: "I have read the CLA and I
  agree."

The `Signed-off-by:` route is preferred — it is machine-checkable and
rides the commit forever.

## 6. Contribution Discipline

Contributions land through the same gates the Owner's own work does:

- **English-only tree** — comments, docs, script text, and string
  literals alike. Chat with the Owner may be mixed-language; committed
  artifacts never are.
- **The 600-line law** — every `.rs` file stays under 600 lines
  (scripts under `scripts/` carry a 1000-line cap). Over the cap means
  split, not exempt.
- **Gate-keepers green** — `./scripts/build.sh check-all -q` and
  `./scripts/gate-keepers.sh` must pass on every commit you propose.
- **Commit shape** — micro commits, one purpose each, pure English,
  no emoji, with the `Signed-off-by:` line.
- **No version bumps, no tags** — the Owner alone decides versions and
  release tags. A contribution that touches version fields will be
  asked to drop that change.

The full contributor walkthrough lives in
[CONTRIBUTING.md](CONTRIBUTING.md).
