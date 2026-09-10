#!/usr/bin/env python3
"""Dependency-free structural verification for APC (Architecture Principles and Conventions)."""

from __future__ import annotations

import re
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
errors: list[str] = []


def read(relative: str) -> str:
    path = ROOT / relative
    if not path.is_file():
        errors.append(f"missing: {relative}")
        return ""
    return path.read_text(encoding="utf-8").replace("\r\n", "\n")


def check_entries() -> None:
    groups = {
        "AGENTS": ("AGENTS.md",),
        "Claude": ("CLAUDE.md",),
        "Gemini": ("GEMINI.md",),
        "Cursor": (
            ".cursor/rules/core.mdc",
            ".cursor/rules/workflow.mdc",
        ),
        "Copilot": (".github/copilot-instructions.md",),
        "Cline": (".clinerules/core.md",),
        "Roo": (".roo/rules-code/core.md",),
        "Windsurf": (".windsurfrules",),
        "Aider": ("CONVENTIONS.md",),
        "Augment": ("augment-guidelines.md",),
        "Amazon Q": (".amazonq/rules/core.md",),
    }
    routes = (".apc/manifest.md", ".apc/init.md", ".apc/workflow.md")
    for name, paths in groups.items():
        text = "\n".join(read(path) for path in paths)
        for route in routes:
            if route not in text:
                errors.append(f"{name}: missing route to {route}")
        if "no-op" not in text:
            errors.append(f"{name}: missing task-end no-op")


def ids(text: str, kind: str) -> tuple[set[str], set[str]]:
    if kind == "rules":
        index = set(re.findall(r"^\|\s*(R\d+)\s*\|", text, re.MULTILINE))
        body = set(
            re.findall(
                r"^\s*-\s*\[[ xX]\]\s*(R\d+):",
                text,
                re.MULTILINE,
            )
        )
    else:
        index = set(re.findall(r"^-\s*§(\d+)\s", text, re.MULTILINE))
        body = set(re.findall(r"^###\s*§(\d+)\s", text, re.MULTILINE))
    return index, body


def check_indexes() -> None:
    for relative, kind in (
        (".apc/rules.md", "rules"),
        (".apc/decisions.md", "decisions"),
    ):
        index, body = ids(read(relative), kind)
        if index != body:
            errors.append(
                f"{relative}: index {sorted(index)} != body {sorted(body)}"
            )


def check_memory() -> None:
    path = ".apc/memory.md"
    content = read(path)
    count = len(re.findall(r"^### Session\b", content, re.MULTILINE))
    if count > 3:
        errors.append(f"{path}: {count} recent sessions, maximum is 3")

    for field in (
        "**Status**:",
        "**Latest progress**:",
        "**Next step**:",
        "**Known issues**:",
    ):
        if content.count(field) != 1:
            errors.append(f"{path}: expected exactly one {field}")


def main() -> int:
    check_entries()
    check_indexes()
    check_memory()

    if errors:
        for error in errors:
            print(f"ERROR: {error}")
        return 1
    print("APC structure check passed.")
    return 0


if __name__ == "__main__":
    sys.exit(main())
