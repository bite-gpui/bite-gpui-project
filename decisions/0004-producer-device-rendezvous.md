# 0004 — How a producer reaches the renderer's device

- **Decided:** 2026-09-29
- **Status:** decided
- **Evidence:** [`../spi/rendering/producer-reach.md`](../spi/rendering/producer-reach.md) — the gap,
  what each platform can do, and what is left of it; `bite_v1.22.0-pre-path-a` (PR #6), whose last
  commit is the producer's half. `bite_v1.22.0-pre-device-rendezvous` was replayed into that branch
  and is obsolete.
- **Touches:** [`0002-render-extension-device-model.md`](0002-render-extension-device-model.md) — the
  device rule this implements, and its "the rendezvous is one slot, and it works both ways", whose
  gpui→app half is the thing the code did not have; [`../spi/rendering/milestones.md`](../spi/rendering/milestones.md)
  — this was its M1; [`../spi/rendering/renderer-seam.md`](../spi/rendering/renderer-seam.md) §5.6
  (where a post-construction query belongs) and §10 (what would reopen the seam).

## Decision

**A window lends its renderer's device, erased on the shared traits and typed in the crate that owns
the type.** `PlatformRenderer::device_any`, `PlatformWindow::device_any` and `Window::device_any`
return `Option<Rc<dyn Any>>`; each backend answers with what it already holds; and the typed form an
application writes is a trait in the facade, because that is the lowest crate that can name both a
`Window` and a backend's types.

| what each renderer lends | why that is what it has |
| --- | --- |
| `WgpuRenderer` | the shared `GpuContext` slot — the same `Rc` every window in the process draws through, so a producer reads `device` and `queue` from the rendezvous 0002 already describes instead of a second one |
| `DirectXRenderer` | its `ID3D11Device`, which the platform built (`WindowsPlatformState.directx_devices`, `crates/gpui_windows/src/platform.rs:84`) and the renderer was constructed from (`crates/gpui_windows/src/window.rs:165`) |
| `MetalRenderer` | its `MTLDevice`, which it created (`crates/gpui_apple/src/metal_renderer.rs:195`) and is therefore the only holder of |

`None` is an answer rather than a failure: a renderer that draws offscreen, or one a factory
installed that is not the backend's own, has nothing to lend.

The accessor is half of the reach; the other half is the **token builder**, and it ships the same
way. `ImportedTextureExt`, `DirectXTextureExt` and `MetalTextureExt` are each defined in the crate
that names the API — which is also where the payload is downcast — and re-exported through the
facade, so an application writes `gpui::ImportedTextureExt`, `gpui::DirectXTextureExt` or
`gpui::MetalTextureExt`. The one exception is a Windows window whose factory installed
`WgpuRenderer` rather than the platform's own renderer: that one takes its token from `gpui_wgpu`,
the crate the renderer comes from, because none of the platform crates depend on it. They are not
traits *for* `Window`, so the orphan rule that decides the accessor's home does not constrain them;
what they add to the decision is that the per-backend payload types become published surface, which
"what would reopen this" names.

## Why

**The anchor, not the direction, was the fork.** Both directions of the rendezvous serve someone, and
app→gpui is already reachable for wgpu through the factory. What was missing was gpui→app, and the
question was where a *typed* accessor could hang: a type the application already holds is a `Window`
or a renderer reached through `PlatformWindow::with_renderer`
(`crates/gpui_platform/src/platform_window.rs:151`), which hands out `&mut dyn SceneRenderer` and no
device.

**Owned rather than borrowed.** The only route from a window to a renderer is a closure, and no
borrow outlives one, so a `&dyn Any` accessor cannot be written at all. `Rc<dyn Any>` costs an
allocation per call and removes the lifetime question, and `Rc` rather than `Arc` is honest: the
renderer is `!Send` and 0002 already makes this a same-thread affordance.

**Erased, because the shared trait may not name a device.** The seam's rule is that it "must not name
`metal::*` or `ID3D11Device*`", which is why `MacSceneRenderer` and `WinSceneRenderer` are
cfg-selected supertraits rather than methods on `SceneRenderer`. An erased payload plus a typed trait
in the crate that owns the type is that rule applied to a return value.

**The typed form lives in the facade, and that is not where the sketch put it.** Every backend crate
was a candidate and none of them works: no backend crate depends on `gpui_authoring`, so none can
implement a trait for `Window`. The facade depends on both, and `platform_entry.rs` is already where
its per-platform re-exports live, beside the wasm one.

**What makes the window the right anchor is that it already forwards.** `Window` owns
`Box<dyn PlatformWindow>` (`crates/gpui_authoring/src/window.rs:1265`) and reaches it for everything
it does not own itself (`:1787`), so one erased method on each of the two traits is the whole of the
plumbing, and neither `SceneRenderer` nor `PlatformWindow` grows a hardware type.

## Rejected alternatives

| alternative | why not |
| --- | --- |
| the renderer as the anchor, reached through `with_renderer` | it costs an upcast to `PlatformRenderer` and a downcast in userland, so the typed part of the API is a convention rather than a signature |
| the typed trait per backend crate, as the sketch had it | not implementable: no backend crate depends on `gpui_authoring`, so none can name `Window`. Doing it would invert the layering |
| a borrowed `&dyn Any` | cannot be returned out of the closure that is the only route to a renderer |
| widening `SceneRenderer` with the accessor | the one thing the seam's §1 and the layer-stack ruling both refuse; it would put a device query in the engine's contract |
| handing the application the *platform* window instead | it would publish `Box<dyn PlatformWindow>`, whose other forty methods are none of an application's business |

## What would reopen this

- **A producer that needs the context or the queue as well, under one signature.** The Direct3D
  answer is a device and the immediate context comes from it (`GetImmediateContext`), and wgpu's
  answer is a slot holding both; if a third backend could lend neither, the shape of the payload is
  the thing to revisit, not the anchor.
- **A renderer that cannot lend, and a producer that needs one anyway.** The cross-device bridge
  ([`0002`](0002-render-extension-device-model.md)'s deferred tier) is the answer there, and it is a
  contract change — a token carrying its own synchronisation — rather than a change to this accessor.
- **The payload types becoming a published surface.** They have to be nameable for an application to
  build a token, which is a commitment of the same kind as a swap; 0001 is where this project's
  position on those lives.
