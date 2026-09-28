"""Edits the five i18n literals and the `Texts` struct by field name.

Usage: python tools/i18n_edit.py spec.json
spec: {"remove": ["field", ...],
       "add": [{"after": "anchor", "struct": "pub x: &'static str,", "doc": "optional doc",
                "values": {"de": "x: \"…\",", "en": ..., ...}}],
       "replace": [{"field": "name", "values": {...}, "struct": optional}]}
Values are inserted verbatim (indented by four spaces per line).
"""

import json
import re
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1] / "src" / "i18n"
LANGS = ["de", "en", "fr", "es", "it"]


def depth_change(line: str) -> int:
    depth, in_str, escaped = 0, False, False
    i = 0
    while i < len(line):
        c = line[i]
        if in_str:
            if escaped:
                escaped = False
            elif c == "\\":
                escaped = True
            elif c == '"':
                in_str = False
        else:
            if c == "/" and line[i:i + 2] == "//":
                break
            if c == '"':
                in_str = True
            elif c == "'" and re.match(r"'(\\.|[^\\'])'", line[i:]):
                i += len(re.match(r"'(\\.|[^\\'])'", line[i:]).group(0))
                continue
            elif c in "([{":
                depth += 1
            elif c in ")]}":
                depth -= 1
        i += 1
    return depth


def entry_span(lines, field, struct=False):
    pat = re.compile(rf"^    (pub )?{re.escape(field)}: ")
    for i, line in enumerate(lines):
        if pat.match(line):
            depth, j = 0, i
            while True:
                depth += depth_change(lines[j])
                if depth <= 0 and lines[j].rstrip().endswith(","):
                    break
                j += 1
            start = i
            if struct:  # doc comments belong to the field
                while start > 0 and lines[start - 1].strip().startswith("///"):
                    start -= 1
            return start, j + 1
    raise KeyError(f"{field} not found")


def block(text: str) -> list:
    return ["    " + l if l else "" for l in text.split("\n")]


def edit(path: Path, spec: dict, lang: str | None):
    with path.open(encoding="utf-8", newline="") as fh:
        lines = fh.read().split("\n")
    struct = lang is None
    for field in spec.get("remove", []):
        s, e = entry_span(lines, field, struct)
        del lines[s:e]
    for item in spec.get("replace", []):
        new = item.get("struct") if struct else item["values"][lang]
        if new is None:
            continue
        s, e = entry_span(lines, item["field"], struct)
        doc = []
        if struct:
            # keep existing doc comments
            k = s
            while lines[k].strip().startswith("///"):
                doc.append(lines[k])
                k += 1
        lines[s:e] = doc + block(new)
    for item in spec.get("add", []):
        new = item.get("struct") if struct else item["values"][lang]
        if new is None:
            continue
        _, e = entry_span(lines, item["after"], struct)
        extra = block(new)
        if struct and item.get("doc"):
            extra = block("\n".join("/// " + d for d in item["doc"].split("\n"))) + extra
        lines[e:e] = extra
    with path.open("w", encoding="utf-8", newline="\n") as fh:
        fh.write("\n".join(lines))


def main():
    spec = json.loads(Path(sys.argv[1]).read_text(encoding="utf-8"))
    edit(ROOT / "mod.rs", spec, None)
    for lang in LANGS:
        edit(ROOT / f"{lang}.rs", spec, lang)
    print("ok")


if __name__ == "__main__":
    main()
