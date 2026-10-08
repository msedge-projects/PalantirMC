# Third-party notices

Everything this product ships that we did not write is listed in the **File
manifest** below with its origin and its licence. The manifest is not
bookkeeping: `tools/licence_audit.py` treats it as the authority and fails on
any shipped file it does not list, so nothing can reach the product unnoticed.

Third-party material is allowed only under a permissive licence — ISC, MIT,
Apache-2.0, or a font licence such as the SIL Open Font Licence 1.1 — and only
when its own notice travels beside the files. Nothing here is taken from
another application's bundled copy of anything: icons come from Lucide's own
package, fonts from their foundry.

## Dependencies

None yet. Every runtime dependency joins this file with its name, version,
licence and the one-line reason it is in the tree.

## File manifest

One row per file — or one glob over a single directory of files — with who
made it and under what terms. A row that matches nothing in the tree is a
failure too, so the manifest cannot rot.

| Path | Origin | Licence |
| --- | --- | --- |
| `crates/palantir-core/tests/fixtures/*` | Mojang Studios public metadata API responses (piston-meta/launchermeta, fetched 2026-10-08) | © Mojang AB, all rights reserved; API response data (facts), kept as test fixtures |
<!-- Phase 0 ships no assets. First expected rows: the Lucide icons and the
     Lucide `LICENSE` beside them (origin: Lucide, ISC), and the Inter faces
     with `OFL.txt` (origin: Inter, SIL OFL 1.1). -->
