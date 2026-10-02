- **Opened:** 2026-10-02
- **Status:** open — the host is set up and the Windows installer is running under VNC; the guest install and the probes remain
- **Touches:** this machine (`user-Vostro-14-5459`), [`../script/interop-vm`](../script/interop-vm), [`../script/windows-iso`](../script/windows-iso), [`../spi/rendering/probe-windows-packed.md`](../spi/rendering/probe-windows-packed.md), the `gpui_interop` crate in `bite-gpui`

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
