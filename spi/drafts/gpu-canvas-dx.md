# The GpuCanvas authoring surface

- **Target crates:** `gpui_authoring` (the element and its builder), re-exported by the
  `gpui` facade. It reads the scene primitive the render RFC adds, so it also touches
  `gpui_engine`.
- **Status:** proposed. It is the DX half of the render extension and cannot be built
  before its two prerequisites:
  [`scene-renderer-seam.md`](scene-renderer-seam.md) (a renderer is installable at all)
  and [`dual-path-render-extension.md`](dual-path-render-extension.md) (the
  `CustomRenderPrimitive` and the two hooks). Nothing here exists yet.
- **What it is:** the end-user surface. The RFC's hook is a scene *primitive*; the
  person who wants a map inside a `div()` should never meet the `Element` trait, and
  this document is the shape that keeps them from meeting it.

## The cost the DX is absorbing

The sketch this replaces implements `Element` from scratch. In this fork that is more
than the four hooks it names. `Element: 'static + IntoElement`
(`crates/gpui_authoring/src/element.rs:55`) requires, in full: two associated state
types, **`id()` and `source_location()`** as required methods, and the three lifecycle
methods — `request_layout(id, window, cx) -> (LayoutId, State)`,
`prepaint(...) -> PrepaintState`, `paint(...)`, each taking `&mut Window` and `&mut App`
(`crates/gpui_authoring/src/element.rs:80-108`).

Two of those are cheap to pay and one is not:

- **Style is free.** Implementing `Styled` over a `StyleRefinement`
  (`crates/gpui_authoring/src/styled.rs:26`) brings the whole generated surface with
  it — `.size_full()`, `.rounded_xl()`, `.relative()`, `.p_4()` — because those are
  produced by the `size` and `rounded` style macros
  (`crates/gpui_macros/src/styles.rs:858`, `:1164`). A custom element does not
  reimplement them.
- **Interaction is not.** `on_mouse_down`, `on_mouse_move` and `on_scroll_wheel` are
  trait methods on `InteractiveElement` (`crates/gpui_authoring/src/elements/div.rs:768`,
  at `:869`, `:1000`, `:1054`) — but calling them only stores listeners in an
  `Interactivity` (`crates/gpui_authoring/src/elements/div.rs:2117`). The **hitbox that
  makes them fire is registered in `Div`'s own prepaint**, not by implementing
  `Element`. An element that reimplements the lifecycle therefore silently loses mouse
  and scroll routing unless it reproduces that registration. This — not the layout
  plumbing — is the boilerplate worth removing.

## The precedent already in the tree: `canvas`

`crates/gpui_authoring/src/elements/canvas.rs:10` is exactly "the low-level paint API
without defining a whole custom element": a `Canvas<T>` (`:23`) taking a prepaint and a
paint callback, already `Styled`, already consuming its callbacks with `FnOnce` and
`.take()` so it is single-use per frame, and already exported
(`crates/gpui_authoring/src/elements/mod.rs:18`). `GpuCanvas` should be built **on
`canvas` and `div`**, not beside them. The RFC's error is proposing a new hand-written
`Element` for something the tree already has a composition point for.

## The design: compose a `div()`, delegate, then paint

`GpuCanvas` holds a `Div` and forwards the three traits that make it behave like any
other box, then pushes the primitive after the box has painted:

```rust
// gpui_authoring, beside `canvas`
pub struct GpuCanvas {
    div: Div,
    mode: Option<GpuRenderMode>,
    flip_y: bool,
    corner_radii: Corners<Pixels>,
}

enum GpuRenderMode {
    Texture(Box<dyn FnOnce(Bounds<Pixels>, &mut Window, &mut App) -> ForeignTextureHandle>),
    Inline(Box<dyn FnOnce(Bounds<Pixels>, &mut TransactionalDrawContext)>),
}

impl Styled for GpuCanvas {
    fn style(&mut self) -> &mut StyleRefinement {
        self.div.style()
    }
}

impl InteractiveElement for GpuCanvas {
    fn interactivity(&mut self) -> &mut Interactivity {
        self.div.interactivity()
    }
}

impl Element for GpuCanvas {
    type RequestLayoutState = <Div as Element>::RequestLayoutState;
    type PrepaintState = <Div as Element>::PrepaintState;

    fn id(&self) -> Option<ElementId> { self.div.id() }
    fn source_location(&self) -> Option<&'static Location<'static>> {
        self.div.source_location()
    }
    fn request_layout(&mut self, id, window, cx) -> (LayoutId, Self::RequestLayoutState) {
        self.div.request_layout(id, window, cx)
    }
    fn prepaint(&mut self, id, bounds, state, window, cx) -> Self::PrepaintState {
        self.div.prepaint(id, bounds, state, window, cx)
    }
    fn paint(&mut self, id, bounds, state, prepaint, window, cx) {
        Element::paint(&mut self.div, id, bounds, state, prepaint, window, cx);
        match self.mode.take() {
            Some(GpuRenderMode::Texture(f)) => {
                let id = window.register_foreign_texture(f(bounds, window, cx));
                window.paint_custom_primitive(CustomRenderPrimitive::Texture {
                    id, bounds, radii: self.corner_radii, opacity: 1.0, flip_v: self.flip_y,
                });
            }
            Some(GpuRenderMode::Inline(f)) => {
                window.paint_with_callback(bounds, f);
            }
            None => {}
        }
    }
}
```

Four things follow from that shape, and each is the reason for it:

1. **The box paints first, the primitive second.** Delegating to `Div::paint` and then
   pushing means the border and background sit under the GPU content, and any sibling
   later in the parent's children list paints over it — which is how the sketch's
   floating controls stack on top at all.
2. **Radii travel on the primitive, not through clipping.** The primitive carries
   `radii` (the RFC gives it) because a scene primitive is not a child and cannot be
   clipped by `.overflow_hidden()`. That is also why `corner_radii` is a field here.
3. **`Styled` and `InteractiveElement` are delegation, not reimplementation** — two
   three-line impls, because `Div`'s `style()` already returns
   `&mut interactivity.base_style` (`crates/gpui_authoring/src/elements/div.rs:1865`,
   `:1871`). The from-scratch `Element` in the sketch would have to reproduce both, and
   would still lose the hitbox.
4. **The callbacks are `FnOnce` and `take()`n**, matching `Canvas` — the element tree
   is rebuilt every frame, so a mode is consumed exactly once.

## The public API

```rust
use gpui::{CornerRadii, GpuCanvas, MouseButton, px}; // CornerRadii = Corners<Pixels>, see below

GpuCanvas::new()
    .size_full()           // Styled, delegated
    .rounded_xl()
    .flip_y(true)          // corrects OpenGL/WGPU vs GPUI UV inversion
    .on_render_texture(move |bounds, window, cx| {
        // Path A — offscreen VRAM surface: wgpu passes, a video decoder, a camera
        engine.render_frame(bounds.size).to_foreign_handle()
    })
```

and the inline alternative on the same builder:

```rust
GpuCanvas::new()
    .size_full()
    .on_render_inline(move |bounds, draw| {
        // Path B — commands execute on GPUI's active swapchain pass
        my_vector_map.draw_tiles(draw.encoder(), bounds);
    })
```

`flip_y` is the DX's one genuinely new idea and it belongs on the primitive (`flip_v`
in the RFC), not in the element: the element never sees a texture, so it can only pass
the flag down.

## The corrections the sketch needs

Four, each a real difference from upstream:

1. **There is no `WindowContext`.** Element and paint callbacks take
   `(…, window: &mut Window, cx: &mut App)`; views take `&mut Context<Self>` — e.g.
   `fn render(&mut self, window: &mut Window, cx: &mut Context<Self>)`
   (`crates/gpui_authoring/src/element.rs:164`). The sketch's `cx: &mut WindowContext`
   and `ViewContext<Self>` do not exist in this fork.
2. **The primitive is pushed on `Window`, not `cx`.** Painting is a window capability:
   `paint_quad` (`crates/gpui_authoring/src/window.rs:4972`) and `paint_image`
   (`:5365`) both insert into the frame's scene. The RFC's `cx.register_foreign_texture`
   and `cx.paint_with_callback` are wrong for this tree; they are
   `window.register_foreign_texture` and `window.paint_with_callback`, sitting beside
   `paint_quad` in the same paint phase.
3. **`Styled::style` returns `&mut StyleRefinement`** (`crates/gpui_authoring/src/styled.rs:26`),
   not `&mut Style` as the sketch writes.
4. **The radii type is `Corners<Pixels>`** (`crates/gpui_types/src/geometry.rs:2235`,
   `Corners::all` at `:2274`), not a `CornerRadii` struct with four named `f32`s.

## Interaction and gestures

The sketch's handlers are right, and they work **because of the delegation above**:

```rust
GpuCanvas::new()
    .size_full()
    .on_render_inline(move |bounds, draw| map.render(draw, bounds))
    .on_mouse_down(MouseButton::Left, cx.listener(|this, event: &MouseDownEvent, window, cx| {
        this.drag_start = Some(event.position);
    }))
    .on_mouse_move(cx.listener(|this, event: &MouseMoveEvent, _window, cx| {
        if let Some(start) = this.drag_start {
            this.map.pan(event.position - start);
            cx.notify(); // rebuilds the tree at the display's rate
        }
    }))
    .on_scroll_wheel(cx.listener(|this, event: &ScrollWheelEvent, _window, cx| {
        this.map.zoom(event.delta);
        cx.notify();
    }))
```

The one thing to know is which tier each hook is in. `on_mouse_down`, `on_mouse_move`
and `on_scroll_wheel` are on `InteractiveElement`
(`crates/gpui_authoring/src/elements/div.rs:869`, `:1000`, `:1054`) and need no id.
`on_click` and `on_hover` are on `StatefulInteractiveElement`
(`crates/gpui_authoring/src/elements/div.rs:1584`, `:1655`) and need `.id()`, which
returns `Stateful<GpuCanvas>`. That is itself an element only because `GpuCanvas:
Element` (`crates/gpui_authoring/src/elements/div.rs:4078`, `:4143`) — another reason
the delegation is the design and the from-scratch element is not.

## Where it lives, and what it does not change

`GpuCanvas` and `GpuRenderMode` belong in `gpui_authoring` beside `canvas`, re-exported
by the `gpui` facade, so a consumer writes `use gpui::GpuCanvas`. It adds no seam and
changes no existing trait: it is a component built out of `div`, `canvas`'s pattern, and
the two `Window` hooks the render RFC adds. The renderer it renders through is the one
[`scene-renderer-seam.md`](scene-renderer-seam.md) makes installable; `GpuCanvas` neither
chooses it nor knows its name.

## What would reopen it

- **If the primitive must interleave between children** rather than sit under them
  (a GPU layer with UI both under and over it inside one box), delegation-to-`Div` is
  the wrong shape and the painter has to become a real child, which brings the sizing
  and clipping questions the delegation avoids.
- **If a renderer cannot honour per-primitive radii**, rounded GPU content would need
  the box to clip (`overflow_hidden`) and the primitive to be a child after all.
- **If a second canvas kind appears** (audio waveform, 3D viewport) with different
  sizing or input semantics, the builder should be split rather than grown a mode enum
  per variant.
