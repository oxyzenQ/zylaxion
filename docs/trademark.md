# Trademark & Branding

**Project:** zylaxion
**Author:** rezky_nightky (oxyzenQ)
**Repository:** github.com/oxyzenQ/zylaxion
**License:** GPL-3.0-only
**Contact:** with dot rezky at gmail dot com

The name "zylaxion" and its associated logos, branding, and documentation
are the intellectual property of rezky_nightky.

## DSP Architecture & Algorithms

The Zylaxion DSP architecture, TPT (Topology-Preserving Transform) filter
implementations, and procedural acoustic models are proprietary
mathematical works authored by rezky_nightky (oxyzenQ). These encompass:

- The dual TPT SVF (State Variable Filter) click + spring synthesis chain
  in `zactrix-profiles/src/mechanical.rs`.
- The polyphonic voice pool with oldest-first voice stealing in
  `zactrix-engine/src/pool.rs`.
- The noise excitation + exponential decay envelope model.
- The per-key acoustic override system and TOML preset architecture.
- All DSP parameter validation, clamping, and safe-range guardrails.

Unauthorized commercial redistribution of the code or algorithms without
adhering to the GPL-3.0-only license is strictly prohibited. This
includes but is not limited to:

- Re-packaging the DSP engine into a closed-source product.
- Extracting the TPT filter or acoustic model algorithms into a
  proprietary codebase without releasing the derivative work under
  GPL-3.0-only.
- Selling the software or its algorithms under a different license
  without explicit written permission from the author.

For licensing inquiries, commercial use questions, or permission
requests, contact: **with dot rezky at gmail dot com**.
