# 0009 — GPU preference on the Windows and macOS backends

- **Drafted:** 2026-10-18
- **Status:** draft — design only, not decided and not implemented. The Linux arm is in place
  ([`0008`](0008-gpu-preference-and-enumeration.md) steps 1–2); `gpui_windows` and `gpui_apple` still
  inherit the no-op/empty defaults of `set_gpu_preference`, `enumerate_gpus` and `active_gpu`.
- **Evidence:** [`0008`](0008-gpu-preference-and-enumeration.md) (the seam and the process-wide
  ruling), [`0010`](../issues/0010-application-gpu-preference.md) / [`0012`](../issues/0012-gpu-enumeration-and-negotiation.md)
  (the application API), [`windows-adapter-luid-probe.md`](windows-adapter-luid-probe.md) (the LUID is
  the Windows identity a producer matches), [`optimus-dmabuf-topologies.md`](optimus-dmabuf-topologies.md)
  (why `LowPower` is the Linux default), and the probes for the two platforms.
- **Touches:** `gpui_platform/src/gpu.rs`, `gpui_platform/src/platform.rs`; `gpui_windows/src/…`
  (`platform.rs`, `directx_devices.rs`, `directx_renderer.rs`, `direct_write.rs`); `gpui_macos/src/…`
  (`platform.rs`, `window.rs`); `gpui_apple/src/metal_renderer.rs`; `gpui_wgpu/src/wgpu_context.rs`
  (to share the ranking); `gpui_interop` (identity note only).

## 1. The seam already exists; the backends do not implement it

`Platform` already carries the three methods (`crates/gpui_platform/src/platform.rs:119`,
`:129`, `:138`) and `gpui_linux` implements all three. On Windows and macOS they are the trait
defaults: the preference is ignored, enumeration is empty and the active GPU is `None`. This design
fills them in without moving the seam.

The observable contracts stay those of 0008:

- the device is process-wide and chosen before the first window;
- `set_gpu_preference` is the only path the renderer sees;
- runtime switching is refused;
- a `Custom` selector names one adapter, and an invalid choice fails the window naming the shortfall.

## 2. Identity: a permanent part and a session handle

The identity is two facts, not one:

- **permanent** — what a config can store, and what should still name the same hardware after a
  restart;
- **session** — what the backend can actually match *now*: a LUID, a registry id, a DRM node.

This refines 0008's `GpuId` (a PCI pair) into that contract; the code still carries the old shape.

The earlier draft had one tagged `GpuId`. The documentation research below shows why one is not
enough: every per-process handle is per boot, and every stable handle is ambiguous between two
identical cards. So the persistent part is part of the contract, and the session part carries the
handle:

```rust
/// The identity of an adapter.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct GpuId {
    /// What survives a restart, where the platform has anything that does.
    pub permanent: PermanentGpuId,
    /// The backend's handle for this adapter in this process. A config cannot know it, so the
    /// field is skipped on serialisation and reads back as `None`.
    #[serde(skip)]
    pub session: Option<SessionGpuId>,
}

/// The stable part of an identity. Every adapter `enumerate_gpus` reports carries one.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub enum PermanentGpuId {
    /// PCI vendor/device, plus the subsystem/revision that distinguishes board variants. Stable
    /// across restarts; two identical cards share it. `None` is a wildcard — `ZED_DEVICE_ID` names
    /// a device only.
    Pci {
        vendor_id: Option<u32>,
        device_id: u32,
        subsys_id: Option<u32>,
        revision: Option<u32>,
    },
    /// The Windows PnP device instance id — the one documented persistent per-adapter identity.
    /// DXGI does not expose it; a later `SetupAPI` read fills it in.
    Pnp(String),
    /// The adapter/driver name, where the platform has nothing better. Best-effort: two identical
    /// cards share it.
    Name(String),
    /// A platform with one GPU, or a backend that cannot enumerate: there is only "the GPU".
    Single,
}

/// The handle the backend matches in this process.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SessionGpuId {
    /// A DRM render node (Linux), from `VK_EXT_physical_device_drm`; the node number is assigned
    /// per boot and is not a hardware id.
    Drm { render_minor: u32 },
    /// A DXGI adapter LUID (Windows), packed from `Luid.HighPart`/`LowPart`. It lasts for the
    /// adapter's lifetime and changes on reboot, driver upgrade, or device disable/enable.
    Luid(u64),
    /// A Metal registry id (macOS) — an IOKit registry entry id, valid only until reboot.
    Metal { registry_id: u64 },
    /// A platform with one GPU, or a backend whose enumeration cannot name one.
    Default,
}
```

[`GpuAdapterInfo`](crates/gpui_platform/src/gpu.rs) carries one `pub id: GpuId`, filled with a
`session` by `enumerate_gpus` and `active_gpu`. `vendor_id`/`device_id` become accessors reading the
`Pci` permanent part (`None` where the backend has no PCI identity). Matching is:

- a `GpuId` **with** a session matches only the adapter with that session — the exact card in this
  process;
- a `GpuId` **without** one (a config, or a caller that only knows the model) matches the permanent
  part, with `None` as a wildcard for the PCI fields; two identical cards both match and the first
  is chosen.

The same `GpuId` therefore round-trips through a config as its permanent part and re-resolves at
startup. The per-backend values are:

| backend | `permanent` | `session` |
| --- | --- | --- |
| Linux | `Pci { vendor, device }` | `Drm { render_minor }`, falling back to the PCI pair where the extension is absent |
| Windows | `Pci { vendor, device, subsys, revision }`; later `Pnp` | `Luid` |
| macOS | `Name` (or `Single` where `all()` is unavailable) | `Metal { registry_id }` |
| one GPU / no enumeration | `Single` | `Default` |

- `ZED_DEVICE_ID` lowers to `GpuId { permanent: Pci { vendor_id: None, device_id, .. }, session: None }`
  and stays a Linux read.
- Windows carries the LUID because it is the identity a producer that shares a texture matches on;
  the PCI pair is what a config stores. The probe measured that DXGI's LUID and wgpu's DX12 LUID are
  the same number ([`windows-adapter-luid-probe.md`](windows-adapter-luid-probe.md) §2).
- macOS has no PCI identity. `MTLDevice::registryID` is global to all tasks, but it is an IOKit
  registry entry id, which Apple documents as valid only until reboot — hence the `Name` permanent
  part and the registry id as the session handle.
- **Rejected:** one tagged id that is both stored and matched. It either cannot survive a restart
  (a LUID or registry id) or cannot name one of two identical cards (a PCI pair), and the caller
  cannot tell which promise it is getting.

The platform documentation says why the split is required:

| identifier | documented lifetime | source |
| --- | --- | --- |
| PCI vendor/device (`DXGI_ADAPTER_DESC1.VendorId`/`DeviceId`, Linux PCI) | hardware ids; stable across restarts, but two identical cards share them | [`DXGI_ADAPTER_DESC1`](https://learn.microsoft.com/en-us/windows/win32/api/dxgi/ns-dxgi-dxgi_adapter_desc1) |
| Windows PnP **device instance id** (`PCI\VEN_…\1&08`) | "persistent across system restarts" | [Device instance ID](https://learn.microsoft.com/en-us/windows-hardware/drivers/install/device-instance-ids) |
| DXGI LUID / DXCore `InstanceLuid` | "changes on reboot, driver upgrade, or device disablement/enablement" | [`DXCoreAdapterProperty`](https://learn.microsoft.com/en-us/windows/win32/api/dxcore_interface/ne-dxcore_interface-dxcoreadapterproperty), [MS-DTYP LUID](https://learn.microsoft.com/en-us/openspecs/windows_protocols/ms-dtyp/48cbee2a-0790-45f2-8269-931d7083b2c3) |
| Metal `registryID` | "global to all tasks" — it is an IOKit registry entry id, and those are "valid only until the machine reboots" | [`registryID`](https://developer.apple.com/documentation/metal/mtldevice/registryid), [`IORegistryEntryGetRegistryEntryID`](https://developer.apple.com/documentation/iokit/1514719-ioregistryentrygetregistryentryi) |
| Metal device `name` | not an identity: two identical cards share it | 0008 |

The contract a caller can rely on:

- `Device(GpuId)` with a session is "this exact adapter, this run"; with only a permanent part it is
  "this model of card, this boot", re-resolved through `enumerate_gpus` and falling back to `Auto`
  when nothing matches.
- A preferences panel stores the permanent part, not the session.
- "This exact card across restarts" needs `Pnp` on Windows and has no documented equivalent on
  macOS; a saved macOS choice is a type or a name, and is best-effort.
- On a platform with one GPU, or one whose enumeration is not implemented, `enumerate_gpus` returns
  one synthesized entry with `PermanentGpuId::Single` and `SessionGpuId::Default`; any `Device`
  preference resolves to it (or falls back to `Auto`), and the contract that every reported adapter
  has both parts still holds.

## 3. Selection: one ranking, `Auto` is the backend's own order

The pure selection code currently lives in `gpui_wgpu` —
[`custom_choice`](crates/gpui_wgpu/src/wgpu_context.rs), [`adapter_key`](crates/gpui_wgpu/src/wgpu_context.rs),
`type_priority` — and would otherwise be copied three times. Move it beside the types in
`gpui_platform`:

```rust
pub fn rank(
    adapter: &GpuAdapterInfo,
    preference: &GpuPreference,
    compositor: Option<&PermanentGpuId>,
) -> GpuRank;

pub fn custom_choice(
    adapters: &[GpuAdapterInfo],
    selector: &dyn Fn(&[GpuAdapterInfo]) -> Option<usize>,
) -> anyhow::Result<usize>;
```

- `LowPower`, `HighPerformance`, `Device` and `Custom` are portable: device type, the id match, the
  chosen index. `Device(GpuId)` matches the session when it has one and the permanent part otherwise
  (§2). `Custom`'s chosen adapter is tried first and the rest keep their order as fallbacks; an
  invalid index or `None` is an error naming the shortfall.
- **`Custom` is the application's runtime query.** It receives the whole list — `permanent` and
  `session` ids, `device_type`, `backend`, `is_software`, and the capability fields 0012 adds — and
  returns one index. That covers both `GpuPreference` shapes the application needs: "take the first
  adapter by a stable id order" is a `Custom` that sorts on `permanent`, and "the adapter that can
  decode video / drive the 3D engine" is a `Custom` that reads the capability report. Nothing is
  hardcoded in the platform for either.

  ```rust
  // The first adapter by a stable order, without user interaction.
  GpuPreference::Custom(Arc::new(|gpus| {
      gpus.iter()
          .enumerate()
          .min_by_key(|(_, gpu)| &gpu.id.permanent)
          .map(|(index, _)| index)
  }));

  // The best adapter for the job, once 0012's capability report is on `GpuAdapterInfo`.
  GpuPreference::Custom(Arc::new(|gpus| {
      gpus.iter().position(|gpu| gpu.capabilities.supports_video_decode())
  }));
  ```

  A `Custom` is not serialisable; an application that offers "remember this policy" stores its own
  small enum and rebuilds the closure at startup. `Device(GpuId)` remains the serialisable path for
  "this card".
- `Auto` is not a cross-backend ranking. Each backend establishes its own order **first** — DXGI's
  on Windows, the existing `create_device` key on macOS, the compositor/type key on Linux — and the
  shared `rank` returns a constant under `Auto`, so the stable sort preserves that order. Only the
  explicit preferences overrule it; the compositor hint is used as a tie-break for those, and is
  `None` where a backend has no equivalent.
- The backend sorts **pairs** of `(GpuAdapterInfo, native handle)`, not indices into one list and
  then another: an OpenGL adapter reports `device = 0`, so matching a sorted info back to an adapter
  by vendor/device would be ambiguous.
- `Custom` sees the list the backend's `enumerate_gpus` returns, in that order, so an index an
  application derived from `App::enumerate_gpus` is the index the selector sees — before any sorting.

`gpui_wgpu` loses its private copy and calls the shared functions; `gpui_linux` keeps
`CompositorGpuHint` and converts it to `PermanentGpuId::Pci` for the tie-break. This is the one
behaviour the Linux suites must confirm unchanged.

## 4. Windows

### Enumeration

`DirectXDevices::enumerate_adapters()` — the factory is already created by
[`get_dxgi_factory`](crates/gpui_windows/src/directx_devices.rs), then
`IDXGIFactory6::EnumAdapters1`. **No `D3D11CreateDevice`**: enumeration must not create a device.
Map [`DXGI_ADAPTER_DESC1`](crates/gpui_windows/src/directx_devices.rs):

| `GpuAdapterInfo` | source |
| --- | --- |
| `name` | `Description`, NUL-trimmed |
| `id.permanent` | `Pci { vendor_id: VendorId, device_id: DeviceId, subsys_id: Some(SubSysId), revision: Some(Revision) }`; `Pnp` later |
| `id.session` | `Luid(AdapterLuid)` |
| `is_software` | `Flags & DXGI_ADAPTER_FLAG_SOFTWARE`, or DXCore's `IsHardware == false`; the probe shows a Basic Render Driver both flagged and unflagged, so neither the flag alone nor the name is enough |
| `device_type` | `Cpu` when software; otherwise `Integrated` or `Discrete` from DXCore's `IsIntegrated`, `Other` when DXCore cannot answer |
| `backend` | a new `GpuBackend::Dx11`; `Dx12` stays for wgpu |

Do **not** classify integrated versus discrete from `DedicatedVideoMemory`: an iGPU reserves a small
slice and a low-end dGPU has a modest one, so any threshold is a magic number that mislabels hardware.
Where DXCore cannot answer, `Other` is honest, and `LowPower`/`HighPerformance` fall back to the DXGI
order — which is the OS's own preference — rather than guessing.

### Selection and the text system

`DirectXDevices::new(preference)` orders the enumerated pairs with the shared rank, then walks them
creating a device through [`get_device`](crates/gpui_windows/src/directx_devices.rs), returning the
first that succeeds. That is the existing capability test; the swapchain is still validated later, as
it is today.

The preference cannot simply be a field read at `open_window`, because the platform creates its
devices eagerly for the text system:
[`WindowsPlatform::new`](crates/gpui_windows/src/platform.rs) builds
`DirectXDevices::new(&GpuPreference::Auto)` and then
`DirectWriteTextSystem::new(&devices)`, whose `GPUState` is bound to that device. `App` captures the
text system at construction, before `with_gpu_preference` runs. `new` keeps building the default
(`Auto`) devices for that reason, and the stated preference is applied at the first window:

1. `WindowsPlatformState` stores `gpu_preference` and a `preference_applied` flag.
2. `set_gpu_preference` records it and clears the flag. It does not create a device: it runs before
   `run`, and the trait method returns nothing to report an error through.
3. [`open_window`](crates/gpui_windows/src/platform.rs) — which returns `Result` — applies it. If the
   flag is set, it builds `DirectXDevices::new(&preference)` and, on success, swaps the state's
   devices and rebuilds the text system's GPU state in place with
   [`DirectWriteTextSystem::handle_gpu_lost`](crates/gpui_windows/src/direct_write.rs) (reused, or
   renamed `handle_device_change`). On failure it fails the window with the named shortfall. Swapping
   the `Arc` would not help: the app already holds the old one.
4. After the first window, a later `set_gpu_preference` is refused and logged, per 0008.

`active_gpu` maps the live `state.directx_devices.adapter` through `GetDesc1`. It is `None` in
headless mode, where [no devices are made](crates/gpui_windows/src/platform.rs), and before the first
window.

Every `DirectXDevices::new()` call that exists for device recovery
([`platform.rs`](crates/gpui_windows/src/platform.rs), [`directx_renderer.rs`](crates/gpui_windows/src/directx_renderer.rs))
takes the applied preference, so a lost device returns on the same adapter.

A LUID choice is a `GpuId` whose `permanent` is the adapter's `Pci` part and whose `session` is
`Some(SessionGpuId::Luid(…))`; that is the value `enumerate_gpus`/`active_gpu` hand back. A config
stores the `Pci` (or later `Pnp`) permanent part and leaves the session out. The producer side
already resolves a Direct3D device to its adapter by LUID in `gpui_interop`'s Windows arm, so no new
mapping is needed — only exposing both parts in `GpuAdapterInfo::id`.

## 5. macOS

### Enumeration

`metal::Device::all()` (`MTLCopyAllDevices`), with `MTLCreateSystemDefaultDevice` as the fallback when
the list is empty — upstream has seen `all()` return nothing
([zed#37689](https://github.com/zed-industries/zed/issues/37689)). On iOS/tvOS `all()` does not exist
and there is one GPU; see below.

| `GpuAdapterInfo` | source |
| --- | --- |
| `name` | `MTLDevice::name` |
| `id.permanent` | `Name(MTLDevice::name)` (or `Single` where `all()` is unavailable) |
| `id.session` | `Metal { registry_id: registryID }` |
| `device_type` | `Integrated` when `has_unified_memory` or `is_low_power`; `Discrete` when `!is_low_power && !is_removable`; `Other` for a removable/eGPU |
| `is_software` | `false` — Metal exposes no software device here |
| `backend` | `Metal` |

`vendor_id`/`device_id` are `None`.

### Selection

[`MetalRenderer::create_device`](crates/gpui_apple/src/metal_renderer.rs) already implements today's
policy: `min_by_key((is_removable, !is_low_power))`, i.e. non-removable, low-power. Make it
`create_device(preference)`:

- `Auto` keeps that key exactly;
- `LowPower` prefers `is_low_power`/unified memory;
- `HighPerformance` prefers `!is_low_power && !is_removable`, falling back to `Auto`;
- `Device`/`Custom` select from the enumerated list with the shared rank, then fall back through
  `Auto`'s order if the chosen device cannot be used.

`create_device` answers `Option`/`Result` rather than exiting: an invalid `Custom` must name its
shortfall, and a device that cannot make a layer should fall through to the next in `Auto`'s order.
[`new_renderer`](crates/gpui_apple/src/metal_renderer.rs) becomes fallible, and the default arm in
[`Window::new`](crates/gpui_macos/src/window.rs) propagates it with `?`.

`MacPlatformState` stores `gpu_preference`; [`set_gpu_preference`](crates/gpui_macos/src/platform.rs)
writes it, and `Window::new` passes it to `new_renderer`. The custom `renderer_factory` path is a
renderer of the embedder's own and is not steered by this preference; say so.

`active_gpu` records the chosen `GpuAdapterInfo` when the renderer is built — the platform already
holds the renderer context (`renderer::Context`), so a `selected_gpu` slot on `InstanceBufferPool`
reaches `MacPlatform::active_gpu` without a new coupling. It is `None` until the first renderer
exists.

- **iOS/tvOS:** one GPU, `all()` unavailable. `enumerate_gpus` returns one synthesized entry with
  `PermanentGpuId::Single` and `SessionGpuId::Default`; every preference resolves to it (and an
  unmatched `Device` falls back to it), so the contract that a reported adapter has both parts still
  holds. There is no user-visible choice.
- **Interop:** macOS shares the renderer's `MTLDevice` directly, so the registry id is a preference
  identity only, not a transport identity. No `gpui_interop` change is implied.

## 6. The default is per backend

0008 chose `LowPower` because on the measured Linux laptop the panel is wired to the iGPU: a dGPU
renderer stays powered and every frame crosses PCIe to be scanned out. That argument does not
transfer unchanged.

- **macOS** already picks the low-power, non-removable device, so `LowPower` is today's behaviour.
- **Windows** takes the first DXGI adapter, in the OS's order; on a hybrid laptop that may be the
  discrete one. There is no measured Windows equivalent of the Linux panel-wiring argument, so
  `LowPower` as the default would change existing behaviour on the strength of a Linux measurement.
  This has not been measured on a Windows hybrid.

**Proposal:** the ability is uniform; the default is the backend's current policy — `LowPower` where
it is already the behaviour or where the 0008 argument applies (Linux, macOS), `Auto` on Windows
until a Windows hybrid is measured. Amend 0008's "the default is `LowPower`" to that sentence. An
application that wants one policy everywhere states it with `Application::with_gpu_preference`, which
already overrides every backend.

## 7. Not in this design

- Runtime switching after a window exists (0008 refuses it; Windows would have to tear down D3D11
  devices and swapchains, macOS the layers).
- What a chosen device can **import or export** — 0012's capability tier, asked on a device the
  caller already has.
- A compositor hint on Windows or macOS: there is no equivalent, so their tie-break is `None`.
- A **capability DSL**. `Custom` is the query seam; 0012 supplies the capability data and the
  platform does not interpret it.
- wasm: a synthesized `Single`/`Default` adapter; there is no choice to make.
- iOS/tvOS beyond that single synthesized adapter.

## 8. Validation

- **Pure tests**, on synthetic `GpuAdapterInfo`s: each backend's descriptor mapping, the shared
  `rank`, `Custom` choosing and refusing, and the identity rules — a session matches only the same
  session, a permanent-only id matches the permanent part, and a `Pci` id never matches a
  `Name`/`Single` or a `Metal` session. A `GpuId` serialises without its session and deserialises
  with `None`, then still matches after a simulated restart. The existing Linux ranking tests move
  with the code.
- **Windows**, on the guest from [`0009-windows-probe-vm.md`](../issues/0009-windows-probe-vm.md)
  with `script/d3dprobe`: enumerate and assert the Intel HD 520 `[8086:1916]` and its LUID against
  the probe printout ([`windows-adapter-luid-probe.md`](windows-adapter-luid-probe.md) §2); select it
  with a full `GpuId` (PCI permanent + LUID session), with the permanent-only PCI id from a config,
  and with `Custom`; assert `active_gpu`; assert a refused `Custom` fails the window with the
  shortfall. The **two-GPU** case remains unmeasured (P6 §4) and needs a hybrid laptop.
- **macOS**, on Intel (integrated + discrete) and Apple Silicon: enumerate, select each by its
  permanent `Name` and its session registry id, assert `active_gpu`, and assert the `Auto` choice is
  unchanged. There is no macOS CI host today, so this stays a manual record until one exists.
- **Single-GPU fallback**: a platform that cannot enumerate reports one
  `Single`/`Default` adapter; `enumerate_gpus` is non-empty, `Device(Single)` selects it, and an
  unrelated `Device` falls back to it rather than failing.
- **Linux regression**: `gpui_wgpu`'s and `gpui_va`'s `surface_dmabuf` suites (8/8 and 14/14 on
  Intel and the NVIDIA 930M) after `gpui_wgpu` switches to the shared rank; `ZED_DEVICE_ID` unchanged.
- **Cross-backend**: one `Custom` predicate ("the adapter named *NVIDIA*", or "the one with the
  video capability") compiles and selects on every backend; a session from one backend never matches
  another, and the permanent variant is part of the match.

## 9. Open questions

1. **DXCore feature.** `Win32_Graphics_DxCore` needs adding to the workspace `windows`
   features, and `IsIntegrated` needs Windows 10 2004+. Is that floor acceptable, or should the first
   cut report `Other` and defer classification?
2. **Persistent identity — in the contract.** `GpuId` now carries both parts (§2); `permanent` is
   serialised, `session` is not. The only open part is whether the Windows PnP device instance id
   (`PermanentGpuId::Pnp`) ships in the first Windows slice or later; the draft assumes later.
3. **Linux session handle.** `SessionGpuId::Drm` reads `VK_EXT_physical_device_drm` through the wgpu
   HAL. Is it available on every adapter the Linux renderer can expose, and is the PCI fallback
   enough where it is not? (The measured pair exposes it.)
4. **`GpuBackend::Dx11`.** Adding the variant is a public enum change. Is the Windows renderer's API
   correctly called Direct3D 11 rather than 12, or should the field stay coarse?
5. **Where the Windows text-system swap lives.** `DirectWriteTextSystem::handle_gpu_lost` already
   rebuilds `GPUState`; reuse it, or add an explicit `handle_device_change` so the recovery and
   preference paths do not read as the same event?

## 10. Plan

1. **Identity contract and shared ranking** in `gpui_platform`: `GpuId` with `PermanentGpuId` and
   `SessionGpuId`, serialisation that drops the session, the `Single`/`Default` fallback and the
   shared rank; migrate `gpui_wgpu` and `gpui_linux` to it with no behaviour change; add the pure
   tests. This step is platform-neutral and can land first.
2. **Windows**: enumerate, select, apply at the first window, swap the text system's GPU state,
   implement `active_gpu`, and pass the preference through recovery. Validate on the Windows probe
   VM ([issue 0009](../issues/0009-windows-probe-vm.md)).
3. **macOS**: enumerate, select in `create_device`, thread the preference through `new_renderer`,
   record the active device, make `new_renderer` fallible. Validate on a Mac.
4. **Defaults and record**: settle §6 (amend 0008), note the session scope of `Device`, and point
   `gpui_interop` at the new identity in its docs only.
