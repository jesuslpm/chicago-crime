"""Summarize `git diff --numstat` by area for a closing report.

Usage:
    python -I numstat_by_area.py <base> [<head>] [--lang es|en]

An area is `crates/<name>` for workspace crates, the first path segment for any other folder, and the file
name for files at the repository root. Lockfiles and generated or binary files are listed apart and kept out
of the totals. Prints a Markdown table to stdout.
"""

import subprocess
import sys
from collections import defaultdict

LOCKFILES = {"Cargo.lock", "package-lock.json", "yarn.lock", "pnpm-lock.yaml", "poetry.lock", "uv.lock"}

LABELS = {
    "es": {"area": "Área", "files": "Archivos", "total": "Total", "apart": "Aparte (lockfiles, generados o binarios)"},
    "en": {"area": "Area", "files": "Files", "total": "Total", "apart": "Apart (lockfiles, generated or binary)"},
}


def area_of(path: str) -> str:
    parts = path.split("/")
    if len(parts) == 1:
        return path
    if parts[0] == "crates" and len(parts) > 2:
        return f"crates/{parts[1]}"
    return parts[0]


def thousands(n: int, lang: str) -> str:
    text = f"{n:,}"
    return text.replace(",", ".") if lang == "es" else text


def main() -> int:
    sys.stdout.reconfigure(encoding="utf-8")  # Windows consoles default to cp1252
    args =[a for a in sys.argv[1:] if not a.startswith("--lang")]
    lang = "es"
    for a in sys.argv[1:]:
        if a.startswith("--lang="):
            lang = a.split("=", 1)[1]
    if "--lang" in sys.argv:
        lang = sys.argv[sys.argv.index("--lang") + 1]
        args = [a for a in args if a != lang]
    if lang not in LABELS or not args:
        print(__doc__, file=sys.stderr)
        return 2
    base, head = args[0], (args[1] if len(args) > 1 else "HEAD")
    out = subprocess.run(
        ["git", "diff", "--numstat", f"{base}..{head}"], capture_output=True, text=True, check=True
    ).stdout

    areas = defaultdict(lambda: [0, 0, 0])
    apart = []
    for line in out.splitlines():
        added, removed, path = line.split("\t", 2)
        if " => " in path:  # rename: keep the destination path
            path = path.split(" => ")[-1].replace("}", "").replace("{", "")
        name = path.rsplit("/", 1)[-1]
        if added == "-" or name in LOCKFILES:
            apart.append((path, added, removed))
            continue
        a = areas[area_of(path)]
        a[0] += int(added)
        a[1] += int(removed)
        a[2] += 1

    t = LABELS[lang]
    print(f"| {t['area']} | + | − | {t['files']} |")
    print("|---|---|---|---|")
    total = [0, 0, 0]
    for area, (add, rem, files) in sorted(areas.items(), key=lambda kv: -(kv[1][0] + kv[1][1])):
        print(f"| `{area}` | {thousands(add, lang)} | {thousands(rem, lang)} | {files} |")
        total = [total[0] + add, total[1] + rem, total[2] + files]
    print(
        f"| **{t['total']}** | **{thousands(total[0], lang)}** | **{thousands(total[1], lang)}** "
        f"| **{total[2]}** |"
    )
    if apart:
        print()
        print(f"{t['apart']}: " + ", ".join(f"`{p}` (+{a} −{r})" for p, a, r in apart))
    return 0


if __name__ == "__main__":
    sys.exit(main())
