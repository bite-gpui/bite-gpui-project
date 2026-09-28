# The rendering project

Everything the render extension needs, at one path, as symlinks. The documents live where
their subject lives — the chapters in [`../rendering/`](../rendering/README.md), the
authoring surface in [`../authoring/`](../authoring/gpu-canvas.md), the device decision and
its evidence in [`decisions/`](../../decisions/README.md), the layer and frame views in
[`architecture/`](../../architecture/README.md) — and this folder is the one place that
shows the whole set at once.

Nothing here is a document. Every entry is a symlink, so opening one opens the original and
editing one edits the original.

The subfolders mirror the sources — `rendering/`, `authoring/`, `decisions/`,
`architecture/` — so a path here reads as the path there with a prefix, and `ls -R` answers
"what do I have to read" without prose.

| here | is |
| --- | --- |
| [`rendering/README.md`](../rendering/README.md) | the chapter order, the trade-off, and the reconciliation |
| [`rendering/renderer-seam.md`](../rendering/renderer-seam.md) | the seam: `PlatformRenderer`, the typed target, the factory, recovery, and the implementation order |
| [`rendering/foreign-texture.md`](../rendering/foreign-texture.md) | Path A — importing a texture produced outside GPUI |
| [`rendering/inline-commands.md`](../rendering/inline-commands.md) | Path B — drawing into the window's own pass |
| [`rendering/verification.md`](../rendering/verification.md) | the matrix, and the platform each check needs |
| [`authoring/gpu-canvas.md`](../authoring/gpu-canvas.md) | the authoring surface |
| [`decisions/0002-render-extension-device-model.md`](../../decisions/0002-render-extension-device-model.md) | decision 0002 — which devices a producer may use, and which it may not |
| [`decisions/windows-path-a-probe.md`](../../decisions/windows-path-a-probe.md) | the first measurement 0002 rests on |
| [`decisions/shared-surface.md`](../../decisions/shared-surface.md) | the shared-buffer probe that reopens 0002 — Windows adoption and the macOS pool, token and fence measured; macOS adoption open |
| [`decisions/windows-presentation-probe.md`](../../decisions/windows-presentation-probe.md) | the measurement 0002's Windows clause rests on |
| [`decisions/macos-presentation-probe.md`](../../decisions/macos-presentation-probe.md) | the measurement §6's macOS commit and §10 rest on |
| [`architecture/frame-flow.md`](../../architecture/frame-flow.md) | where the seam is entered in a frame |
| [`architecture/layer-stack.md`](../../architecture/layer-stack.md) | the ruling the extension has to clear |

## Why symlinks, and why folders

A copy would be a second place for a fact to go stale, and this repository's rule is that a
fact specified once is not restated — so nothing here is copied. Folders rather than a flat
list, because the table above is the index and `ls -R` should agree with it.

**One caveat, which follows from what a relative link is: it resolves against the path a
document was *opened* by, and every symlink here sits one level deeper than its source.**
Opened at its real path a document resolves all of its own links; opened *through* its
symlink, only links into the same group survive — the rendering chapters and
`authoring/gpu-canvas.md` still reach each other, because those subtrees move together —
while a link into `decisions/`, `architecture/` or `spi/` lands one level off. That is why
the table links every document where it lives rather than through the entry beside it; the
symlinks are for `ls`.
