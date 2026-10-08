# Evidence: the interop row that never started

- **Evidence for** [decision 0005](0005-external-rendering-unifies-under-surface.md) — the
  cross-device arm it commits to — and for the gate in §3 of
  [`../spi/rendering/verification.md`](../spi/rendering/verification.md). Kept as evidence: what the
  design and the gate make of the answer is that section.
- **The question.** Why did `cargo test -p gpui_interop a_shared_d3d12_surface` die on
  `windows-latest` with `0xc0000139 STATUS_ENTRYPOINT_NOT_FOUND` and **no test output at all**, in
  both the debug and the release profile, while the `gpui_windows` rows on the same runner passed?
  **Answered: the test binary imports `TaskDialogIndirect`, and a binary without an application
  manifest gets the common-controls *5.82* `comctl32.dll`, which does not export it.** Nothing to do
  with Direct3D.
- **What it was not.** Not `d3d12.dll` — the runner has it, it exports `D3D12CreateDevice`, and
  `D3D12Core.dll` sits beside it. Not the profile — debug and release failed identically. Not
  `d3dcompiler_47.dll`. The loader names no symbol, so each was ruled out by measurement rather than
  by argument, and the two readings before the answer — a missing Direct3D 12 runtime, then the
  run-time shader compiler — were both wrong.
- **The fix** is on `bite_v1.23.1-pre-interop` (PR #11): the crate embeds its own manifest
  (`crates/gpui_interop/build.rs`, `crates/gpui_interop/resources/windows/`), and the row now refuses
  a skip. Citations to those files wait for the canonical ref to advance, so they are named here
  without line numbers.

## 1. The import that cannot resolve

`TaskDialogIndirect` is the one symbol in the test binary's import table that no System32 DLL
exports, and it arrives through the window rather than through the test: the test opens a real
`WindowsWindow`, whose `prompt` calls it (`crates/gpui_windows/src/window.rs:745`,
`crates/gpui_windows/src/window.rs:815`). The symbol lives only in common-controls **6**, which a
binary reaches through an SxS assembly it asks for in its **manifest**; without one the loader binds
the 5.82 `comctl32.dll` already in System32, the import does not resolve, and the process is killed
before `main`. That is why the test's own skip — the guard that exists for a machine with no
Direct3D 12 device — never got a chance to run.

`gpui` already carries exactly this manifest. `crates/gpui/resources/windows/gpui.rc:2` names it as
`RT_MANIFEST` 1, `crates/gpui/resources/windows/gpui.manifest.xml` holds the
`Microsoft.Windows.Common-Controls` 6.0.0.0 dependency, and `gpui`'s build script embeds it
(`crates/gpui/build.rs:15`, `crates/gpui/build.rs:21`). What it does **not** do is reach a downstream
binary. `gpui` has no bin target, so `embed-resource` takes its no-bins branch
(`embed-resource-3.0.6/src/lib.rs:445-449`) and asks for a `rustc-link-lib` — at a `.res` filed under
a `.lib` name (`embed-resource-3.0.6/src/windows_msvc.rs:32`). Measured, not inferred: the test
binary's own bytes carry no `Microsoft.Windows.Common-Controls`, while the `gpui_windows` test
binary — which never links the window's prompt — carries no `TaskDialogIndirect` either, and so loads
with or without a manifest.

## 2. How it was found

The loader reports the *fact* and not the symbol, so the diagnosis was mechanical:

- **Load the failing binary into a process that already has a manifest.** `.NET`'s
  `NativeLibrary.Load` starts it; running it alone does not. Two loaders disagreeing about one
  import table is the hint that a manifest is the difference.
- **Compare every imported symbol against the System32 DLL that owns it**, `dumpbin /imports` against
  `dumpbin /exports`, minus the `api-ms-win-*` sets that resolve elsewhere. Exactly one misses:
  `comctl32.dll ! TaskDialogIndirect`.
- **Read the manifest out of the binary's bytes.** Absent — while the WinSxS 6.0 assembly is present.

## 3. What it changed

- **The crate embeds a manifest of its own**, in `gpui`'s resource layout but through
  `embed-resource`'s `compile_for_everything` (`embed-resource-3.0.6/src/lib.rs:539`) rather than its
  `compile`, because a test target is not a bin: the bin-only helper is exactly what reaches `gpui`'s
  own targets and not a downstream test binary.
- **The row refuses a skip.** It runs the test with `-- --nocapture`, which is what lets the test's
  skip message past libtest's capture, and fails when that message is there. A hosted Windows runner
  always has WARP, so a skip now means the runner or the test regressed rather than that the hardware
  is absent; §3 of [`../spi/rendering/verification.md`](../spi/rendering/verification.md) is the gate
  itself.
