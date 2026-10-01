"""Shared machinery for the citation tooling.

Where the documents and the cited source live, how a citation is parsed, and how a
`path:line` or a symbol is resolved to a line of text.

The documents live in this repository (`bite-gpui-project`); the cited source lives in
the `bite_*` branches of the `bite-gpui` clone, checked out as a parallel working copy
beside this one. Both are found by location, not configuration.
"""

from __future__ import annotations

import glob
import os
import re
import subprocess
from pathlib import Path

# The ref the documents are written against. Changing it invalidates every citation in
# the repository and means a pass over all of them.
#
# Only the bare name is recorded, because a local branch called `bite_v1.23.1-pre` does
# not always exist: in a fresh clone it is a remote-tracking branch under `origin/`.
# `resolve_ref` tries the bare name first, then the `origin/` spelling, so the tooling
# works whether the branch is checked out locally or not.
CANONICAL_REF = "bite_v1.23.1-pre"

# A citation is a source path followed by a line, or a line range. Rust is the bulk of
# them; the shaders are why the extension list is not just `rs`.
CITATION = re.compile(r"([A-Za-z0-9_][A-Za-z0-9_/.-]*\.(?:rs|wgsl|metal|hlsl)):(\d+)(?:-(\d+))?")

# Where a bare filename is looked up, in order. Every bare citation in the documents
# today is `gpui_authoring`'s; anything from another crate has to say so, because
# `window.rs` and `lib.rs` are not unique across the tree.
BARE_BASES = ("crates/gpui_authoring/src", "crates/gpui_authoring/src/app")

# A citation of the form `crate-1.2.3/src/file.rs` is a published dependency's source,
# not this repository's. It is read from the cargo registry cache.
VERSIONED = re.compile(r"^[A-Za-z0-9_.-]+-\d+\.\d+")

# The cargo registry cache. Honour CARGO_HOME: the toolchain is not always at `~/.cargo`.
CARGO_HOME = Path(os.environ.get("CARGO_HOME", Path.home() / ".cargo"))
REGISTRY = CARGO_HOME / "registry/src"

# The canonical name of the source repository (see repositories.md), checked out as a
# sibling of this one in the parallel layout.
SOURCE_REPO = "bite-gpui"

# A Rust symbol definition, by structural kind. `pattern` matches the definition line and
# captures the name in group 1. `field` is last because it is the loosest.
SYMBOL_KINDS = (
    ("fn", re.compile(r"\bfn\s+([A-Za-z_]\w*)\b")),
    ("struct", re.compile(r"\bstruct\s+([A-Za-z_]\w*)\b")),
    ("enum", re.compile(r"\benum\s+([A-Za-z_]\w*)\b")),
    ("trait", re.compile(r"\btrait\s+([A-Za-z_]\w*)\b")),
    ("type", re.compile(r"\btype\s+([A-Za-z_]\w*)\b")),
    ("const", re.compile(r"\bconst\s+([A-Za-z_]\w*)\b")),
    ("mod", re.compile(r"\bmod\s+([A-Za-z_]\w*)\b")),
    ("field", re.compile(r"^\s*(?:pub(?:\([^)]*\))?\s+)?([A-Za-z_]\w*)\s*:")),
)


def run(args: list[str]) -> subprocess.CompletedProcess:
    return subprocess.run(args, capture_output=True, text=True)


def repo_root() -> Path:
    """The git repository that owns the documents (this repository)."""
    script = Path(__file__).resolve()
    result = run(["git", "-C", str(script.parent), "rev-parse", "--show-toplevel"])
    if result.returncode == 0 and result.stdout.strip():
        return Path(result.stdout.strip())
    return script.parent.parent


def docs_root(root: Path) -> Path:
    """The directory the documents live in.

    Inside a checkout that nests the record as `.meta` it is `root/.meta`; in the
    standalone checkout it is the repository root itself.
    """
    meta = root / ".meta"
    return meta if meta.is_dir() else root


def source_root(docs: Path) -> Path:
    """The `bite-gpui` clone whose `bite_*` branches carry the cited source.

    In the nested layout the record is a child of the clone, so the clone is the
    documents' parent; in the parallel layout the clone is a sibling named `SOURCE_REPO`.
    Tell them apart by the `crates/` tree only the clone has.
    """
    for candidate in (docs.parent / SOURCE_REPO, docs.parent):
        if (candidate / "crates").is_dir():
            return candidate
    return docs  # fall back; ref resolution will report the refs as unresolved


def resolve_ref(root: Path, named: str) -> str:
    """Return a git ref that resolves, given the bare name.

    A `bite_*` branch is a remote-tracking branch in a fresh clone, so the bare name
    resolves only after someone checks it out locally. Try the bare name first, then the
    `origin/` spelling; `git show <ref>:<path>` works for either. A name that resolves to
    nothing at all is returned unchanged so the caller still names what the user wrote
    rather than what was guessed.
    """
    if run(["git", "-C", str(root), "rev-parse", "--verify", "--quiet", named]).returncode == 0:
        return named
    remote = f"origin/{named}"
    if run(["git", "-C", str(root), "rev-parse", "--verify", "--quiet", remote]).returncode == 0:
        return remote
    return named


def exists_at(root: Path, ref: str | None, path: str) -> bool:
    if ref is None:
        return (root / path).is_file()
    result = run(["git", "-C", str(root), "cat-file", "-e", f"{ref}:{path}"])
    return result.returncode == 0


def read_at(root: Path, ref: str | None, path: str) -> list[str] | None:
    if ref is None:
        file = root / path
        return file.read_text().splitlines() if file.is_file() else None
    result = run(["git", "-C", str(root), "show", f"{ref}:{path}"])
    return result.stdout.splitlines() if result.returncode == 0 else None


def read_registry(path: str) -> list[str] | None:
    matches = sorted(glob.glob(str(REGISTRY / "*" / path)))
    if not matches:
        return None
    return Path(matches[-1]).read_text().splitlines()


def resolve(root: Path, ref: str | None, cited: str) -> tuple[str, str]:
    """Return (kind, path): 'repo', 'registry', or 'unresolved'."""
    head = cited.split("/", 1)[0]
    if "/" in cited:
        if VERSIONED.match(head):
            return "registry", cited
        return ("repo", cited) if exists_at(root, ref, cited) else ("unresolved", cited)
    for base in BARE_BASES:
        candidate = f"{base}/{cited}"
        if exists_at(root, ref, candidate):
            return "repo", candidate
    return "unresolved", cited


def find_symbols(content: list[str], name: str) -> list[tuple[int, str]]:
    """Return the (line, kind) of every definition of `name` in `content`."""
    found = []
    for number, line in enumerate(content, start=1):
        for kind, pattern in SYMBOL_KINDS:
            match = pattern.search(line)
            if match and match.group(1) == name:
                found.append((number, kind))
                break
    return found


def symbols_in_text(text: str) -> list[str]:
    """Backticked identifiers in a prose line, as candidate symbol names.

    `Window::dispatch_event` becomes `dispatch_event`: a citation names the method, not
    the `impl` it sits in.
    """
    return [ident.split("::")[-1] for ident in re.findall(r"`([A-Za-z_]\w*(?:::[A-Za-z_]\w*)*)`", text)]
