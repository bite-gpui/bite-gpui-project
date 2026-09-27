# The rendering project

A flat view of everything the render extension needs, as symlinks. The documents live where
their subject lives — the chapters in [`../rendering/`](../rendering/README.md), the
authoring surface in [`../authoring/`](../authoring/gpu-canvas.md), the device decision and
its evidence in [`decisions/`](../../decisions/README.md) — and this folder is the one place
that shows the whole set at once.

Nothing here is a document. Every entry is a symlink, so opening one opens the original and
editing one edits the original.

| here | is |
| --- | --- |
| [`overview.md`](overview.md) | the chapter order, the trade-off, and the reconciliation |
| [`renderer-seam.md`](renderer-seam.md) | the seam: `PlatformRenderer`, the typed target, the factory, recovery, and the patch set |
| [`foreign-texture.md`](foreign-texture.md) | Path A — importing a texture produced outside GPUI |
| [`inline-commands.md`](inline-commands.md) | Path B — drawing into the window's own pass |
| [`gpu-canvas.md`](gpu-canvas.md) | the authoring surface |
| [`verification.md`](verification.md) | the matrix, and the platform each check needs |
| [`device-model.md`](../../decisions/0002-render-extension-device-model.md) | decision 0002 — which devices a producer may use, and which it may not |
| [`windows-path-a-probe.md`](../../decisions/windows-path-a-probe.md) | the measurement 0002 rests on |
| [`frame-flow.md`](../../architecture/frame-flow.md) | where the seam is entered in a frame |
| [`layer-stack.md`](../../architecture/layer-stack.md) | the ruling the extension has to clear |

## Why symlinks, and why a folder

A copy would be a second place for a fact to go stale, and this repository's rule is that a
fact specified once is not restated — so nothing here is copied. A folder rather than a
list in prose, because the question this answers is "what do I have to read", and `ls` is a
better index for that than a sentence is.

**One caveat, which follows from what a relative link is: it resolves against the path a
document was *opened* by.** The six entries from `rendering/` and `authoring/` sit at this
folder's own depth — two levels down — so every link inside them resolves from here as
well. The four from `decisions/` and `architecture/` sit one level higher, so opening one
*through its symlink* leaves its own links pointing at nothing. The table above links those
four where they live; the symlinks are there for `ls`.
