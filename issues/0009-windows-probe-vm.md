- **Opened:** 2026-10-02
- **Status:** open — boot-time passthrough works, and the guest's exact VBIOS was recovered and both
  passed via `romfile=` **and** injected as an ACPI `_ROM` method, but the NVIDIA driver still hits
  **Code 43** (`CM_PROB_FAILED_POST_START`) after a ~7 s POST; the host device sits in `D3hot`, so the
  muxless power path (`_ON`) is the last untried lever, and a mediated iGPU (GVT-g) is the fallback.
  The probes have not run
- **Touches:** this machine (`user-Vostro-14-5459`), [`../script/interop-vm`](../script/interop-vm), [`../script/vbios-from-firmware.py`](../script/vbios-from-firmware.py), [`../script/windows-iso`](../script/windows-iso), [`../spi/rendering/probe-windows-packed.md`](../spi/rendering/probe-windows-packed.md), the `gpui_interop` crate in `bite-gpui`

# The Windows guest that runs the interop probes

## The problem

W5's probes — **P5** (the fence loop), **P6** (adapter LUID matching) and **P9** (device loss) — and
the PR-2 sufficiency test need a **real** Direct3D device. The fork's `windows-latest` CI has none: it
runs WARP, which `probe-p6-adapter-luid.md` §5 says is *"recorded but … not representative of
hardware"*, and P6 in particular wants a **two-GPU** machine. So the Windows half of W5 has nowhere
to run, and this box is Linux. A QEMU guest with the discrete GPU passed through is the answer.

## What this machine already offers (measured 2026-10-02)

- `vmx` + `ept`; **VT-d present** (DMAR tables, `dmar0`/`dmar1`); **IOMMU in `Translated` mode** —
  the mode passthrough needs.
- The discrete GPU, `0000:01:00.0`, is a **MUX-less 3D controller** (`[0302] GeForce 930M GM108M`) —
  no display output — **alone in IOMMU group 10**, idle (0%, 4 MiB), and **not used by the host
  desktop** (which is on the Intel iGPU; `nvidia-drm` found no CRTCs). So binding it away does not
  disturb the session.
- `/dev/kvm` is writable by the user (ACL); `qemu-system-x86_64` 10.2.1 and `qemu-img` are installed;
  360 GB free disk.

## The plan, and what is built

1. **Build a client Windows ISO** — [`script/windows-iso`](../script/windows-iso) drives UUP dump
   (`--list 11`, then the id), and `--dry-run` resolves without downloading. **Client, not Server:**
   NVIDIA's GeForce drivers do not install on Windows Server, so a Server guest would fall back to the
   Basic Display Adapter — WARP again, the thing the guest exists to avoid.
2. **Bind the GPU to vfio-pci** — [`script/interop-vm`](../script/interop-vm) `bind`/`unbind` (root),
   `driver_override` + unbind + bind, with `--dry-run`.
3. **Create / install / run** — `create`, `install --iso …`, `run`. The guest gets `-vga std` *and*
   the passed-through 930M, so it has **two adapters** — closer to the hybrid shape P6 warns about
   than a single-adapter host. `-cpu host,kvm=off` hides the hypervisor (NVIDIA Code 43).
4. **Run the packed probe** — [`probe-windows-packed.md`](../spi/rendering/probe-windows-packed.md).

`check` reports the prerequisites read-only and is the entry point.

## The ISO (built 2026-10-02)

`script/windows-iso 94b8c5e5-d0ea-41c1-937c-c5ce99ad518b` (24H2, 26100.6508) produced
`windows-iso/uup/26100.1_PROFESSIONAL_X64_EN-US.ISO` — 4.1 GB, UDF + El Torito, carrying
`install.wim`, `boot.wim`, `setup.exe` and the EFI boot files. Two bugs in the script had to be
fixed to get there:

- it fetched the package with a `GET get.php?…&lang=…&pack=1`, but the site wants a **`POST`** to
  `get.php?id=…&pack=<lang>&edition=<edition>` with `autodl=2` — `pack` is the language, so `pack=1`
  read as one and the run died on a `Specified language is not supported` page;
- its prerequisites omitted `chntpw`, the converter's own registry dependency (`boot.wim`), so the
  build stopped mid-conversion. The apt line is `aria2 cabextract wimtools chntpw genisoimage
  xorriso`.

The ISO is build output and is ignored (`/windows-iso/`).

## Host-side passthrough — what it took (2026-10-02)

Binding the GPU **at boot, not live** turned out to be the only workable shape. Four separate
obstacles, all now handled by `script/interop-vm prepare` (`prepare --revert` undoes it):

1. **The nvidia services load the module explicitly.** A `/etc/modprobe.d` blacklist stops only
   udev's *alias* auto-loading; `nvidia-powerd.service` (`WantedBy=multi-user.target`) runs its own
   `modprobe nvidia`, which the blacklist cannot stop — the boot log shows it load 5 s after the
   service starts, and a live unbind then sits on
   `NVRM: Attempting to remove device … with non-zero usage count!`. `prepare` masks `nvidia-powerd`,
   `nvidia-persistenced` and the suspend/hibernate/resume units.
2. **A live unbind hangs.** GNOME Shell, the terminal and the editor each hold `/dev/nvidia0` open
   (EGL/Vulkan enumeration), so a live `bind` never returns. Instead `prepare` loads `vfio-pci` from
   `/etc/modules-load.d` at boot, where it claims the GPU before anything else can.
3. **`/dev/vfio/<group>` is root-only** (`crw------- root root`). `prepare` writes a udev rule
   (`SUBSYSTEM=="vfio", TAG+="uaccess"`) so the seated user gets an ACL — the same mechanism that
   already grants `/dev/kvm` (the user is *not* in the `kvm` group).
4. **`RLIMIT_MEMLOCK` is 8 MB, soft *and* hard.** VFIO pins the whole guest RAM, so
   `vfio_container_dma_map` returns `-12 (ENOMEM)` until the limit is raised; `prepare` writes
   `/etc/security/limits.d/99-vfio-memlock.conf` (`… - memlock unlimited`), applied at next login.

The GPU has **no readable VBIOS** (MUX-less), so QEMU warns `Cannot read device rom at
0000:01:00.0` and carries on — non-fatal for the install. If the guest driver wants it,
`install --rom <vbios>` passes a dump (`romfile=`) and `--no-rom` sets `rombar=0` to silence the
probe. VNC now binds `127.0.0.1:0` (localhost only).

### Unattended install

`script/interop-vm install --unattend` stages [`script/autounattend.xml`](../script/autounattend.xml)
as a tiny ISO and attaches it as a second CD-ROM, so Setup runs without prompts: it wipes disk 0,
installs **Windows 11 Pro**, bypasses the TPM/Secure Boot/RAM checks, skips OOBE and auto-logs-in a
local admin `probe`. `--answer PATH` takes a custom file. The layout is BIOS/MBR (the SeaBIOS
default, and what the running guest uses); `--uefi` would need a GPT variant of the file.

## Follow-up (2026-10-05): the nvidia module loop, and two script bugs

`prepare` as first written still let the host churn. Three fixes, all now in
[`../script/interop-vm`](../script/interop-vm):

1. **An explicit `modprobe` ignores a blacklist — and nvidia's udev rules make one.**
   `/usr/lib/udev/rules.d/71-nvidia.rules` runs `RUN+="/sbin/modprobe nvidia-uvm"` (and
   `nvidia-drm`, `nvidia-modeset`) on *every* `add` of `/bus/pci/drivers/nvidia`. With the GPU on
   `vfio-pci` that load fails, the driver unregisters, the `add` fires again — an endless
   udev/`modprobe` loop (`udevadm monitor` shows `add`/`remove /bus/pci/drivers/nvidia` every few
   seconds; `nvidia` never stays in `/proc/modules`). A `blacklist` only stops *alias* auto-loading,
   so `prepare` now writes `install nvidia /bin/true` (plus the DRM/modeset/uvm submodules) into
   `vfio-nvidia.conf` — making every explicit `modprobe` a no-op — and shadows each `*nvidia*.rules`
   with an empty file under `/etc/udev/rules.d`. This refines obstacle 1 below, which blamed only
   `nvidia-powerd`.
2. **A live `bind` wedges the GPU.** GNOME Shell holds `/dev/nvidia0`, so nvidia refuses to detach
   a device with a non-zero usage count, and the process never returns. It can leave the GPU
   *orphaned* (bound to no driver at all); a **reboot** is the reliable recovery, after which the
   boot-time `vfio-pci` claim lands it cleanly. This is why `prepare` binds at boot, and why `check`
   may report an empty `driver:` after a wedged unbind.
3. **`--qemu-arg` values were split on spaces.** `qemu_argv` printed its arguments and the call
   sites consumed them with an unquoted `$(…)`, so any value containing a space (e.g.
   `-smbios type=1,manufacturer=Dell Inc.,…`) was torn apart and QEMU tried to open the fragment as
   a disk. `qemu_argv` now populates a `QEMU_ARGV` array and the call sites pass `"${QEMU_ARGV[@]}"`.

State: Windows 11 Pro 26100.1 is installed (auto-logon `probe`, SSH on `127.0.0.1:2222`) and the
guest GPU is the passed-through 930M with driver **582.78 DCH** installed. The remaining blocker is
**Code 43** (`CM_PROB_FAILED_POST_START`): the PCI subsystem-ID override
(`x-pci-sub-vendor-id=0x1028,x-pci-sub-device-id=0x070b`) got the driver to bind (the card's standard
subsystem registers read `0000:0000`, so every subsystem-qualified INF entry missed), and
`-cpu host,kvm=off,-hypervisor` clears the hypervisor bit, but the device still fails POST. It has
**no readable VBIOS** (MUX-less; `EIO` on the ROM even unbound), which is the leading suspect.

## VBIOS recovery from the host firmware (2026-10-06)

The usual fix for Code 43 is handing the guest the GPU's option ROM (`romfile=`). Route B (a dump
from TechPowerUp) was a dead end: TPU has no entry for a muxless part like the 930M, and it
Cloudflare-blocks non-browser clients. So we recovered the **exact** ROM from this laptop's own SPI
flash. [`../script/vbios-from-firmware.py`](../script/vbios-from-firmware.py) automates the last
step; the obstacles, in order, were:

1. **Secure Boot / kernel lockdown.** `flashrom -p internal` failed with *Could not get I/O
   privileges* and `/dev/mem` with *Permission denied*: `mokutil --sb-state` = enabled, so lockdown
   was `integrity` (`dmesg`: *Lockdown: flashrom: raw io port access is restricted*), which revokes
   raw port I/O and `/dev/mem` even for root. Disabling Secure Boot in the Dell BIOS drops lockdown
   to `none`.
2. **The ME region is read-protected.** A full read dies at *cannot read inside Management Engine
   region (0x001000..0x3fffff)*; the descriptor is `fd 0-0xfff / me 0x1000-0x3fffff / bios
   0x400000-0xbfffff` (12 MB). Read only the BIOS region:
   `flashrom -p internal:laptop=force_I_want_a_brick --ifd -i bios -r bios.bin` (the `laptop=` switch
   is mandatory on laptops; `-r` is a pure read).
3. **The VBIOS is compressed, not a plain option ROM.** The 8 MB BIOS region has **no `NVIDIA`
   string, no `55AA`+`PCIR`, no `NPDE`** -- the ROM is LZMA-compressed inside a firmware volume that
   AMI parks in an *AMI ROM hole* at `0x5FAF10`, under file GUID
   `DB8F87A0-8921-4D09-BE7B-93D5F1ECB0A0`. `UEFIExtract bios.bin all` decompresses it.
4. **Non-standard PDS pointer.** `PCIR` is *not* at ROM offset 0x18 here: the 16-bit value at 0x18
   is a *pointer* to the PCI Data Structure, and this ROM points to `0x190`. Following the pointer
   is the difference between "no ROM found" and two valid ones.

The result is **two distinct 36352-byte option ROMs** for `10DE:1346` (`checksum 0`, PDS
image-length 71 units, `VIDEO` / `IBM VGA Compatible`, date `08/04/15`) -- the exact production VBIOS
for this board.

**It does not fix Code 43.** Passing either variant via `-device vfio-pci,...,romfile=...` (verified
in the live QEMU command line) leaves the device at `Problem Code 43 (CM_PROB_FAILED_POST_START)` and
`nvidia-smi` still cannot talk to the driver.

So the VBIOS was **necessary-but-not-sufficient**. The remaining cause is the second muxless hurdle:
the guest cannot bring the GPU up because the power/routing methods (`_ON` / `_OFF` / `_PS0` /
`_PS3` / `_DSM`, and a `_ROM` in SSDT10) live in the **host** DSDT/SSDT, which the guest never sees.
The ACPI tables were captured to `/home/user/interop-vm/acpi/` for that work. Next candidates, in
order: an older guest NVIDIA driver, then making the GPU the primary VGA (`-vga none` +
`x-vga=on`); failing those, real-GPU D3D on this muxless laptop is likely infeasible.

### The primary-VGA route is impossible here (2026-10-06)

Making the 930M the primary/boot adapter was the obvious next try, and it fails on two counts:

- **`x-vga=on` is refused outright.** QEMU aborts at startup: *vfio 0000:01:00.0: failed getting
  region info for VGA region index 8: Invalid argument / device does not support requested feature
  x-vga*. The 930M is a `[0302]` **3D controller with no VGA region**, so VFIO will never let it be
  a boot VGA -- the same muxless fact (no display outputs) showing up in config space.
- **Dropping the emulated VGA does not help either.** With `-vga none` (and no `x-vga`) the guest
  boots headless and the device enumerates *undriven* -- a generic "3D Video Controller",
  `CM_PROB_REINSTALL` (Code 18) -- instead of driver-bound-but-failed (Code 43). Forcing a driver
  update left it at Code 18.

Both knobs are kept in [`../script/interop-vm`](../script/interop-vm) as `--x-vga` / `--no-vga`,
with the caveat recorded.

### The ACPI `_ROM` route — injected, loaded, still Code 43 (2026-10-06)

The vendor SSDT (`/home/user/interop-vm/acpi/SSDT10`, *"Genuine NVIDIA Certified Optimus Ready
Motherboard"*) shows *how* this board serves the VBIOS: the node `\_SB.PCI0.RP01.PEGP` carries a
`_ROM` method that returns the image from the GPU's on-card NVS region in 0x8000-byte chunks
(`RBF1..RBF4`), alongside `_ON`/`_OFF`/`_PS0`/`_PS3` and the Optimus `_DSM`. A guest has none of it,
which is exactly the kind of thing a mobile driver reaches for when the ROM BAR is empty.

[`../script/nvrom-ssdt.py`](../script/nvrom-ssdt.py) builds an SSDT that defines a device under
`\_SB.PCI0` at the guest GPU's `_ADR` with a `_ROM` method returning the recovered VBIOS;
`run --qemu-arg -acpitable --qemu-arg file=…` injects it. On this guest the 930M is `00:04.0`, so
`_ADR = 0x00040000`.

**It does not help.** The table *is* loaded (Windows exposes an `SSDT\VFIOPC` key under
`HKLM\HARDWARE\ACPI\SSDT`) and the device still fails: `CM_PROB_FAILED_POST_START` (43), `nvlddmkm`
Stopped, and no `VideoBiosVersion`/`HardwareInformation` is ever written for the adapter.

Two more measurements from the same run:

- **The driver's start is slow, not an instant VM rejection.** `Microsoft-Windows-Kernel-PnP/Driver
  Watchdog` records the device event-queue thread running ~7 s before failing with *Event Argument:
  0x2B* (= 43) — the driver drives the hardware and times out.
- **The host device is `D3hot` while the guest runs** (`power_state`), with its ROM BAR `[virtual]`
  (the `romfile`) but never read by the mobile driver. On a muxless board the dGPU core is powered up
  by the firmware's ACPI `_ON` (`HGON`/the EC), which nothing on the host runs — no nvidia, no nouveau.

So the last untried lever on this path is **power**, not configuration: run the board's `_ON` *on the
host* before booting the guest (`acpi-call-dkms`, then `\_SB.PCI0.RP01.PEGP._ON`), which needs root and
a reboot. If that too fails, real-GPU D3D via this adapter is spent.

### The fallback: a mediated iGPU (GVT-g) is still available (2026-10-06)

`modinfo i915` on this host still carries `enable_gvt` (default false), so GVT-g host support is
compiled in, and the Skylake-U **HD Graphics 520** is a Gen9 part with real D3D11/12 (feature level
12_1). Booting with `i915.enable_gvt=1`, loading `kvmgt`, creating an mdev and passing it to the guest
(with Intel's GVT-g Windows driver) would give the probes a *real* hardware adapter — two of them,
with `-vga std` — without the muxless `_ON`/`_ROM`/Code 43 wall. The cost is an out-of-tree guest
driver. This is the recommended pivot if the host-side `_ON` test fails.

## What is blocked

- **root, once.** `sudo script/interop-vm prepare` and the login/reboot that applies the memlock
  limit need `sudo`, which is *not* passwordless here, so they are human steps.
- **the guest install.** Running now (the section above records the host setup); driving Windows
  Setup over VNC is still manual work.
- **RAM.** The 4 GB guest pins ~4 GB; free memory fell to ~1.6 GB during the install — close the
  editor.
- **building inside the guest is heavy.** Better to build the probe on `windows-latest` (where `fxc`
  works anyway) and copy the artifact in via the guest's shared folder.

## Alternatives weighed

- **`dockur/windows`** — mature (53.5k stars), auto-installs a client ISO, `ARGUMENTS` passes extra
  QEMU flags (so the vfio device). Needs Docker, which is not installed.
- **`sh13y/windows-server-qemu-installer`** — Server-oriented, no passthrough, and it `apt upgrade`s
  the host. Rejected.
- **UUP dump directly** — what `script/windows-iso` drives; needs the tooling above.
- **A cloud Windows GPU VM** — no local root/RAM strain, but cost.

## What would close it

A probe run inside the guest printing P5, P6 and P9 and the sufficiency result — after which **P6's
answer freezes `gpui-interop`'s API** ([`interop-crate.md`](../spi/rendering/interop-crate.md) §4).
