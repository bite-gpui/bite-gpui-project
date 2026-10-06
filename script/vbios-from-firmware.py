#!/usr/bin/env python3
"""Locate and carve a GPU option ROM out of a firmware image or a UEFIExtract dump.

Why this exists (see issues/0009): on a muxless Optimus laptop the discrete GPU's
VBIOS is not on the card (reading its ROM BAR gives EIO) and is *not* a plain
option ROM in the SPI image either -- it is stored **compressed** (LZMA) inside a
firmware volume that AMI parks in an "AMI ROM hole".  Recovering it is a
three-step, outside-in process:

    1. dump the BIOS region  (secure boot must be OFF; see issues/0009):
         sudo flashrom -p internal:laptop=force_I_want_a_brick --ifd -i bios -r bios.bin
    2. decompress the firmware volumes it contains:
         UEFIExtract bios.bin all        # -> bios.bin.dump/
    3. locate + carve the option ROM with this script:
         script/vbios-from-firmware.py <bios.bin | bios.bin.dump> [outdir]

Detection notes learned the hard way:
  * "PCIR" is *not* always at ROM offset 0x18 -- the 16-bit value at 0x18 is a
    *pointer* to the PCI Data Structure.  This GPU's ROM points to 0x190, which
    is why a naive "55AA then PCIR at +0x18" scan finds nothing.
  * a decompressed VBIOS can carry no readable strings; the reliable tell that a
    blob is NVIDIA is the magic bytes NPDE / NBSI / NBFU.

A well-formed carve has its full-image checksum == 0 and PDS image-length * 512
== the carved size.
"""
import hashlib
import os
import struct
import sys

NV_MAGICS = (b"NPDE", b"NBSI", b"NBFU")


def u16(d, o):
    return struct.unpack_from("<H", d, o)[0]


def find_option_roms(d):
    """Yield (offset, vendor, device, size) for every 55AA header whose PDS pointer
    lands on a PCIR structure."""
    n = len(d)
    i = 0
    while True:
        j = d.find(b"\x55\xaa", i)
        if j < 0:
            return
        i = j + 1
        if j + 0x1A > n:
            continue
        pds = u16(d, j + 0x18)  # pointer to the PCI Data Structure
        if pds == 0 or j + pds + 0x12 > n:
            continue
        if d[j + pds:j + pds + 4] != b"PCIR":
            continue
        vendor = u16(d, j + pds + 4)
        device = u16(d, j + pds + 6)
        size = u16(d, j + pds + 0x10) * 512  # image length, in 512-byte units
        yield j, vendor, device, size


def scan_blob(d, label, outdir):
    roms = list(find_option_roms(d))
    magics = [m.decode() for m in NV_MAGICS if m in d]
    if not roms and not magics:
        return
    print(f"{label}: size={len(d)}")
    if magics:
        print(f"    NVIDIA magics: {' '.join(magics)}")
    for j, vendor, device, size in roms:
        tag = "  <== NVIDIA" if vendor == 0x10DE else ""
        print(f"    ROM off=0x{j:x} vendor=0x{vendor:04x} device=0x{device:04x} size=0x{size:x}{tag}")
        if vendor == 0x10DE and size:
            end = min(len(d), j + size)
            digest = hashlib.md5(d[j:end]).hexdigest()[:8]
            out = os.path.join(outdir, f"vbios_{vendor:04x}_{device:04x}_{digest}.rom")
            with open(out, "wb") as fh:
                fh.write(d[j:end])
            print(f"      carved {out} ({end - j} bytes, checksum {sum(d[j:end]) & 0xff})")


def main():
    if len(sys.argv) < 2:
        print(__doc__)
        return 1
    target = sys.argv[1]
    outdir = sys.argv[2] if len(sys.argv) > 2 else "."

    if os.path.isdir(target):  # a UEFIExtract .dump tree
        for dirpath, _dirnames, filenames in os.walk(target):
            for name in filenames:
                path = os.path.join(dirpath, name)
                try:
                    scan_blob(open(path, "rb").read(), path, outdir)
                except OSError:
                    pass
    else:
        scan_blob(open(target, "rb").read(), target, outdir)
    return 0


if __name__ == "__main__":
    sys.exit(main())
