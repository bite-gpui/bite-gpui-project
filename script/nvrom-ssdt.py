#!/usr/bin/env python3
"""Build an SSDT that hands a VBIOS to the guest through the ACPI `_ROM` method.

Why this exists
---------------
On a MUX-less Optimus laptop the discrete GPU carries **no option ROM in its PCI
ROM BAR** -- a host read gives EIO and `lspci` shows the ROM as `[disabled]`.  The
vendor firmware instead parks the VBIOS in the GPU's "NVS" memory and serves it
through an ACPI `_ROM` method; on this Dell the node is `\\_SB.PCI0.RP01.PEGP` and
the method lives in SSDT10 (`VBIOS` in four 256 KiB `RBFx` chunks).  The Windows
mobile driver fetches its VBIOS through that method.

When the GPU is passed through to a guest, that ACPI node does not exist, so the
driver cannot obtain a VBIOS and fails to start the device with
`CM_PROB_FAILED_POST_START` (Problem Code 43).  A `romfile=` supplies the PCI
ROM BAR instead, which the mobile driver appears not to use.

What this does
--------------
Emits an SSDT that defines a device under `\\_SB.PCI0` whose `_ADR` matches the
passed-through GPU, carrying a `_ROM` method that returns the recovered VBIOS.
Hand the compiled table to QEMU with `-acpitable file=<out>.aml`.

Usage
-----
    script/nvrom-ssdt.py --rom vbios_pad.rom --adr 0x00040000 --out nvrom.aml

`--adr` is the guest's PCI address of the GPU: `(device << 16) | function`, so
`00:04.0` is `0x00040000`.  `iasl` (Debian/Ubuntu `acpica-tools`) must be on
PATH, or passed with `--iasl`.
"""
from __future__ import annotations

import argparse
import shutil
import subprocess
import sys
import tempfile
from pathlib import Path

TEMPLATE = """\
DefinitionBlock ("", "SSDT", 2, "VFIOPC", "NVROM", 0x00000001)
{{
    External (_SB_.PCI0, DeviceObj)

    Scope (\\_SB.PCI0)
    {{
        Device (NVD0)
        {{
            Name (_ADR, {adr:#010x})
            Name (VROM, Buffer (0x{size:04X})
            {{
{body}
            }})
            Method (_ROM, 2, Serialized)
            {{
                // The driver calls _ROM(offset, length) in bytes and expects
                // that slice of the image back.
                Return (Mid (VROM, Arg0, Arg1))
            }}
        }}
    }}
}}
"""


def render_asl(rom: bytes, adr: int) -> str:
    rows = []
    for i in range(0, len(rom), 16):
        chunk = rom[i:i + 16]
        rows.append("                " + ", ".join(f"0x{b:02X}" for b in chunk) + ",")
    body = "\n".join(rows)
    return TEMPLATE.format(adr=adr, size=len(rom), body=body)


def find_iasl(explicit: str | None) -> str:
    if explicit:
        return explicit
    found = shutil.which("iasl")
    if not found:
        sys.exit("error: iasl not found; install acpica-tools or pass --iasl PATH")
    return found


def main() -> int:
    ap = argparse.ArgumentParser(description=__doc__,
                                 formatter_class=argparse.RawDescriptionHelpFormatter)
    ap.add_argument("--rom", required=True, type=Path, help="recovered VBIOS image")
    ap.add_argument("--adr", default="0x00040000",
                    help="guest PCI address as (dev<<16)|func (default: 0x00040000)")
    ap.add_argument("--out", type=Path, help="output .aml (default: alongside the ROM)")
    ap.add_argument("--iasl", help="path to the iasl compiler")
    ap.add_argument("--keep-asl", action="store_true", help="keep the generated .dsl")
    args = ap.parse_args()

    rom = args.rom.read_bytes()
    if len(rom) < 2 or rom[:2] != b"\x55\xaa":
        sys.exit(f"error: {args.rom} does not start with an option ROM signature (55 AA)")

    adr = int(args.adr, 0)
    if not 0 <= adr <= 0xFFFFFFFF:
        sys.exit("error: --adr must fit in 32 bits")

    asl = render_asl(rom, adr)
    out = args.out or args.rom.with_suffix(".aml")
    iasl = find_iasl(args.iasl)

    with tempfile.TemporaryDirectory() as td:
        dsl = Path(td) / "nvrom.dsl"
        dsl.write_text(asl)
        proc = subprocess.run([iasl, "-p", str(out.with_suffix("")), str(dsl)],
                              capture_output=True, text=True)
        if proc.returncode != 0 or not out.exists():
            sys.stderr.write(proc.stdout + proc.stderr)
            return proc.returncode or 1
        if args.keep_asl:
            out.with_suffix(".dsl").write_text(asl)

    print(f"{out}: {len(rom)} bytes of VBIOS, _ADR={adr:#010x}")
    return 0


if __name__ == "__main__":
    sys.exit(main())
