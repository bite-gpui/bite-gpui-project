- **Opened:** 2026-10-02
- **Status:** closed — the MUX-less GeForce cannot be passed through (every route tried, recorded
  below), but the guest runs on a **mediated Intel iGPU (GVT-g)**: a real **D3D11 FL 11_1 / D3D12
  FL 12_1** device at problem code 0. The environment is usable; running the probes, and
  productionising it into a golden image, are separate next steps.
- **Touches:** this machine (`user-Vostro-14-5459`), [`../script/interop-vm`](../script/interop-vm),
  [`../script/d3dprobe`](../script/d3dprobe), [`../script/windows-iso`](../script/windows-iso),
  [`../script/vbios-from-firmware.py`](../script/vbios-from-firmware.py),
  [`../script/nvrom-ssdt.py`](../script/nvrom-ssdt.py), [`../script/README.md`](../script/README.md),
  [`../spi/rendering/probe-windows-packed.md`](../spi/rendering/probe-windows-packed.md), the
  `gpui_interop` crate in `bite-gpui`

# The Windows guest that runs the interop probes

## The problem

W5's probes — **P5** (the fence loop), **P6** (adapter LUID matching) and **P9** (device loss) — and
the PR-2 sufficiency test need a **real** Direct3D device. The fork's `windows-latest` CI has none: it
runs WARP, which `probe-p6-adapter-luid.md` §5 says is *"recorded but … not representative of
hardware"*, and P6 in particular wants a **two-GPU** machine. So the Windows half of W5 has nowhere
to run, and this box is Linux. A QEMU guest with the discrete GPU passed through is the answer.

## The environment, as it stands (2026-10-06)

The guest is **Windows 11 Pro 26100.1**, auto-logon `probe`/`probe`, reachable over SSH on
`127.0.0.1:2222` (QEMU monitor on `127.0.0.1:4444`). Its real adapter is a **GVT-g mediated Intel HD
Graphics 520** (`8086:1916`, driver `31.0.101.2111`, problem code 0), which
[`../script/d3dprobe`](../script/d3dprobe) confirms is **D3D11 FL 11_1 / D3D12 FL 12_1**; QEMU's
`-vga std` supplies a second, basic adapter. The MUX-less GeForce is **not** passed through -- it
cannot be (the routes that failed are below) -- so it is left out of the guest entirely, and the
host keeps its nvidia driver.

One-time host setup: `i915.enable_gvt=1` on the kernel cmdline. After every host reboot the mediated
device is gone, so the bring-up is two commands:

    sudo script/interop-vm gvt     # load kvmgt, create the mdev, print the run line
    script/interop-vm run --no-gpu --gvt last --vnc   # `last` reads the uuid from gvt.uuid

The guest needs its Intel display driver installed once (see "GVT-g works" below).
[`../script/README.md`](../script/README.md) is the operating manual: the guest's SSH access, GVT-g,
the host prep and the `d3dprobe` cross-compile all live there.

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

So the last untried lever on this path was **power**: run the board's `_ON` *on the host* before
booting the guest. It was tried, and it does not help either:

### The host `_ON` power test also fails (2026-10-06)

With `acpi-call-dkms` installed, `script/interop-vm poweron` runs the GPU's own
`\_SB_.PCI0.RP01.PEGP._ON` (the path comes from the GPU's `firmware_node/path`). The method runs to
completion -- it returns `0xb7`, the `SSMP = 0xB7` it ends with -- and moves the device `D3cold ->
D3hot`, but the guest is unchanged: Code 43, `nvlddmkm` Stopped, no `VideoBiosVersion`, and
`nvidia-smi` still cannot talk to the driver.

That exhausts every lever on this adapter: hypervisor masking, subsystem-ID overrides, `romfile=`
(padded and unpadded), an injected ACPI `_ROM`, a host-side `_ON`, and forcing primary VGA. A muxless
mobile GeForce is not passable here.

### GVT-g works: the guest has a real hardware adapter (2026-10-06)

`modinfo i915` carried `enable_gvt`, so GVT-g host support was compiled in. The steps that worked:

1. `i915.enable_gvt=1` on the kernel cmdline, reboot, then `sudo modprobe kvmgt`. i915 logs a
   non-fatal `Direct firmware load for i915/gvt/vid_0x8086_did_0x1916_rid_0x07.golden_hw_state
   failed`; the mdev types appear anyway.
2. Create a mediated device:
   `sudo tee /sys/bus/pci/devices/0000:00:02.0/mdev_supported_types/i915-GVTg_V5_4/create`
   (128 MB low / 512 MB high GM; `device_api = vfio-pci`).
3. Boot with `--no-gpu --gvt <uuid>`. The 930M is dead weight, so it is dropped; the guest gets
   `-vga std` plus the vGPU (`8086:1916`).
4. Install the display driver **in the guest**. Windows Update offers `Intel Corporation - Display -
   31.0.101.2111` for the vGPU, but the WU COM downloader returns `E_ACCESSDENIED` over SSH and the
   SYSTEM-scheduled install returns `0x80240016`. The package still lands in
   `C:\Windows\SoftwareDistribution\Download\Install` (1.35 GB; it ships `ig9icd64.dll` and
   `igd9dxva64.dll`, i.e. Gen9), so install it directly -- this takes several minutes:

       pnputil /add-driver C:\Windows\SoftwareDistribution\Download\Install\iigd_dch.inf /install

   (pnputil cannot be driven over a plain SSH channel -- the channel closes and kills it; run it from
   a scheduled task.)

The guest then reports `Intel(R) HD Graphics 520`, driver `31.0.101.2111`, problem code 0, next to the
`-vga std` adapter -- two adapters, and a **real** D3D11/12 device. This is the adapter the probes
should use. The driver install is heavy, so it is a manual step for now.

An obvious last idea for the GeForce is ruled out too: passing the 930M *alongside* the Intel vGPU
(`run --rom … --gvt …`, three adapters) leaves the Intel device at problem code 0 but the NVIDIA
device still at **Code 43**. So the mobile driver does not need a real iGPU next to it -- the muxless
GeForce is simply not usable in a guest, and GVT-g is the adapter to build on.

### Verified: the vGPU is a real D3D11/12 device (2026-10-06)

[`../script/d3dprobe`](../script/d3dprobe) is a dependency-free Rust probe that enumerates the DXGI
adapters and asks each one for its Direct3D feature levels. It is cross-compiled on this host with the
project's rustup:

    rustup target add x86_64-pc-windows-gnu          # once
    sudo apt-get install -y gcc-mingw-w64-x86-64     # the gnu target's linker
    cargo build --release --target x86_64-pc-windows-gnu

Run in the guest over SSH, it reports:

    adapter 0: Intel(R) HD Graphics 520  [8086:1916]  luid 00000000:00005430  128 MiB dedicated
        D3D11 : OK, feature level 11_1 (0xB100)
        D3D12 : OK, feature level 12_1 (0xC100)
    adapter 1/2: Microsoft Basic Render Driver (WARP software) -- 11_1 / 12_1

So the GVT-g vGPU is a **real D3D11 FL 11_1 / D3D12 FL 12_1 device**, and DXGI hands over its
**LUID** -- the datum P6 needs. The `luid` is assigned **per boot**, so the value above is an
example: it changes between runs and is an identity to match *within* a run, not a constant.

**Gotcha worth keeping:** `D3D11CreateDevice` takes a *10th* parameter, `UINT SDKVersion`
(`D3D11_SDK_VERSION == 7`), between `FeatureLevels` and `ppDevice`. Leaving it out shifts every later
argument; the Intel UMD then faults on the garbage pointers, which reads exactly like a driver
failure (it cost hours of false leads across C#, Rust and C before `gcc`'s argument-count warning on
the C version exposed it). The `windows` crate gets this right; hand-rolled FFI must not forget it.

### The memlock limit GVT-g needs -- and the BSOD when it is missing (2026-10-06)

GVT-g's vGPU pins the guest's pages for its shadow GTT (`vfio_pin_pages`), so QEMU needs a raised
`RLIMIT_MEMLOCK`. The host default is 8 MiB, and with it the vGPU dies. The kernel log says so
plainly:

    vfio_pin_page_external: Task qemu-system-x86 (PID) RLIMIT_MEMLOCK (8388608) exceeded
    gvt: vgpu 1: vfio_pin_pages failed for iova 0x…, ret -12
    gvt: vgpu 1: fail to emulate MMIO write 00002230 len 4

and QEMU mirrors it, once per failed write:

    qemu-system-x86_64: vfio_region_write(…:region0+0x2230, …) failed: Bad address

The guest's Intel driver gets a dead vGPU and Windows then fails to boot (`Recovery: Windows didn't
load correctly`), so the visible symptom is a BSOD boot-loop, not a QEMU error.

`sudo script/interop-vm prepare` had installed `user - memlock unlimited` as part of its
*passthrough* wiring; `prepare --revert` removed it, and **that** is what broke the otherwise
working GVT-g guest. So GVT-g does share two things with the dGPU path after all: the `/dev/vfio`
udev rule, and this memlock limit. `gvt` now asserts both, `prepare --revert` keeps the memlock
limit, and `run` refuses to start with an 8 MiB limit. It is a **login** limit: writing the conf
takes effect at the next login (or reboot), not in the session that wrote it.

### The packed probe passes on the vGPU -- P5 and P6 (2026-10-06)

[`../script/interop-probe`](../script/interop-probe) is the packed W5 checklist: P6 asks whether DXGI
and wgpu agree on one adapter LUID, P5 runs a D3D12 -> D3D11 shared-texture fence loop. Run against
the GVT-g adapter over SSH, both pass on **hardware**:

    == P6 DXGI ==
    adapter 0: Intel(R) HD Graphics 520  [8086:1916]  luid 00000000:0000533C  128 MiB dedicated
    D3D11 device: adapter luid 00000000:0000533C  feature level 11_1
    P6 DXGI+LUID : PASS -- 3 DXGI adapter(s); D3D11 device on luid ...533C via IDXGIAdapter::GetDesc
    == P6 wgpu ==
    wgpu adapter 0: Intel(R) HD Graphics 520  backend=Dx12 type=IntegratedGpu  luid 00000000:0000533C
    P6 wgpu      : PASS -- LUID match between the D3D11 device and wgpu adapter 0
    == P5 fence loop ==
    D3D12 clear -> shared 64x64 R8G8B8A8 texture -> ID3D11Fence handle -> context4.Wait(fence, 1)
    P5 fence loop: PASS -- shared texture read back byte-exact on D3D11 (0/4096 mismatched)

So P6's answer -- **DXGI and wgpu name the same adapter by the same LUID** -- is recorded on a *real*
adapter, which is the datum that freezes `gpui-interop`'s `attach`/`Adapter` API
([`interop-crate.md`](../spi/rendering/interop-crate.md) §4). P5's loop is byte-exact through the
GPU-side `ID3D11Fence::Wait`. **P9 (device loss) is still outstanding** -- it needs a deliberate
reset/teardown, not just a boot. (The LUIDs are per boot, so the exact numbers are illustrative.)

## Operating it

The day-to-day commands -- SSH in, screenshot, run something privileged, rebuild the probe -- are
in [`../script/README.md`](../script/README.md). Two host-side facts are worth repeating here:

- The guest's **Intel display driver install is a manual, one-time step** for this VM; a golden
  image would fold it in.
- `sudo script/interop-vm prepare` / `prepare --revert` manage the *dGPU's* binding, which the
  abandoned GeForce no longer needs -- so with the GeForce dropped the host keeps its nvidia driver.
  GVT-g needs none of the *modprobe* or *udev-masking* wiring either, **but it does need two of the
  same bits**: the `/dev/vfio` udev rule, and the raised **memlock** limit (see "The memlock limit
  GVT-g needs" above). `gvt` installs both, and `prepare --revert` no longer removes the memlock
  limit.

**RAM.** Give the guest `--ram 3G`, not 4G: on this 7.2 GiB host the 4 GB guest swaps hard and
boots take minutes.

## Alternatives weighed

- **`dockur/windows`** — mature (53.5k stars), auto-installs a client ISO, `ARGUMENTS` passes extra
  QEMU flags (so the vfio device). Needs Docker, which is not installed.
- **`sh13y/windows-server-qemu-installer`** — Server-oriented, no passthrough, and it `apt upgrade`s
  the host. Rejected.
- **UUP dump directly** — what `script/windows-iso` drives; needs the tooling above.
- **A cloud Windows GPU VM** — no local root/RAM strain, but cost.

## What is next: productionising

The environment works; it is not yet reusable. The gaps, in order:

- **A golden image -- done.** `script/interop-vm bake` freezes the driver-complete disk as a read-only
  `windows-golden.qcow2` and runs the VM on a qcow2 overlay on it; `clone --to DIR` makes another
  overlay for a fresh VM. What remains is only to re-bake when the guest changes.
- **One command to bring the harness up -- done.** `sudo script/interop-vm gvt` loads `kvmgt`,
  creates the mdev, re-asserts the `/dev/vfio` rule and the memlock limit, and prints the `run` line
  (which uses `--gvt last`, so it survives a reboot); `run` refuses to start a wrongly-configured
  guest instead of booting a silently different one. What persists across a reboot, and what does
  not, is tabulated in [`../script/README.md`](../script/README.md).
- **Run the probe -- P5 and P6 done, P9 outstanding.** [`../script/interop-probe`](../script/interop-probe)
  was run against the adapter and **both P6 gates and P5 pass** ("The packed probe passes" above), so
  **P6's answer is recorded -- the datum that freezes `gpui-interop`'s API**
  ([`interop-crate.md`](../spi/rendering/interop-crate.md) §4). What remains is **P9** (device loss),
  which needs a deliberate reset (see [`probe-windows-packed.md`](../spi/rendering/probe-windows-packed.md)).

### Boot performance -- measured, and the levers that did not help (2026-10-06)

A boot takes ~4 minutes, and a fresh probe cycle is dominated by it. Three levers were tried:

| run | time |
| --- | --- |
| cold boot | ~4 m 11 s |
| cold boot with `--cache unsafe` | ~4 m 53 s |
| hibernate (`shutdown /h`) | 47 s to power off, resume ~4 m 47 s |
| hybrid shutdown (Fast Startup) | hung > 8 min |

None helps, because the cost is the **host** -- a 44 GB image on a 5400 rpm HDD with 7.2 GiB RAM and
`load ~6` -- not the guest's init phase:

- `cache=unsafe` only skips *write* flushes, and a boot is read-bound (kept as a `--cache` knob).
- Hibernation resumes by reading `hiberfil` from the same slow disk, so it costs a cold boot's worth
  of I/O; the init it saves is a small slice.
- Fast Startup's **hybrid shutdown hangs** with the vGPU attached -- a regression, so Fast Startup
  stays off and the golden is baked with hibernation off.

So the "instant" cycle is to **keep the guest running** and drive it over SSH, rebooting only when a
test needs a fresh device (P9 does).
