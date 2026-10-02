- **Opened:** 2026-10-02
- **Status:** closed — accepted: the Linux arm is linear-only, and the fix is future `wgpu-hal` work
- **Touches:** `bite-gpui/crates/gpui_wgpu/src/dmabuf.rs`, the `wgpu`/`wgpu-hal` dependency, [`../decisions/linux-dmabuf-probe.md`](../decisions/linux-dmabuf-probe.md), [`../spi/rendering/surfaces.md`](../spi/rendering/surfaces.md)

# A tiled dma-buf cannot be imported on wgpu's device; the arm is linear-only

## Decision

**Accept linear-only.** `SurfaceSource::DmaBuf` imports `DRM_FORMAT_MOD_LINEAR` buffers; a
vendor-tiled modifier is refused with a message. The limitation is deliberate, not unbuilt work, and
the change that would lift it is future `wgpu-hal` work §"Future work", not something this project
forks `wgpu` to get.

Why accept it:

- **Cross-vendor sharing needs linear anyway.** A vendor-private modifier means nothing to a different
  vendor, so any buffer that crosses GPUs must be `DRM_FORMAT_MOD_LINEAR` (P3 measured both
  directions).
- **A same-vendor producer can usually be asked to allocate linear**, and the cost — bandwidth on one
  device — is the smaller problem for GPUI's use than carrying a `wgpu` fork.
- **The fix is a dependency change, not ours.** Enabling the extension is a `wgpu-hal` change; forking
  `wgpu` for it is a maintenance commitment the benefit does not justify today.

## The problem, and why it is wgpu's

Sampling a tiled buffer requires creating the `VkImage` with `VK_IMAGE_TILING_DRM_FORMAT_MODIFIER_EXT`
and an explicit [`VkImageDrmFormatModifierExplicitCreateInfoEXT`][ext] carrying the plane layout. Both
come from `VK_EXT_image_drm_format_modifier`, and the spec requires that extension to be *enabled on
the device*: `VUID-VkImageCreateInfo-tiling-02261`.

The image must be created on the device that samples it — the renderer's — and the renderer's Vulkan
device is `wgpu`'s. `wgpu-hal` 29.0.4 selects device extensions from an explicit list
(`wgpu-hal/src/vulkan/adapter.rs` `get_required_extensions`, ~L1171–1382): it enables
`VK_KHR_external_memory_fd` and `VK_EXT_external_memory_dma_buf` (which is why the *linear* import
works) and `VK_KHR_external_memory_win32`, but **not** `VK_EXT_image_drm_format_modifier`. There is no
mention of it anywhere in `wgpu`, `wgpu-core` or `wgpu-hal`, no `wgpu::Features` bit for it, and no
`DeviceDescriptor` hook to add a raw device extension. `wgpu` exposes `Device::as_hal`/`Queue::as_hal`
— access to what it made, not influence over what it makes. Calling it anyway is undefined behaviour
per the spec, so it is not a path to ship.

**Why the probe did not see this:** P3 created its *own* device and enabled the extension on it
(`probes/linux-dmabuf/src/main.rs:679`). The renderer's device is not that device. The probe answered
"can a tiled dma-buf be imported?" — yes — but not "on the device GPUI actually draws with". P3's
result stands; the missing piece is enabling the extension on `wgpu`'s device.

## The limitation, as shipped

`gpui_wgpu::dmabuf` refuses any modifier but `DmaBufHandle::LINEAR`, with a message that names this
reason. `spi/rendering/surfaces.md` §2 records it, and `DmaBufHandle` is unchanged — a descriptor that
carries a modifier is still valid; the renderer simply cannot import a non-linear one.

## Future work: the `wgpu-hal` change that would lift it

To import vendor-tiled buffers, enable `VK_EXT_image_drm_format_modifier` on the renderer's device and
create the modifier image. In order of preference:

1. **Upstream a small `wgpu-hal` change** — add the extension to the optional list in
   `get_required_extensions` when the physical device supports it, beside the `external_memory_*`
   entries already there. Clean, benefits every `wgpu` user, but on `wgpu`'s release cadence. This is
   the recommendation.
2. **A `[patch.crates-io]` fork of `wgpu-hal`** carrying that one change — unblocks now, at the cost
   of maintaining a `wgpu` fork. The tree already patches several crates (`Cargo.toml`
   `[patch.crates-io]`), so the mechanism exists; the commitment is new.

Once the extension is enabled, the `gpui_wgpu` half is mechanical: build the
`VkExternalMemoryImageCreateInfo` + `ImageDrmFormatModifierExplicitCreateInfoEXT` chain from the
handle's plane offsets and strides, and lift the linear-only `ensure!`. The descriptor and the tests
already carry the modifier, so nothing above the renderer changes.

[ext]: https://registry.khronos.org/vulkan/specs/latest/man/html/VkImageDrmFormatModifierExplicitCreateInfoEXT.html
