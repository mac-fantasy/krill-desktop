# /// script
# requires-python = ">=3.10"
# dependencies = ["tree-sitter==0.26.0", "tree-sitter-swift==0.7.4", "tree-sitter-bash==0.25.1"]
# ///
"""Cross-platform syntax/manifest check; this does NOT compile or type-check Swift."""
from __future__ import annotations

import json
import plistlib
from pathlib import Path

from tree_sitter import Language, Parser
import tree_sitter_bash
import tree_sitter_swift


def main() -> None:
    root = Path(__file__).resolve().parents[2]
    problems: list[str] = []
    counts: dict[str, int] = {}
    for name, language, files in [
        ("swift", tree_sitter_swift, sorted((root / "apps/macos").rglob("*.swift"))),
        ("bash", tree_sitter_bash, sorted((root / "scripts/macos").glob("*.sh"))),
    ]:
        parser = Parser(Language(language.language()))
        counts[name] = len(files)
        for file in files:
            tree = parser.parse(file.read_bytes())
            stack = [tree.root_node]
            while stack:
                node = stack.pop()
                if node.is_error or node.is_missing:
                    problems.append(f"{file.relative_to(root)}:{node.start_point.row + 1}:{node.start_point.column + 1}: {node.type}")
                stack.extend(reversed(node.children))

    with (root / "apps/macos/Support/Info.plist").open("rb") as file:
        info = plistlib.load(file)
    assert info["LSUIElement"] is True
    assert info["CFBundleExecutable"] == "KrillMac"
    assert info["LSMinimumSystemVersion"] == "14.0"
    assert (root / "apps/macos/Sources/KrillCore").is_dir()
    assert (root / "apps/macos/Sources/KrillMac").is_dir()
    for file in (root / "contracts/fixtures").glob("*.json"):
        assert json.loads(file.read_text(encoding="utf-8"))["success"] is True

    report = {
        "success": not problems,
        "scope": "syntax_trees_plist_and_synthetic_fixtures_only",
        "swift_files": counts["swift"],
        "shell_files": counts["bash"],
        "apple_swift_compiler_executed": False,
        "apple_sdk_typechecking_executed": False,
        "macos_ui_executed": False,
        "errors": problems,
    }
    output = root / "artifacts/macos-source-validation.json"
    output.parent.mkdir(parents=True, exist_ok=True)
    output.write_text(json.dumps(report, indent=2) + "\n", encoding="utf-8")
    print(json.dumps(report, indent=2))
    if problems:
        raise SystemExit(1)


if __name__ == "__main__":
    main()
