#!/usr/bin/env python3
"""Inventory mole `safe_clean` call sites vs vole `data/rules/*.toml`.

Usage:
  python3 scripts/inventory-mole-rules.py
  python3 scripts/inventory-mole-rules.py --json /tmp/mole-rules.json
  python3 scripts/inventory-mole-rules.py --csv /tmp/mole-rules.csv
  python3 scripts/inventory-mole-rules.py --self-test
"""

from __future__ import annotations

import argparse
import csv
import json
import re
import sys
from pathlib import Path

SAFE_CLEAN_RE = re.compile(
    r"""safe_clean\s+"""
    r"""(?P<path>(?:\\.|[^\s"'])+|"(?:\\.|[^"])*"|'(?:\\.|[^'])*')\s+"""
    r""""(?P<label>[^"]*)"""
)

ID_RE = re.compile(r'(?m)^\s*id\s*=\s*"([^"]+)"\s*$')
LABEL_RE = re.compile(r'(?m)^\s*label\s*=\s*"([^"]+)"\s*$')
QUOTED_PATH_RE = re.compile(r'"((?:\\.|[^"\\])*)"')

# mole proposed_id → vole rule id when labels differ but coverage is known.
ID_ALIASES: dict[str, str] = {
    "homebrew-cache": "homebrew-downloads-cache",
}


def repo_root() -> Path:
    return Path(__file__).resolve().parent.parent


def unescape_bash_path(raw: str) -> str:
    s = raw.strip()
    if (s.startswith('"') and s.endswith('"')) or (s.startswith("'") and s.endswith("'")):
        s = s[1:-1]
    # bash backslash-escapes spaces: Application\ Support → Application Support
    out: list[str] = []
    i = 0
    while i < len(s):
        if s[i] == "\\" and i + 1 < len(s):
            out.append(s[i + 1])
            i += 2
            continue
        out.append(s[i])
        i += 1
    return "".join(out)


def guess_complexity(line: str, path_expr: str, context: str) -> str:
    blob = f"{line}\n{context}".lower()
    if "sudo" in blob:
        return "sudo"
    if path_expr.startswith("$") or "${" in path_expr:
        return "custom"
    if any(
        k in blob
        for k in (
            "keep_newest",
            "keep newest",
            "mtime",
            "by age",
            "older than",
            "retain",
            "keep=",
        )
    ):
        return "mtime"
    if any(k in blob for k in ("pgrep", "not_running", "is_running", "osascript")):
        return "guard"
    if "custom" in blob or "while " in blob:
        return "custom"
    return "all"


def normalize_path_expr(path: str) -> str:
    """Normalize path expressions for equality checks (slug noise reduction)."""
    s = unescape_bash_path(path).strip()
    s = s.replace("\\ ", " ")
    s = re.sub(r" {2,}", " ", s)
    return s


def load_ported(rules_dir: Path) -> tuple[set[str], set[str], set[str]]:
    ids: set[str] = set()
    labels: set[str] = set()
    paths: set[str] = set()
    if not rules_dir.is_dir():
        return ids, labels, paths
    for path in sorted(rules_dir.glob("*.toml")):
        text = path.read_text(encoding="utf-8")
        ids.update(ID_RE.findall(text))
        labels.update(LABEL_RE.findall(text))
        for block in re.split(r"(?m)^\[\[rule\]\]\s*$", text)[1:]:
            for m in re.finditer(r"(?ms)^\s*paths\s*=\s*\[(.*?)\]", block):
                for qm in QUOTED_PATH_RE.finditer(m.group(1)):
                    paths.add(normalize_path_expr(qm.group(1)))
    return ids, labels, paths


def slugify_label(label: str) -> str:
    s = label.lower()
    s = re.sub(r"[^a-z0-9]+", "-", s)
    return s.strip("-")[:64]


def match_ported(
    *,
    proposed: str,
    label: str,
    path_expr: str,
    ported_ids: set[str],
    ported_labels: set[str],
    ported_paths: set[str],
) -> tuple[bool, str]:
    """Return (ported, match_reason)."""
    if proposed in ported_ids:
        return True, "id"
    alias = ID_ALIASES.get(proposed)
    if alias is not None and alias in ported_ids:
        return True, "id_alias"
    if any(proposed == pid or proposed.startswith(pid) for pid in ported_ids):
        return True, "id_prefix"
    if label in ported_labels:
        return True, "label"
    if normalize_path_expr(path_expr) in ported_paths:
        return True, "path"
    return False, "none"


def inventory(clean_dir: Path, rules_dir: Path) -> list[dict]:
    ported_ids, ported_labels, ported_paths = load_ported(rules_dir)
    rows: list[dict] = []
    for sh in sorted(clean_dir.glob("*.sh")):
        text = sh.read_text(encoding="utf-8", errors="replace")
        lines = text.splitlines()
        for idx, line in enumerate(lines, start=1):
            if "safe_clean" not in line:
                continue
            # Collect a small window for heuristics (sudo / keep / pgrep).
            lo = max(0, idx - 6)
            hi = min(len(lines), idx + 5)
            context = "\n".join(lines[lo:hi])
            for m in SAFE_CLEAN_RE.finditer(line):
                path_expr = unescape_bash_path(m.group("path"))
                label = m.group("label")
                complexity = guess_complexity(line, path_expr, context)
                proposed = slugify_label(label)
                ported, reason = match_ported(
                    proposed=proposed,
                    label=label,
                    path_expr=path_expr,
                    ported_ids=ported_ids,
                    ported_labels=ported_labels,
                    ported_paths=ported_paths,
                )
                rows.append(
                    {
                        "source_file": sh.name,
                        "source_path": str(sh),
                        "line": idx,
                        "label": label,
                        "path_expr": path_expr,
                        "complexity_guess": complexity,
                        "proposed_id": proposed,
                        "ported": bool(ported),
                        "match_reason": reason,
                    }
                )
    return rows


def self_test() -> int:
    assert normalize_path_expr(r"~/Library/Application\ Support/Arc/ShaderCache/*") == (
        "~/Library/Application Support/Arc/ShaderCache/*"
    )
    ported_ids = {"homebrew-downloads-cache"}
    ported_paths = {
        normalize_path_expr("~/Library/Caches/Homebrew/downloads/*"),
        normalize_path_expr("~/Library/Application Support/Arc/ShaderCache/*"),
    }
    ok, reason = match_ported(
        proposed="homebrew-cache",
        label="Homebrew cache",
        path_expr="~/Library/Caches/Homebrew/downloads/*",
        ported_ids=ported_ids,
        ported_labels=set(),
        ported_paths=ported_paths,
    )
    assert ok and reason in {"path", "id_alias"}, (ok, reason)
    ok, reason = match_ported(
        proposed="arc-shader-cache",
        label="Arc shader cache",
        path_expr="~/Library/Application Support/Arc/ShaderCache/*",
        ported_ids=ported_ids,
        ported_labels=set(),
        ported_paths=ported_paths,
    )
    assert ok and reason == "path", (ok, reason)
    ok, reason = match_ported(
        proposed="missing-thing",
        label="Missing",
        path_expr="~/Library/Caches/nope/*",
        ported_ids=ported_ids,
        ported_labels=set(),
        ported_paths=ported_paths,
    )
    assert not ok and reason == "none", (ok, reason)
    print("self-test ok", file=sys.stderr)
    return 0


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument(
        "--repo-root",
        type=Path,
        default=None,
        help="Repository root (default: parent of scripts/)",
    )
    parser.add_argument("--json", type=Path, default=None, help="Write JSON array to PATH")
    parser.add_argument("--csv", type=Path, default=None, help="Write CSV to PATH")
    parser.add_argument(
        "--self-test",
        action="store_true",
        help="Run normalize/match unit checks and exit",
    )
    args = parser.parse_args()

    if args.self_test:
        return self_test()

    root = args.repo_root or repo_root()
    clean_dir = root / "third_party" / "mole-1.48.1" / "lib" / "clean"
    rules_dir = root / "data" / "rules"
    if not clean_dir.is_dir():
        print(f"missing clean dir: {clean_dir}", file=sys.stderr)
        return 1

    rows = inventory(clean_dir, rules_dir)
    summary = {
        "total": len(rows),
        "all": sum(1 for r in rows if r["complexity_guess"] == "all"),
        "mtime": sum(1 for r in rows if r["complexity_guess"] == "mtime"),
        "guard": sum(1 for r in rows if r["complexity_guess"] == "guard"),
        "custom": sum(1 for r in rows if r["complexity_guess"] == "custom"),
        "sudo": sum(1 for r in rows if r["complexity_guess"] == "sudo"),
        "ported": sum(1 for r in rows if r["ported"]),
        "unported_all": sum(
            1 for r in rows if r["complexity_guess"] == "all" and not r["ported"]
        ),
        "ported_by_path": sum(1 for r in rows if r.get("match_reason") == "path"),
        "ported_by_id_alias": sum(1 for r in rows if r.get("match_reason") == "id_alias"),
    }
    print(json.dumps(summary, indent=2))

    if args.json:
        args.json.write_text(json.dumps(rows, indent=2) + "\n", encoding="utf-8")
        print(f"wrote {args.json} ({len(rows)} rows)", file=sys.stderr)
    if args.csv:
        fields = [
            "source_file",
            "line",
            "label",
            "path_expr",
            "complexity_guess",
            "proposed_id",
            "ported",
            "match_reason",
        ]
        with args.csv.open("w", encoding="utf-8", newline="") as f:
            w = csv.DictWriter(f, fieldnames=fields, extrasaction="ignore")
            w.writeheader()
            for row in rows:
                w.writerow({k: row.get(k) for k in fields})
        print(f"wrote {args.csv}", file=sys.stderr)

    if not args.json and not args.csv:
        # Preview a few unported `all` rows from app_caches.sh
        preview = [
            r
            for r in rows
            if r["source_file"] == "app_caches.sh"
            and r["complexity_guess"] == "all"
            and not r["ported"]
        ][:12]
        print("--- preview unported all @ app_caches.sh ---")
        for r in preview:
            print(
                f"{r['line']}: {r['proposed_id']} | {r['path_expr']} | {r['label']} | {r['match_reason']}"
            )

    return 0


if __name__ == "__main__":
    raise SystemExit(main())
