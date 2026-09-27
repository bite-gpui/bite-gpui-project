# `GpuCanvas`: the authoring surface

- **Status:** proposed. Nothing of it is implemented.
- **Assumes:** [`foreign-texture.md`](foreign-texture.md) and
  [`inline-commands.md`](inline-commands.md) — the primitive and the two `Window` hooks
  it is pushed through — and, under them, [`renderer-seam.md`](renderer-seam.md).
- **Target crates:** `gpui_authoring` (the element and its builder), re-exported by the
  `gpui` facade. It reads the scene primitive, so it also touches `gpui_engine`.
- **What it is:** the end-user surface. The seam's primitive is a scene type; the person
  who wants a map inside a `div()` should never meet the `Element` trait, and this is the
  shape that keeps them from meeting it.

## 1. The cost it absorbs

The sketch this replaces implements `Element` from scratch. In this fork that is more
than the three lifecycle methods it names. `Element: 'static + IntoElement`
(`crates/gpui_authoring/src/element.rs:55`) also requires `id()` and `source_location()`,
and the lifecycle is `request_layout(id, window, cx) -> (LayoutId, State)`,
`prepaint(…) -> PrepaintState` and `paint(…)`, each taking `&mut Window` and `&mut App`
(`crates/gpui_authoring/src/element.rs:80-108`).

Two of the costs are worth separating, because only one is real:

- **Style is free.** Implementing `Styled` over a `StyleRefinement`
  (`crates/gpui_authoring/src/styled.rs:26`) brings the whole generated surface with it —
  `.size_full()`, `.rounded_xl()`, `.relative()`, `.p_4()` — because those are produced by
  the `size` and `rounded` style macros
  (`crates/gpui_macros/src/styles.rs:858`, `:1164`). A custom element does not
  reimplement them.
- **Interaction is not.** `on_mouse_down`, `on_mouse_move` and `on_scroll_wheel` are
  trait methods on `InteractiveElement`
  (`crates/gpui_authoring/src/elements/div.rs:869`, `:1000`, `:1054`) — but calling them
  only stores listeners in an `Interactivity`
  (`crates/gpui_authoring/src/elements/div.rs:2117`). **The hitbox that makes them fire
  is registered in `Div`'s own prepaint.** An element that reimplements the lifecycle
  therefore silently loses mouse and scroll routing, which is the boilerplate worth
  removing rather than the layout plumbing.

## 2. The precedent already in the tree: `canvas`

`crates/gpui_authoring/src/elements/canvas.rs:10` is exactly "the low-level paint API
without defining a whole custom element": a `Canvas<T>` (`:23`) taking a prepaint and a
paint callback, already `Styled`, already consuming its callbacks with `FnOnce` and
`.take()` so it is single-use per frame, and already exported
(`crates/gpui_authoring/src/elements/mod.rs:18`). `GpuCanvas` belongs **beside `canvas`
and on `div`**, not as a hand-written `Element`.

## 3. The design: compose a `div()`, delegate, then paint

`GpuCanvas` holds a `Div`, forwards the traits that make it behave like any other box,
and pushes the primitive after the box has painted:

```rust
pub struct GpuCanvas {
    div: Div,
    mode: Option<GpuRenderMode>,
    flip_y: bool,
    corner_radii: Corners<Pixels>,
}

enum GpuRenderMode {
    Texture(Box<dyn FnOnce(Bounds<Pixels>, &mut Window, &mut App) -> ImportedTextureHandle>),
    Inline(Box<dyn FnOnce(Bounds<Pixels>, &mut TransactionalDrawContext)>),
}
```

Four things follow from that shape, and each is the reason for it:

1. **The box paints first, the primitive second.** Delegating to `Div::paint` and then
   pushing means the canvas's own background and border sit under the GPU content, and
   any sibling later in the parent's children list paints over it — which is how a
   floating control stack on top of a map works at all.
2. **Radii travel on the primitive, not through clipping.** A scene primitive is not a
   child and cannot be clipped by `.overflow_hidden()`, so the primitive carries `radii`
   — which is why `corner_radii` is a field here.
3. **`Styled` and `InteractiveElement` are delegation, not reimplementation** — two
   three-line impls, because `Div::style()` already returns
   `&mut interactivity.base_style` (`crates/gpui_authoring/src/elements/div.rs:1865`,
   `:1871`). The from-scratch element would have to reproduce both and would still lose
   the hitbox.
4. **The callbacks are `FnOnce` and `take()`n**, matching `Canvas` — the element tree is
   rebuilt every frame, so a mode is consumed exactly once.

`Element` is then implemented on `GpuCanvas` by forwarding each of the five required
methods to the inner `Div`, with `RequestLayoutState` and `PrepaintState` bound to
`<Div as Element>`'s. That is what makes `.id()` work: it returns
`Stateful<GpuCanvas>`, which is an element only because `GpuCanvas: Element`
(`crates/gpui_authoring/src/elements/div.rs:4078`, `:4143`), and it is another reason the
delegation is the design.

## 4. The public API

```rust
use gpui::{GpuCanvas, px};

GpuCanvas::new()
    .size_full()            // Styled, delegated
    .rounded_xl()
    .flip_y(true)           // whether the producer's UVs are inverted
    .on_render_texture(move |bounds, window, cx| {
        // Path A: an offscreen VRAM surface — wgpu passes, a decoder, a camera
        engine.render_frame(bounds.size).to_imported_handle()
    })
```

and the inline alternative on the same builder:

```rust
GpuCanvas::new()
    .size_full()
    .on_render_inline(move |bounds, draw| {
        // Path B: commands execute in GPUI's own pass
        my_vector_map.draw_tiles(draw.encoder(), bounds);
    })
```

`on_click` and `on_hover` come from `StatefulInteractiveElement`
(`crates/gpui_authoring/src/elements/div.rs:1584`, `:1655`) and therefore need `.id()`.
`on_mouse_down`, `on_mouse_move` and `on_scroll_wheel` are on `InteractiveElement` and
need none. Both work because of the delegation in §3.

## 5. Names that do not exist here

Every sketch of this component was written against upstream, and four of its names have
no counterpart in this fork:

- **No `WindowContext` and no `ViewContext`.** Element and paint callbacks take
  `(…, window: &mut Window, cx: &mut App)`; a view's `render` takes
  `(&mut self, window: &mut Window, cx: &mut Context<Self>)`
  (`crates/gpui_authoring/src/element.rs:164`, `:180`).
- **Painting is a `Window` capability, not a `cx` one.** `paint_quad`
  (`crates/gpui_authoring/src/window.rs:4972`) and `paint_image` (`:5365`) both insert
  into the frame's scene, so the hooks are `window.paint_imported_texture` and
  `window.paint_with_callback`, beside them in the same phase. The drafts' separate
  `register_foreign_texture` step between them is gone — it existed to name the texture for
  a registry, and there is no registry
  ([`foreign-texture.md`](foreign-texture.md) §3).
- **`Styled::style` returns `&mut StyleRefinement`**
  (`crates/gpui_authoring/src/styled.rs:26`), not `&mut Style`.
- **The radii type is `Corners<Pixels>`** (`crates/gpui_types/src/geometry.rs:2235`,
  `Corners::all` at `:2274`), not a `CornerRadii` with four named `f32`s.

## 6. Where it lives, and what it does not change

`GpuCanvas` and `GpuRenderMode` belong in `gpui_authoring` beside `canvas`, re-exported
by the `gpui` facade, so a consumer writes `use gpui::GpuCanvas`. It adds no seam and
changes no existing trait: it is a component built from `div`, `canvas`'s pattern, and
the two `Window` hooks the render extension adds. The renderer it draws through is the
one [`renderer-seam.md`](renderer-seam.md) makes installable, and `GpuCanvas` neither
chooses it nor knows its name — which is why a canvas cannot be used at all unless the
window's renderer implements the path it asks for.

## 7. What would reopen it

- **If the primitive has to interleave *within* one box's own children** — a GPU layer
  with UI both under and over it inside a single box — delegation-to-`Div` is the wrong
  shape and the painter has to become a real child, which brings back the sizing and
  clipping questions the delegation avoids.
- **If a renderer cannot honour per-primitive radii**, rounded GPU content would need the
  box to clip and the primitive to be a child after all.
- **If a second canvas kind appears** (audio waveform, 3D viewport) with different sizing
  or input semantics, the builder should be split rather than grow a mode enum per
  variant.
