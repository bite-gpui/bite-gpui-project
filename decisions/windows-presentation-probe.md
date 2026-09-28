# Evidence: the Windows presentation probe

- **Evidence for** [decision 0002](0002-render-extension-device-model.md) — item 2's Windows
  clause, that Path A and Path B there require installing `gpui_wgpu::WgpuRenderer`. Kept as
  evidence, not as a proposal: what the design does with the answer is
  [`../spi/rendering/renderer-seam.md`](../spi/rendering/renderer-seam.md) §6's Windows commit.
- **The question:** can `gpui_wgpu::WgpuRenderer` own a Windows window's presentation?
  **Answered: yes — on Direct3D 12, and not on the backends `gpui_wgpu` asks for today.**
- **The probe** is the crate at `probes/windows-wgpu-present`, on `bite_v1.22.0-pre`
  (PR #2). Its job answered and was removed, so the printout in §2 is the durable record
  and the crate is the reproduction.

## 1. What it answered

One `windows-latest` job, five attempts, each building a surface from the `HWND` the window
was created with:

| backend set | surface from the `HWND` | adapter | a frame presented |
| --- | --- | --- | --- |
| `VULKAN \| GL` — what `gpui_wgpu` asked for at the probe (`crates/gpui_wgpu/src/wgpu_context.rs:309`) | created | **none** | — |
| `DX12` | created | `Microsoft Basic Render Driver`, `Dx12`, `Cpu` | **yes** |
| `VULKAN` | not created | — | — |
| `GL` | created | **none** | — |
| `DX12`, with a DirectComposition tree live on the window | created | `Microsoft Basic Render Driver` | **yes** |

Three things follow, and the first is the one that would have cost a patch:

- **`gpui_wgpu`'s instance did not enable Direct3D 12.** Its non-wasm instance was
  `Backends::VULKAN | Backends::GL`
  (`crates/gpui_wgpu/src/wgpu_context.rs:309`) — right for Linux, and never run on Windows,
  where it found no adapter at all: *"vulkan drivers/libraries could not be loaded ... dx12 not
  requested, gl found no adapters"*. So `WgpuRenderer::new` failed on Windows **before**
  presentation was reached, and the Windows commit was not plumbing alone: `gpui_wgpu` had to
  enable `DX12`, which the seam does (`crates/gpui_wgpu/src/wgpu_context.rs:311`).
- **Once it does, presentation works.** A surface built from `RawWindowHandle::Win32` over
  the `HWND` finds the DX12 adapter, configures, acquires a frame and presents it.
- **A DirectComposition tree on the same `HWND` does not stop it.** Committing a target,
  visual and composition swap chain the way `gpui_windows` does
  (`crates/gpui_windows/src/directx_renderer.rs:1047`) and then presenting from wgpu on the
  same window was accepted. What that does not show is which of the two is *visible* — the
  probe never presents the composition swap chain — so it is "they coexist", not "the
  window can keep both".

## 2. The printout

```
window created: HWND(0x40152)
machine adapter: "Microsoft Basic Render Driver", backend = Dx12, device_type = Cpu
--- gpui_wgpu's set (VULKAN | GL): Backends(VULKAN | GL) ---
gpui_wgpu's set (VULKAN | GL): surface created from the raw handle
gpui_wgpu's set (VULKAN | GL): NO ADAPTER: No suitable graphics adapter found; noop not
    requested, vulkan drivers/libraries could not be loaded, metal not requested,
    dx12 not requested, gl found no adapters, webgpu not requested
--- DX12: Backends(DX12) ---
DX12: surface created from the raw handle
DX12: adapter = "Microsoft Basic Render Driver", backend = Dx12, device_type = Cpu
DX12: formats = [Bgra8UnormSrgb, Rgba8UnormSrgb, Bgra8Unorm, Rgba8Unorm, Rgb10a2Unorm, Rgba16Float]
DX12: present_modes = [Mailbox, Fifo, Immediate]
DX12: alpha_modes = [Opaque]
DX12: configured format = Bgra8UnormSrgb, alpha_mode = Opaque
DX12: PRESENT OK
--- VULKAN: Backends(VULKAN) ---
VULKAN: surface NOT created: Failed to create surface for any enabled backend: {}
--- GL: Backends(GL) ---
GL: surface created from the raw handle
GL: NO ADAPTER: No suitable graphics adapter found; ... gl found no adapters, ...
--- probe 2: a DirectComposition tree on the HWND, then wgpu ---
probe 2: D3D11 device created (hardware)
probe 2: composition swap chain created
probe 2: DirectComposition target, visual and swap chain committed
--- DX12 with the dcomp tree live: Backends(DX12) ---
DX12 with the dcomp tree live: adapter = "Microsoft Basic Render Driver", backend = Dx12, device_type = Cpu
DX12 with the dcomp tree live: configured format = Bgra8UnormSrgb, alpha_mode = Opaque
DX12 with the dcomp tree live: PRESENT OK
```

## 3. The outcome, and what it costs

**Yes**, with the backend fix — and one casualty:

- **Transparency.** The DX12 surface reports `alpha_modes = [Opaque]` and nothing else.
  GPUI's transparent path is DirectComposition's, which a wgpu surface does not offer, so a
  window rendered by `WgpuRenderer` on Windows cannot be per-pixel transparent the way the
  default renderer's can. That is the "partly" outcome this spike anticipated, and it
  belongs in the guide and in `WgpuSurfaceConfig`'s documentation rather than in a patch.
- **Colour space.** The surface offers `Bgra8UnormSrgb` first and accepts it, where GPUI's
  Windows target is `B8G8R8A8_UNORM` (`crates/gpui_windows/src/directx_renderer.rs:32`).
  The probe shows both are reachable; it does not settle which a foreign texture should use,
  which is [`../spi/rendering/foreign-texture.md`](../spi/rendering/foreign-texture.md) §4.

## 4. What is not measured

- **Hardware.** The runner's only adapter is `Microsoft Basic Render Driver` — WARP — so
  what is established is one adapter, exactly as
  [`windows-path-a-probe.md`](windows-path-a-probe.md) records for its own result. Hardware
  is expected to behave the same and was not what was measured.
- **Transparency on a real window**, which needs the readback fixture
  [`../spi/rendering/verification.md`](../spi/rendering/verification.md) §1 describes rather
  than the capability list.
- **Resize and device loss on the wgpu surface** — §1's rows, and the acceptance test for
  the Windows commit rather than a question about it.
