#!/usr/bin/env python3
"""Fail a release build if the shipped exe needs a DLL Windows does not have.

`PalantirMC.exe` is downloaded by people who will not install a toolchain, so
the one packaging property that must never regress is that the import table
names nothing but Windows itself. This checks it by parsing the PE headers --
no execution, no `dumpbin`, no MSVC.

The gate is deliberately one-directional: it *fails* only on a known
non-Windows runtime (the mingw/GNU family, which a dependency can start
pulling in dynamically without any manifest change), and merely *reports*
everything else. An allowlist would turn a future Windows component we have
not heard of into a broken release; this cannot.

Usage:  python .github/scripts/check_exe.py path/to/PalantirMC.exe
"""

from __future__ import annotations

import struct
import sys

# Families that mean the download only runs where a compiler or a
# redistributable happens to be installed. `libgcc_s_*`/`libstdc++-6` come from
# a GNU target, `*winpthread*` from an unstatic-linked mingw, and
# `vcruntime140`/`msvcp140` are the MSVC equivalent of the same mistake.
# Both directions have been observed here: the MSVC build was the one importing
# VCRUNTIME140.dll, while the GNU build imported nothing outside Windows.
# The `api-ms-win-crt-*` set is Microsoft's own UCRT and is delivered with the
# OS, so it is deliberately *not* listed here.
NON_WINDOWS = (
    "libgcc_s_dw2-1",
    "libgcc_s_seh-1",
    "libstdc++-6",
    "libwinpthread-1",
    "libssp-0",
    "vcruntime140",
    "msvcp140",
)


def imports(path: str) -> tuple[str, list[str]]:
    data = open(path, "rb").read()
    if data[:2] != b"MZ":
        raise SystemExit(f"error: {path} is not a PE file")
    pe = struct.unpack_from("<I", data, 0x3C)[0]
    if data[pe:pe + 4] != b"PE\0\0":
        raise SystemExit(f"error: {path} has no PE signature")

    coff = pe + 4
    machine, sections_n, _, _, _, opt_size, _ = struct.unpack_from("<HHIIIHH", data, coff)
    opt = coff + 20
    magic = struct.unpack_from("<H", data, opt)[0]
    if magic == 0x20B:
        dirs = opt + 112  # PE32+
    elif magic == 0x10B:
        dirs = opt + 96  # PE32
    else:
        raise SystemExit(f"error: {path} has unknown optional-header magic {magic:#x}")

    # Field order is Name, VirtualSize, VirtualAddress, SizeOfRawData,
    # PointerToRawData -- size precedes address.
    table = []
    base = opt + opt_size
    for i in range(sections_n):
        head = base + i * 40
        vsize, va, raw_size, raw_ptr = struct.unpack_from("<IIII", data, head + 8)
        table.append((va, max(vsize, raw_size), raw_ptr))

    def offset(rva: int) -> int | None:
        for va, size, raw_ptr in table:
            if va <= rva < va + size:
                return raw_ptr + (rva - va)
        return None

    import_rva, import_size = struct.unpack_from("<II", data, dirs + 8)
    found: list[str] = []
    if import_rva:
        at = offset(import_rva)
        # Bounded by the declared directory: past the terminating null
        # descriptor the following bytes are arbitrary data.
        for _ in range(max(import_size // 20, 1)):
            if at is None or at + 20 > len(data):
                break
            first_thunk, _, _, name_rva, _ = struct.unpack_from("<IIIII", data, at)
            if not (first_thunk or name_rva):
                break
            name_at = offset(name_rva)
            if name_at is not None:
                end = data.find(b"\0", name_at, min(name_at + 260, len(data)))
                if end != -1:
                    found.append(data[name_at:end].decode("latin1"))
            at += 20

    arch = {0x8664: "x64", 0x014C: "x86", 0xAA64: "arm64"}.get(machine, f"{machine:#x}")
    return arch, found


def main(argv: list[str]) -> int:
    if len(argv) < 2:
        raise SystemExit(__doc__)

    status = 0
    for path in argv[1:]:
        arch, found = imports(path)
        unique = sorted({name for name in found})
        print(f"{path} ({arch}) imports {len(unique)} DLL(s):")
        for name in unique:
            print(f"  {name}")

        portability = [name for name in unique if any(pat in name.lower() for pat in NON_WINDOWS)]
        if portability:
            status = 1
            print(f"error: {path} depends on a non-Windows runtime:")
            for name in portability:
                print(f"  {name}")
            print(
                "hint: link the C runtime statically -- `-C target-feature=+crt-static` "
                "in RUSTFLAGS -- or the download needs a redistributable installed first."
            )
        else:
            print(f"ok: {path} needs nothing beyond Windows' own DLLs")

    return status


if __name__ == "__main__":
    sys.exit(main(sys.argv))
