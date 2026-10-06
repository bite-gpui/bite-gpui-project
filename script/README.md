# `script/` — the tooling

The operator tooling for the Windows probe VM and the repository's own workflow scripts.

| script | what it is |
| --- | --- |
| [`interop-vm`](interop-vm) | the QEMU launcher and host preparation for the Windows probe guest |
| [`d3dprobe/`](d3dprobe) | a dependency-free Windows probe: each DXGI adapter's D3D11/12 feature level and LUID |
| [`windows-iso`](windows-iso) | build a client Windows ISO from UUP dump |
| [`autounattend.xml`](autounattend.xml), [`autounattend-uefi.xml`](autounattend-uefi.xml) | answer files for the unattended install (BIOS/MBR and UEFI/GPT) |
| [`vbios-from-firmware.py`](vbios-from-firmware.py) | carve a GPU option ROM out of a host SPI-flash dump |
| [`nvrom-ssdt.py`](nvrom-ssdt.py) | build an SSDT exposing a recovered VBIOS to a guest via ACPI `_ROM` |
| [`dev-env`](dev-env) | the recommended host build environment (toolchain paths, memory/swap) |
| [`check-citations`](check-citations), [`citations.py`](citations.py) | resolve every `path:line` citation against the canonical ref |

The last four are mostly historical or repository-wide; the first two are what the probe work uses.

---

## The Windows probe VM — `interop-vm`

`interop-vm` boots a Windows 11 guest under QEMU, with the guest's GPU coming from a **GVT-g
mediated Intel iGPU** (`--gvt`, see below), an SSH port-forward into the guest, and a QEMU monitor
socket the script drives for screenshots and keys.

### Subcommands

```
script/interop-vm check                    # report the prerequisites, read-only (the entry point)
sudo script/interop-vm prepare             # keep the dGPU for VFIO at boot (dGPU path only)
sudo script/interop-vm prepare --revert    # undo that, giving the GPU back to the host
sudo script/interop-vm bind | unbind       # one-off live bind/unbind (the live bind wedges; see below)
script/interop-vm create                   # create the qcow2 disk
script/interop-vm install --iso ISO        # boot the installer
script/interop-vm run                      # boot the installed disk
script/interop-vm powerdown                # ACPI power button
sudo script/interop-vm poweron             # run the dGPU's ACPI _ON on the host (dGPU path only)
script/interop-vm screenshot [--out F]     # save the running guest's screen to a PNG
script/interop-vm sendkey --keys KEYS      # inject keys (e.g. --keys 'shift+f10')
script/interop-vm type --text STR          # type a string
script/interop-vm bake                     # freeze the disk as a golden image; run on an overlay
script/interop-vm clone --to DIR           # a fresh VM: a new overlay on the golden image
```

### Options

`--dir DIR` (default `$HOME/interop-vm`), `--disk SIZE`, `--ram SIZE` (**use `3G`** on this 7.2 GiB
host; 4G swaps), `--cpus N`, `--cpu MODEL`, `--uefi`, `--vnc`, `--no-gpu`, `--gvt UUID`,
`--gpu-subsys VVVV:DDDD`, `--rom PATH`, `--no-rom`, `--x-vga`, `--no-vga`, `--iso PATH`,
`--unattend`, `--answer PATH`, `--qemu-arg ARG` (repeatable; raw QEMU arguments), `--out`, `--keys`,
`--text`, `--to DIR` (`clone` destination), `--cache MODE` (QEMU disk cache; e.g. `unsafe`), `--dry-run`.

The guest is built by `install --unattend` (it wipes disk 0, installs Windows 11 Pro, skips OOBE and
auto-logs-in a local admin `probe`). Once installed, `run` boots it.

### The guest, and talking to it

- **Windows 11 Pro 26100.1**, auto-logon `probe` / `probe`.
- **SSH** is forwarded to `127.0.0.1:2222`; the **QEMU monitor** listens on `127.0.0.1:4444`.
- The SSH token is **elevated** (High integrity, in the Administrators group), so registry reads,
  `pnputil`, etc. work without `runas`.

```sh
SSH='sshpass -p probe ssh -p 2222 -o StrictHostKeyChecking=no -o UserKnownHostsFile=/dev/null probe@127.0.0.1'
$SSH 'powershell -NoProfile -Command "Get-PnpDevice -Class Display | ft FriendlyName,Status"'
```

Three traps, each of which cost time to find:

1. **`scp` needs `sshpass` too.** It reads the password from the tty and will hang on a bare
   `sshpass -p probe ssh …` habit — wrap the `scp` in `sshpass` as well.
2. **Quote PowerShell by shipping the script, not by nesting.** Deeply nested `-Command "… $_.…"`
   through `ssh` → `cmd` gets mangled. Write the `.ps1`, `scp` it in, run it with
   `powershell -File`. (`Out-File` defaults to UTF-16 — pass `-Encoding utf8`, or strip NULs when
   reading it back.)
3. **An SSH session is not an interactive desktop.** It runs in window station `Service-0x0-…`, not
   `WinSta0`. D3D11/D3D12 device creation works there (verified), but **GUI apps hang** — `dxdiag`
   never returns. To run something in the console session, use a scheduled task with
   `/ru probe /rp probe /it` (and confirm with `WinSta0` from `GetProcessWindowStation`).

**If SSH times out or resets, suspect the host, not the guest.** QEMU's port-forward accepts the
connection — so `nc -z 127.0.0.1 2222` still says "open" — but the guest's vCPUs are starved while
the host is out of memory, so sshd never completes the banner exchange. On this 7.2 GiB box QEMU
(≈2.7 GB) plus an editor (≈1.5 GB) is enough; the guest also sits on the "Welcome" logon screen
until it gets scheduled. `free -h` and `uptime` are the tell — free memory and retry. It is not the
VM, and there is nothing to fix in it.

For anything **long or privileged**, a scheduled task also solves a second problem: closing the SSH
channel kills the remote process. Windows Update and `pnputil /add-driver` (the Intel driver below)
both need this:

```sh
# a .cmd wrapper, run as SYSTEM, logging to a file
schtasks /create /tn job /tr C:\Users\probe\job.cmd /sc once /st 00:00 /ru SYSTEM /rl HIGHEST /f
schtasks /run /tn job
```

The QEMU monitor takes raw commands, so scripting it is just `printf`:

```sh
printf 'system_powerdown\n' | nc -q1 127.0.0.1 4444     # same as: script/interop-vm powerdown
```

### Security posture

The guest is reached by **password, not a key**: a local administrator `probe` whose password is
literally `probe` (`script/autounattend.xml`), with password-authenticated OpenSSH — the answer file
installs `OpenSSH.Server`, sets `sshd` to auto-start, and opens the guest firewall for port 22.

What keeps that acceptable is the **network shape**, not the credential:

- QEMU uses a **user-mode NIC**, so the guest is not on the real network at all. The only way in is
  the host's forward — `hostfwd=tcp:127.0.0.1:2222-:22` — bound to **loopback**.
- The monitor (`127.0.0.1:4444`) and VNC (`127.0.0.1:0`) are loopback too.

The password is public in this repository **on purpose** (it is a local, throwaway probe VM), which
is exactly the property to keep an eye on. Two changes would make it unacceptable and should be made
*first*, not after: forwarding to a non-loopback address (or switching to a bridged/tap NIC), and
putting the golden image somewhere anyone else can reach. If either is ever needed, move to key auth
— `ssh-keygen`, then drop the public key in the guest's `administrators_authorized_keys` (the Windows
OpenSSH convention for an admin user) — and stop passing the password through `sshpass`.

### GVT-g — the adapter that works

The guest's real GPU is a **mediated Intel iGPU**, not a passed-through card. One-time host setup:

1. `i915.enable_gvt=1` on the kernel cmdline, reboot, then `sudo modprobe kvmgt` after each boot.
   i915 logs a *non-fatal* `Direct firmware load for i915/gvt/vid_0x8086_did_0x1916_rid_0x07.golden_hw_state
   failed`; the mdev types appear anyway.
2. Create the mediated device — it does **not** survive a host reboot:

   ```sh
   sudo tee /sys/bus/pci/devices/0000:00:02.0/mdev_supported_types/i915-GVTg_V5_4/create <<< "$(uuidgen)"
   ls /sys/bus/mdev/devices/      # -> the new <uuid>
   ```

3. Boot with it: `run --no-gpu --gvt <uuid>` (`--gvt` attaches
   `-device vfio-pci,sysfsdev=/sys/bus/pci/devices/0000:00:02.0/<uuid>`; `--no-gpu` drops the dead
   dGPU). The guest sees an Intel `8086:1916` VGA controller.
4. **Install the Intel display driver once, in the guest.** Windows Update offers
   `Intel Corporation - Display - 31.0.101.2111`, but the WU COM downloader returns `E_ACCESSDENIED`
   over SSH and `0x80240016` from a SYSTEM task. The package still lands in
   `C:\Windows\SoftwareDistribution\Download\Install` (1.35 GB; it ships `ig9icd64.dll` /
   `igd9dxva64.dll`, i.e. Gen9), so apply it directly — this takes several minutes, hence the
   scheduled task:

   ```cmd
   pnputil /add-driver "C:\Windows\SoftwareDistribution\Download\Install\iigd_dch.inf" /install
   ```

   The adapter then reports `Intel(R) HD Graphics 520`, driver `31.0.101.2111`, problem code 0.

### Host preparation, and the dGPU

`sudo script/interop-vm prepare` makes the host hand `0000:01:00.0` to `vfio-pci` at *boot* (a live
`bind` wedges: the GNOME shell holds `/dev/nvidia0`, so nvidia refuses to detach it). It writes a
modules-load entry, a udev rule, a memlock limit, and masks nvidia's services and udev rules.
`prepare --revert` undoes all of it — run it, then reboot, to give the GPU back to the host driver:

```sh
sudo script/interop-vm prepare --revert
sudo reboot
script/interop-vm check      # driver: nvidia
```

**None of this is needed for GVT-g.** It exists only for the failed attempt to pass the discrete GPU
through; see [`../issues/0009-windows-probe-vm.md`](../issues/0009-windows-probe-vm.md) for why that
MUX-less GeForce cannot be passed to a guest. `sudo script/interop-vm poweron` (ACPI `_ON` via
`acpi-call-dkms`, for the muxless dGPU's power state) is likewise dGPU-only.

### A golden image, and clones

Booting a probe VM should be a copy, not a Windows install:

```sh
script/interop-vm powerdown          # the VM must be stopped
script/interop-vm bake               # freeze the disk, run the VM on a read-only overlay
script/interop-vm clone --to ~/interop-vm-2
```

`bake` renames the installed disk to `windows-golden.qcow2` (and makes it read-only), then replaces
`$VM_DIR/windows.qcow2` with a qcow2 **overlay** backed by it — so the VM carries on unchanged, and
`clone --to DIR` creates another overlay for a fresh VM. The golden is a dependency of every overlay:
keep it, or they break. Run one guest at a time (the SSH and monitor ports are fixed at 2222/4444,
and the GVT-g mdev allows a single instance).

### Booting is slow, and what does not help

A boot takes ~4 minutes. Three levers were measured on 2026-10-06 and **none** helps, because the
cost is the host — a 44 GB image on a 5400 rpm HDD with 7.2 GiB RAM — not the guest's init phase:

| run | time |
| --- | --- |
| cold boot | ~4 m 11 s |
| cold boot with `--cache unsafe` | ~4 m 53 s |
| hibernate (`shutdown /h`) | 47 s to power off, resume ~4 m 47 s |
| hybrid shutdown (Fast Startup) | hung > 8 min |

So `cache=unsafe` is just a knob (it skips *write* flushes; a boot is read-bound), hibernation buys
nothing, and Fast Startup's hybrid shutdown *hangs* with the vGPU — keep it off. The only "instant" is
to **keep the guest running** and drive it over SSH, rebooting only when a test needs a fresh device
(P9 does).

---

## Cross-compiling a Windows probe — `script/d3dprobe`

`d3dprobe` is a small Rust program that enumerates the DXGI adapters and prints, per adapter, its
name, ids, **LUID**, dedicated VRAM, and the highest **D3D11** and **D3D12** feature level it
supports. It is dependency-free (raw FFI), so it cross-compiles with nothing but the mingw CRT.

One-time setup — the Rust toolchain is the project's, at `projects/.rustup-home` and
`projects/.cargo-home` (`script/dev-env env` prints the environment):

```sh
export RUSTUP_HOME=…/projects/.rustup-home CARGO_HOME=…/projects/.cargo-home
export PATH="$CARGO_HOME/bin:$PATH"

rustup target add x86_64-pc-windows-gnu          # the msvc target needs MSVC's linker; gnu does not
sudo apt-get install -y gcc-mingw-w64-x86-64     # the gnu target's linker
```

Build, copy in, run:

```sh
cd script/d3dprobe
cargo build --release --target x86_64-pc-windows-gnu
scp -P 2222 -o StrictHostKeyChecking=no -o UserKnownHostsFile=/dev/null \
    ../target/x86_64-pc-windows-gnu/release/d3dprobe.exe probe@127.0.0.1:C:/Users/probe/d3dprobe.exe
sshpass -p probe ssh -p 2222 -o StrictHostKeyChecking=no -o UserKnownHostsFile=/dev/null \
    probe@127.0.0.1 'C:\Users\probe\d3dprobe.exe'
```

Expected output on the GVT-g guest:

```
adapter 0: Intel(R) HD Graphics 520  [8086:1916]  luid 00000000:00005430  128 MiB dedicated (1535 shared)
    D3D11 : OK, feature level 11_1 (0xB100)
    D3D12 : OK, feature level 12_1 (0xC100)
```

Two things to carry into any hand-written Windows FFI (the `windows` crate gets both right):

- **`D3D11CreateDevice` has a tenth parameter.** `UINT SDKVersion` (`D3D11_SDK_VERSION == 7`) sits
  between `FeatureLevels` and `ppDevice`. Omitting it shifts every later argument; the Intel UMD
  then faults on the garbage pointers, which reads exactly like a driver failure. This is what made
  the earlier C#/Rust/C attempts "crash" for hours.
- **`DXGI_ADAPTER_DESC1` puts `Description[128]` first** (`WCHAR[128]`, 256 bytes), *then* `VendorId`
  … `Flags` — not the reverse.
