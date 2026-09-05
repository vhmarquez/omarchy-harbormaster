# Frozen design bundle provenance

The user supplied the outputs bundle for `design-an-omarchy-native-desktop-agent`, dated 2026-09-05. Local source was the `outputs` directory of that named design artifact; repository provenance intentionally does not embed the operator's home path. **Option 02 / Project manager was selected.** This is not approval of every behavior in the supplied code or of newly authored supplemental screens.

## Preservation

- `original/` is a byte-for-byte copy of the entire supplied bundle, not a curated subset or regenerated export: 23 files, including HTML, all composition/state SVG and PNG boards, design notes, the font and its license.
- `original-inventory.json` records every relative path, byte size and SHA-256. `original-sha256.txt` is directly usable from this directory with `sha256sum --check original-sha256.txt`.
- Initial copy was compared by SHA-256 against every source file. Future verification can check the frozen copy without access to the source. Do not modify files below `original/`; fixes and additions belong beside it.
- `01-console-*` and `03-attention-*` remain as rejected composition alternatives/provenance. They do not reopen option selection.
- Supplied PNGs are **static visual boards**, with separately drawn SVG sources. They are not browser screenshots. Their original notes explicitly report unavailable browser verification. Copying them cannot change that fact.

## License and privacy

`original/assets/OFL.txt` preserves the complete SIL Open Font License 1.1 and the copyright notice: Copyright 2020 The JetBrains Mono Project Authors. `JetBrainsMono-Regular.ttf` is unmodified. The font remains under OFL; a repository-wide source license must not be presented as replacing its license.

The supplied design bundle contains no separate license grant for its HTML/SVG/PNG/design notes. Preserve it as user-supplied design material; do not invent an upstream license or attribute it to a third-party product. Project licensing is governed by the separate M0 product/license decision. These notes do not make an independent rights determination.

Original design notes label all session data fictitious. New study data are also fictitious and use explicit `/fixture/…` paths; no actual prompts, terminal content, session database, user credentials or screenshots of live sessions are included. Supplemental HTML uses in-memory state only, no remote assets, no telemetry and no live integration.

## Known original ambiguities — intentionally not patched

1. Compact view labels/counts can crowd; supplemental contract requires separately sized label/count tracks and wrapping, never reducing the type size.
2. Shared-checkout warning says “worktree for isolation.” Supplemental copy says **separate checkout for parallel edits, not a security sandbox**.
3. Original Open terminal wording admits “focus or open”; the new contract reserves it for focusing a verified live terminal and keeps Attach/Resume separate.
4. Original view fixture buckets are exclusive and include unknown activity under Running. The new contract treats views as intersecting projections, includes unreviewed work in Needs you, and separates live/working/unknown counts.
5. Review disposition, settings, full recovery and personal supplemental approval are absent from the originals. Added studies address the design gap; actual user review remains explicitly unclaimed.
