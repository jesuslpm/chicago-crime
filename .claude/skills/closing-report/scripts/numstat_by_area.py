"""Summarize `git diff --numstat` by area for a closing report.

Usage:
    python -I numstat_by_area.py <base> [<head>] [--lang es|en] [--apart <glob> ...]

An area is `crates/<name>` for workspace crates, the first path segment for any other folder, and the file
name for files at the repository root. Lockfiles, binary files and any path matching an `--apart` glob
(use it for generated files, e.g. `--apart 'docs/methodology/slides.html'`) are listed apart and kept out of
the totals. Renames are counted under their destination path. Prints a Markdown table to stdout.
"""

import argparse
import fnmatch
import subprocess
import sys
from collections import defaultdict
from dataclasses import dataclass

LOCKFILES = {"Cargo.lock", "package-lock.json", "yarn.lock", "pnpm-lock.yaml", "poetry.lock", "uv.lock"}

LABELS = {
    "es": {"area": "Área", "files": "Archivos", "total": "Total", "apart": "Aparte (lockfiles, generados o binarios)"},
    "en": {"area": "Area", "files": "Files", "total": "Total", "apart": "Apart (lockfiles, generated or binary)"},
}


@dataclass
class Counts:
    added: int = 0
    removed: int = 0
    files: int = 0


def area_of(path: str) -> str:
    parts = path.split("/")
    if len(parts) == 1:
        return path
    if parts[0] == "crates" and len(parts) > 2:
        return f"crates/{parts[1]}"
    return parts[0]


def parse_args(argv):
    parser = argparse.ArgumentParser(description="Lines added and removed by area, from git diff --numstat.")
    parser.add_argument("base", help="base commit, e.g. the branch point")
    parser.add_argument("head", nargs="?", default="HEAD", help="head commit (default: HEAD)")
    parser.add_argument("--lang", choices=sorted(LABELS), default="es", help="table language (default: es)")
    parser.add_argument("--apart", action="append", default=[], metavar="GLOB",
                        help="path glob listed apart and kept out of the totals (repeatable)")
    return parser.parse_args(argv)


def parse_numstat_z(data: str):
    """Parse `git diff --numstat -z` output into (added, removed, path) tuples.

    With -z, paths are never quoted, and a rename is written as an empty path field followed by the old
    and the new path, each terminated by NUL. The destination path is kept.
    """
    fields = data.split("\0")
    entries = []
    i = 0
    while i < len(fields) and fields[i]:
        added, removed, path = fields[i].split("\t", 2)
        if path == "":
            path = fields[i + 2]  # rename: skip the old path, keep the new one
            i += 3
        else:
            i += 1
        entries.append((added, removed, path))
    return entries


def summarize(entries, apart_globs):
    areas = defaultdict(Counts)
    apart = []
    for added, removed, path in entries:
        name = path.rsplit("/", 1)[-1]
        if added == "-" or name in LOCKFILES or any(fnmatch.fnmatch(path, g) for g in apart_globs):
            apart.append((path, added, removed))
            continue
        counts = areas[area_of(path)]
        counts.added += int(added)
        counts.removed += int(removed)
        counts.files += 1
    return areas, apart


def thousands(n: int, lang: str) -> str:
    text = f"{n:,}"
    return text.replace(",", ".") if lang == "es" else text


def render(areas, apart, lang):
    t = LABELS[lang]
    lines = [f"| {t['area']} | + | − | {t['files']} |", "|---|---|---|---|"]
    total = Counts()
    for area, c in sorted(areas.items(), key=lambda kv: -(kv[1].added + kv[1].removed)):
        lines.append(f"| `{area}` | {thousands(c.added, lang)} | {thousands(c.removed, lang)} | {c.files} |")
        total.added += c.added
        total.removed += c.removed
        total.files += c.files
    lines.append(
        f"| **{t['total']}** | **{thousands(total.added, lang)}** | **{thousands(total.removed, lang)}** "
        f"| **{total.files}** |"
    )
    if apart:
        lines.append("")
        lines.append(f"{t['apart']}: " + ", ".join(f"`{p}` (+{a} −{r})" for p, a, r in apart))
    return "\n".join(lines)


def main(argv=None) -> int:
    sys.stdout.reconfigure(encoding="utf-8")  # Windows consoles default to cp1252
    args = parse_args(sys.argv[1:] if argv is None else argv)
    result = subprocess.run(
        ["git", "diff", "--numstat", "-z", f"{args.base}..{args.head}"],
        capture_output=True, text=True, encoding="utf-8",
    )
    if result.returncode != 0:
        print(f"git diff failed: {result.stderr.strip()}", file=sys.stderr)
        return 2
    areas, apart = summarize(parse_numstat_z(result.stdout), args.apart)
    print(render(areas, apart, args.lang))
    return 0


if __name__ == "__main__":
    sys.exit(main())
