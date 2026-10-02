- **Opened:** 2026-10-02
- **Status:** open — blocked on a decision about the `wgpu` dependency
- **Touches:** `bite-gpui/crates/gpui_wgpu/src/dmabuf.rs`, the `wgpu`/`wgpu-hal` dependency, [`../decisions/linux-dmabuf-probe.md`](../decisions/linux-dmabuf-probe.md), [`../spi/rendering/surfaces.md`](../spi/rendering/surfaces.md)

# A tiled dma-buf cannot be imported: `wgpu` does not enable `VK_EXT_image_drm_format_modifier`

## The problem

P3 proved a vendor-tiled dma-buf (`I915_FORMAT_MOD_Y_TILED`) round-trips on this machine
([`../decisions/linux-dmabuf-probe.md`](../decisions/linux-dmabuf-probe.md)), but the production arm
refuses anything but `DRM_FORMAT_MOD_LINEAR` (`crates/gpui_wgpu/src/dmabuf.rs`). The comment read "for
now", implying it was unbuilt work. It is not: it is **blocked by `wgpu`**, and cannot be built in
`gpui_wgpu` alone.

## Why

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
`DeviceDescriptor` hook to add a raw device extension.

`wgpu` exposes only `Device::as_hal`/`Queue::as_hal` — access to what it made, not a way to influence
what it makes. So a `DRM_FORMAT_MODIFIER_EXT` image cannot be created on the renderer's device, and a
tiled buffer cannot be sampled. (Calling it anyway is undefined behaviour per the spec, and would be a
validation error under layers; that is not a path to ship.)

**Why the probe did not see this:** P3 created its *own* device and enabled the extension on it
(`probes/linux-dmabuf/src/main.rs:679`). The renderer's device is not that device. The probe answered
"can a tiled dma-buf be imported?" — yes — but not "on the device GPUI actually draws with".

## What would close it

Enable `VK_EXT_image_drm_format_modifier` on the renderer's device, then create the modifier image in
`gpui_wgpu`. Three ways, in order of preference:

1. **Upstream a small `wgpu-hal` change** — add the extension to the optional list when the physical
   device supports it (alongside the `external_memory_*` entries already there). Clean, benefits every
   `wgpu` user, but on `wgpu`'s release cadence. This is the recommendation.
2. **A `[patch.crates-io]` fork of `wgpu-hal`** carrying that one change — unblocks now, at the cost of
   maintaining a `wgpu` fork. The tree already patches several crates (`Cargo.toml` `[patch.crates-io]`),
   so the mechanism exists; the commitment is new.
3. **Accept linear-only.** Cross-vendor sharing needs `DRM_FORMAT_MOD_LINEAR` regardless (a
   vendor-private modifier means nothing to another vendor), and most in-process producers can be told
   to allocate linear. The cost is bandwidth for a same-vendor tiled producer.

With (1) or (2), the `gpui_wgpu` side is mechanical: build the `VkExternalMemoryImageCreateInfo` +
`ImageDrmFormatModifierExplicitCreateInfoEXT` chain from the handle's plane offsets/strides, and lift the
linear-only `ensure!`. The descriptor and the tests already carry the modifier.

[ext]: https://registry.khronos.org/vulkan/specs/latest/man/html/VkImageDrmFormatModifierExplicitCreateInfoEXT.html
