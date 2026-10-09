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

Direct runtime dependencies, with the one line each was justified by. The
full transitive graph lives in `Cargo.lock` and its licences are checked on
every push by `tools/licence_audit.py` walking `cargo metadata`.

| Crate | Version | Licence | Why it is here |
| --- | --- | --- | --- |
| `serde` | 1 | MIT OR Apache-2.0 | The metadata formats are JSON documents; this parses them |
| `serde_json` | 1 | MIT OR Apache-2.0 | JSON reading and writing for the same documents |
| `regex` | 1 | MIT OR Apache-2.0 | Rule conditions on OS versions are specified as regular expressions |
| `reqwest` | 0.12 | MIT OR Apache-2.0 | Pooled HTTP with blocking reads; the rewrite spec names this client |
| `sha1` | 0.10 | MIT OR Apache-2.0 | Metadata hashes are SHA-1, so downloads verify SHA-1 |

TLS inside `reqwest` is rustls over the **operating system's certificate
store** (`rustls-tls-native-roots`). The bundled-root-lists alternative
(`webpki-roots`) is under a weak share-alike licence and is deliberately not
in the dependency graph. The rustls stack itself (rustls, ring,
rustls-native-certs and their dependencies) is MIT / ISC / Apache-2.0.

Workspace siblings `palantir-core` and `palantir-theme` are this product's
own source under this product's licence.

**Licence elections.** Where a dependency offers a choice of licences, this
product takes the permissive arm and records the choice here:

| Crate | Offered | Elected |
| --- | --- | --- |
| `r-efi` (transitive, via `ring`) | MIT OR Apache-2.0 OR LGPL-2.1-or-later | **MIT** |

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
