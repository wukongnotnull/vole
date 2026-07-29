#!/usr/bin/env python3
"""Semi-automated extractor: mole clean_*.bats → tests/fixtures/clean/*.json.

See scripts/extract-clean-fixtures.md for usage and limitations.
"""

from __future__ import annotations

import argparse
import json
import re
import sys
from pathlib import Path
from typing import Any

# Bats files with mkdir/touch + SAFE_CLEAN or assert_output_* patterns we can parse.
ALLOWLIST = (
    "clean_ai_cli_caches.bats",
    "clean_dev_caches.bats",
)

RE_TEST = re.compile(r'@test\s+"([^"]+)"\s*\{', re.MULTILINE)
RE_LOCAL = re.compile(
    r'^\s*local\s+(\w+)="(\$HOME[^"]*)"\s*$', re.MULTILINE
)
RE_LOCAL_SIMPLE = re.compile(r'^\s*local\s+(\w+)="([^"]*)"\s*$', re.MULTILINE)
RE_MKDIR = re.compile(
    r'mkdir\s+-p\s+((?:"[^"]+"\s*)+|"[^"]+")', re.MULTILINE
)
RE_TOUCH = re.compile(
    r'touch\s+-t\s+(\d{12})\s+((?:"[^"]+"\s*)+|"[^"]+")', re.MULTILINE
)
RE_WRITE = re.compile(
    r'echo\s+"([^"]*)"\s*>\s*"([^"]+)"', re.MULTILINE
)
RE_ASSERT_CONTAINS = re.compile(
    r'assert_output_contains\s+"([^"]+)"', re.MULTILINE
)
RE_ASSERT_NOT_CONTAINS = re.compile(
    r'assert_output_not_contains\s+"([^"]+)"', re.MULTILINE
)
RE_INLINE_ASSERT_CONTAINS = re.compile(
    r'\[\[\s*"\$output"\s*==\s*\*"\$?([^"]+)"\*\s*\]\]', re.MULTILINE
)
RE_INLINE_ASSERT_NOT = re.compile(
    r'\[\[\s*"\$output"\s*!=\s*\*"\$?([^"]+)"\*\s*\]\]', re.MULTILINE
)
RE_SAFE_CLEAN = re.compile(r"SAFE_CLEAN:([^|]+)\|(.+)")


def slugify(name: str) -> str:
    s = name.lower()
    s = re.sub(r"[^a-z0-9]+", "_", s)
    return s.strip("_")[:96]


def touch_to_iso(ts: str) -> str:
    # touch -t [[CC]YY]MMDDhhmm[.ss]
    if len(ts) == 12:
        year, mm, dd, hh, mi = ts[0:4], ts[4:6], ts[6:8], ts[8:10], ts[10:12]
    elif len(ts) == 10:
        year, mm, dd, hh, mi = f"20{ts[0:2]}", ts[2:4], ts[4:6], ts[6:8], ts[8:10]
    else:
        return ts
    return f"{year}-{mm}-{dd}T{hh}:{mi}"


def split_quoted_paths(blob: str) -> list[str]:
    return re.findall(r'"([^"]+)"', blob)


def expand_path(raw: str, locals_map: dict[str, str]) -> str:
    path = raw
    for _ in range(8):
        changed = False
        for name, value in locals_map.items():
            token = f"${name}"
            if token in path:
                path = path.replace(token, value)
                changed = True
        if not changed:
            break
    path = path.replace("$HOME", "~")
    return path


def collect_locals(body: str) -> dict[str, str]:
    locals_map: dict[str, str] = {}
    for m in RE_LOCAL.finditer(body):
        locals_map[m.group(1)] = m.group(2).replace("$HOME", "~")
    for m in RE_LOCAL_SIMPLE.finditer(body):
        name, value = m.group(1), m.group(2)
        if name in locals_map:
            continue
        if value.startswith("$HOME"):
            locals_map[name] = value.replace("$HOME", "~")
        elif "$HOME" in value:
            locals_map[name] = value.replace("$HOME", "~")
    return locals_map


def parse_expect_selected(text: str, locals_map: dict[str, str]) -> list[str]:
    selected: list[str] = []
    for m in RE_ASSERT_CONTAINS.finditer(text):
        raw = expand_path(m.group(1), locals_map)
        sm = RE_SAFE_CLEAN.search(raw)
        if sm:
            label, path = sm.group(1), sm.group(2)
            selected.append(f"{expand_path(path, locals_map)}|{label}")
    for m in RE_INLINE_ASSERT_CONTAINS.finditer(text):
        raw = expand_path(m.group(1), locals_map)
        sm = RE_SAFE_CLEAN.search(raw)
        if sm:
            label, path = sm.group(1), sm.group(2)
            selected.append(f"{expand_path(path, locals_map)}|{label}")
    # Stable order for deterministic output.
    return sorted(dict.fromkeys(selected))


def parse_expect_not_selected(text: str, locals_map: dict[str, str]) -> list[str]:
    not_selected: list[str] = []
    for m in RE_ASSERT_NOT_CONTAINS.finditer(text):
        raw = expand_path(m.group(1), locals_map)
        if "|" in raw and raw.startswith("SAFE_CLEAN:"):
            continue
        if raw.startswith("SAFE_CLEAN:"):
            continue
        if "skipped" in raw or "Claude Desktop" in raw and "/" not in raw:
            continue
        not_selected.append(raw)
    for m in RE_INLINE_ASSERT_NOT.finditer(text):
        raw = expand_path(m.group(1), locals_map)
        if "SAFE_CLEAN:" in raw:
            continue
        if "skipped" in raw:
            continue
        not_selected.append(raw)
    return sorted(dict.fromkeys(not_selected))


def parse_fixture_steps(body: str, locals_map: dict[str, str]) -> list[dict[str, Any]]:
    mtime_by_path: dict[str, str] = {}
    mkdir_paths: list[str] = []
    writes: list[dict[str, str]] = []

    for m in RE_MKDIR.finditer(body):
        for p in split_quoted_paths(m.group(1)):
            mkdir_paths.append(expand_path(p, locals_map))

    for m in RE_TOUCH.finditer(body):
        iso = touch_to_iso(m.group(1))
        for p in split_quoted_paths(m.group(2)):
            mtime_by_path[expand_path(p, locals_map)] = iso

    for m in RE_WRITE.finditer(body):
        content, path = m.group(1), expand_path(m.group(2), locals_map)
        writes.append({"write": path, "content": content})

    steps: list[dict[str, Any]] = []
    seen_mkdir: set[str] = set()
    for p in sorted(dict.fromkeys(mkdir_paths)):
        if p in seen_mkdir:
            continue
        seen_mkdir.add(p)
        step: dict[str, Any] = {"mkdir": p}
        if p in mtime_by_path:
            step["mtime"] = mtime_by_path[p]
        steps.append(step)

    steps.extend(writes)
    return steps


def split_tests(source: str) -> list[tuple[str, str]]:
    matches = list(RE_TEST.finditer(source))
    tests: list[tuple[str, str]] = []
    for i, m in enumerate(matches):
        start = m.end()
        end = matches[i + 1].start() if i + 1 < len(matches) else len(source)
        tests.append((m.group(1), source[start:end]))
    return tests


def extract_from_file(bats_path: Path, repo_root: Path) -> list[dict[str, Any]]:
    source = bats_path.read_text(encoding="utf-8")
    rel_bats = bats_path.relative_to(repo_root).as_posix()
    fixtures: list[dict[str, Any]] = []

    for test_name, body in split_tests(source):
        locals_map = collect_locals(body)
        fixture_steps = parse_fixture_steps(body, locals_map)
        expect_selected = parse_expect_selected(body, locals_map)
        expect_not_selected = parse_expect_not_selected(body, locals_map)

        if not fixture_steps:
            continue
        if not expect_selected and not expect_not_selected:
            continue

        fx: dict[str, Any] = {
            "id": slugify(test_name),
            "source_bats": rel_bats,
            "source_test": test_name,
            "fixture": fixture_steps,
        }
        if expect_selected:
            fx["expect_selected"] = expect_selected
        if expect_not_selected:
            fx["expect_not_selected"] = expect_not_selected
        fixtures.append(fx)

    return fixtures


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument(
        "--repo-root",
        type=Path,
        default=Path(__file__).resolve().parent.parent,
        help="Vole repository root (default: parent of scripts/)",
    )
    parser.add_argument(
        "--out-dir",
        type=Path,
        default=None,
        help="Output directory (default: <repo>/tests/fixtures/clean)",
    )
    parser.add_argument(
        "--bats",
        action="append",
        default=[],
        help="Bats filename under third_party/mole-1.48.1/tests/ (repeatable)",
    )
    args = parser.parse_args()

    repo_root = args.repo_root.resolve()
    out_dir = (args.out_dir or repo_root / "tests" / "fixtures" / "clean").resolve()
    mole_tests = repo_root / "third_party" / "mole-1.48.1" / "tests"
    allowlist = tuple(args.bats) if args.bats else ALLOWLIST

    out_dir.mkdir(parents=True, exist_ok=True)

    written = 0
    for name in sorted(allowlist):
        bats_path = mole_tests / name
        if not bats_path.is_file():
            print(f"skip missing: {bats_path}", file=sys.stderr)
            continue
        for fx in extract_from_file(bats_path, repo_root):
            out_path = out_dir / f"{fx['id']}.json"
            payload = json.dumps(fx, indent=2, ensure_ascii=False) + "\n"
            out_path.write_text(payload, encoding="utf-8")
            written += 1
            print(out_path.relative_to(repo_root))

    if written == 0:
        print("no fixtures extracted", file=sys.stderr)
        return 1
    return 0


if __name__ == "__main__":
    sys.exit(main())
