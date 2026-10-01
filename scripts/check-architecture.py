#!/usr/bin/env python3
"""Check enforceable model/backend/renderer ownership; no third-party modules."""

from __future__ import annotations

import argparse
from pathlib import Path
import re
import sys
import tempfile
import tomllib
import unittest


ROOT = Path(__file__).resolve().parents[1]


def rust_code(source: str) -> str:
    """Ignore comments/strings while preserving offsets and source line numbers."""
    return re.sub(
        r'//[^\n]*|/\*.*?\*/|"(?:\\.|[^"\\])*"',
        lambda match: re.sub(r"[^\n]", " ", match.group()),
        source,
        flags=re.S,
    )


def production_code(source: str) -> str:
    code = rust_code(source)
    pattern = re.compile(r"#\s*\[\s*cfg\s*\(\s*(?:all\s*\(\s*)?test\b[^\]]*\]\s*mod\s+\w+\s*\{")
    for match in reversed(list(pattern.finditer(code))):
        depth = 1
        end = match.end()
        while end < len(code) and depth:
            depth += (code[end] == "{") - (code[end] == "}")
            end += 1
        code = code[:match.start()] + re.sub(r"[^\n]", " ", code[match.start():end]) + code[end:]
    return code


def check(root: Path) -> list[str]:
    errors = []
    manifest_path = root / "crates/pace-model/Cargo.toml"
    if not manifest_path.is_file():
        return ["Missing pure model manifest"]
    manifest = tomllib.loads(manifest_path.read_text())

    def dependencies(table: dict, context: str = "") -> None:
        for key, value in table.items():
            if key in {"dependencies", "build-dependencies", "dev-dependencies"}:
                allowed = {"serde", "serde_json"} if key == "dev-dependencies" else {"serde"}
                for name, options in value.items():
                    package = options.get("package", name) if isinstance(options, dict) else name
                    if package not in allowed:
                        errors.append(f"pace-model {context}{key}: forbidden dependency {package}")
            elif isinstance(value, dict):
                dependencies(value, context + key + ".")

    dependencies(manifest)

    def reject(path: Path, code: str, pattern: str, message: str) -> None:
        for match in re.finditer(pattern, code):
            line = code.count("\n", 0, match.start()) + 1
            errors.append(f"{path.relative_to(root)}:{line}: {message}")

    for path in (root / "crates/pace-model/src").rglob("*.rs"):
        reject(path, rust_code(path.read_text()), r"\b(?:eframe|egui|terminal_core|alacritty_terminal|portable_pty|fs|process|thread)\b", "pure model imports a runtime/backend capability")
    for path in (root / "src").rglob("*.rs"):
        code = production_code(path.read_text())
        reject(path, code, r"\b(?:alacritty_terminal|portable_pty)\b", "desktop imports terminal backend internals")
        if "runtime" not in path.relative_to(root / "src").parts:
            reject(path, code, r"\.\s*lock\s*\(", "UI/persistence must not receive or lock live terminal state")
        if "terminal_view" in path.relative_to(root / "src").parts:
            reject(path, code, r"\b(?:TerminalSession|SessionOptions|MutexGuard)\b", "renderer receives a live session or mutex guard")
    for path in (root / "crates/terminal-core/src").rglob("*.rs"):
        reject(path, production_code(path.read_text()), r"pub\s+fn\s+lock\s*\(", "terminal-core exposes a mutable backend lock")
    return errors


class ArchitectureTests(unittest.TestCase):
    def scaffold(self, root: Path) -> None:
        (root / "crates/pace-model/src").mkdir(parents=True)
        (root / "src/terminal_view").mkdir(parents=True)
        (root / "crates/terminal-core/src").mkdir(parents=True)
        (root / "crates/pace-model/Cargo.toml").write_text('[package]\nname="pace-model"\n[dependencies]\nserde="1"\n')

    def test_rejects_backend_dependency_hidden_in_target_table(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            self.scaffold(root)
            with (root / "crates/pace-model/Cargo.toml").open("a") as file:
                file.write('[target.\'cfg(unix)\'.dependencies]\nengine={package="alacritty_terminal",version="1"}\n')
            self.assertTrue(any("forbidden dependency alacritty_terminal" in error for error in check(root)))

    def test_rejects_live_session_in_renderer_but_ignores_fixture_and_comments(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            self.scaffold(root)
            path = root / "src/terminal_view/paint.rs"
            path.write_text('// TerminalSession\n#[cfg(test)]\nmod tests { use terminal_core::TerminalSession; }\n')
            self.assertEqual(check(root), [])
            path.write_text('fn paint(session: &terminal_core::TerminalSession) {}\n')
            self.assertTrue(any("renderer receives" in error for error in check(root)))

    def test_rejects_desktop_backend_import_and_terminal_lock(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            self.scaffold(root)
            (root / "src/app.rs").write_text('use terminal_core::alacritty_terminal;\nfn draw() { session.lock(); }\n')
            errors = check(root)
            self.assertTrue(any("backend internals" in error for error in errors))
            self.assertTrue(any("live terminal state" in error for error in errors))


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--self-test", action="store_true")
    args = parser.parse_args()
    if args.self_test:
        result = unittest.TextTestRunner(verbosity=2).run(unittest.defaultTestLoader.loadTestsFromTestCase(ArchitectureTests))
        return 0 if result.wasSuccessful() else 1
    errors = check(ROOT)
    if errors:
        print("\n".join(errors), file=sys.stderr)
        return 1
    print("Architecture boundaries passed: pure model, sealed backend, snapshot-only renderer")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
