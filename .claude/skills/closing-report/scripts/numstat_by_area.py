"""Summarize `git diff --numstat` by area for a closing report.

Usage:
    python -I numstat_by_area.py <base> [<head>] [--lang es|en] [--apart <glob> ...]

An area is `crates/<name>` for workspace crates, the first path segment for any other folder, and the file
name for files at the repository root. Lockfiles, binary files and any path matching an `--apart` glob
(use it for generated files, e.g. `--apart 'docs/methodology/slides.html'`) are listed apart and kept out of
the totals. Renames are counted under their destination path. Prints a Markdown table to stdout.

The lines added in code files (see `LANGUAGES`) are split into code and comments, read from the file at
<head>: comments are line and block comments, Rust doc comments and Python docstrings; a line with code and a
trailing comment is code; blank lines count in neither. Other files show no split.
"""

import argparse
import ast
import fnmatch
import io
import re
import subprocess
import sys
import tokenize
from collections import defaultdict
from dataclasses import dataclass

LOCKFILES = {"Cargo.lock", "package-lock.json", "yarn.lock", "pnpm-lock.yaml", "poetry.lock", "uv.lock"}

LABELS = {
    "es": {
        "area": "Área", "files": "Archivos", "total": "Total", "code": "Código +", "comments": "Comentarios +",
        "apart": "Aparte (lockfiles, generados o binarios)",
        "legend": "Código + y Comentarios + desglosan las líneas añadidas en archivos de código (Rust, Python, "
                  "PowerShell, JavaScript/TypeScript, TOML, YAML y shell); las líneas en blanco no cuentan en "
                  "ninguna de las dos.",
    },
    "en": {
        "area": "Area", "files": "Files", "total": "Total", "code": "Code +", "comments": "Comments +",
        "apart": "Apart (lockfiles, generated or binary)",
        "legend": "Code + and Comments + split the lines added in code files (Rust, Python, PowerShell, "
                  "JavaScript/TypeScript, TOML, YAML and shell); blank lines count in neither.",
    },
}

BLANK, CODE, COMMENT = "blank", "code", "comment"


@dataclass
class Counts:
    added: int = 0
    removed: int = 0
    files: int = 0
    code: int = 0
    comments: int = 0
    code_files: int = 0


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


# --- Line classification -------------------------------------------------------------------------------


def _kind(line: str, code: bool, comment: bool) -> str:
    if not line.strip():
        return BLANK
    if code:
        return CODE
    return COMMENT if comment else CODE


def classify_c_like(text: str, rust: bool):
    """Classify the lines of a Rust (nested block comments, raw strings, char literals) or JavaScript file."""
    kinds = []
    depth = 0  # block comment nesting
    quote = None  # (terminator, has_escapes) of the string still open from an earlier line or position
    for line in text.split("\n"):
        line = line.rstrip("\r")
        code = comment = False
        i, n = 0, len(line)
        while i < n:
            c = line[i]
            if depth:
                if line.startswith("*/", i):
                    depth, i = depth - 1, i + 2
                elif rust and line.startswith("/*", i):
                    depth, i = depth + 1, i + 2
                else:
                    i += 1
                comment = comment or not c.isspace()
                continue
            if quote:
                code = code or not c.isspace()
                terminator, escapes = quote
                if escapes and c == "\\":
                    i += 2
                elif line.startswith(terminator, i):
                    quote, i = None, i + len(terminator)
                else:
                    i += 1
                continue
            if c.isspace():
                i += 1
                continue
            if line.startswith("//", i):
                comment = True
                break
            if line.startswith("/*", i):
                depth, i, comment = 1, i + 2, True
                continue
            code = True
            raw = re.match(r'r(#*)"', line[i:]) if rust else None
            if raw and (i == 0 or not _ident(line[i - 1]) or (line[i - 1] == "b" and (i < 2 or not _ident(line[i - 2])))):
                quote, i = ('"' + raw.group(1), False), i + raw.end()
            elif c == '"' or (not rust and c in "'`"):
                quote, i = (c, True), i + 1
            elif rust and c == "'":
                if line.startswith("\\", i + 1):  # escaped char literal such as '\n' or '\''
                    end = line.find("'", i + 3)
                    i = n if end == -1 else end + 1
                elif i + 2 < n and line[i + 2] == "'":  # one-char literal
                    i += 3
                else:  # lifetime
                    i += 1
            else:
                i += 1
        kinds.append(_kind(line, code, comment))
    return kinds


def _ident(c: str) -> bool:
    return c.isalnum() or c == "_"


def classify_hash(text: str, powershell: bool = False):
    """Classify the lines of a language whose comments start with `#` (PowerShell adds `<# #>` and here-strings)."""
    kinds = []
    block = False  # inside a PowerShell <# #> comment
    here = None  # terminator of an open PowerShell here-string
    for line in text.split("\n"):
        line = line.rstrip("\r")
        code = comment = False
        i, n = 0, len(line)
        if here:
            if not line.startswith(here):
                kinds.append(_kind(line, True, False))
                continue
            here, i, code = None, len(here), True
        while i < n:
            c = line[i]
            if block:
                if line.startswith("#>", i):
                    block, i = False, i + 2
                else:
                    i += 1
                comment = comment or not c.isspace()
                continue
            if c.isspace():
                i += 1
                continue
            if powershell and line.startswith("<#", i):
                block, i, comment = True, i + 2, True
                continue
            if c == "#":
                comment = True
                break
            code = True
            if powershell and line[i:].rstrip() in ('@"', "@'"):
                here = line[i + 1] + "@"
                break
            if c in "'\"":
                escape = "`" if powershell else "\\"
                j = i + 1
                while j < n and line[j] != c:
                    j += 2 if c == '"' and line[j] == escape else 1
                i = j + 1
            else:
                i += 1
        kinds.append(_kind(line, code, comment))
    return kinds


def classify_python(text: str):
    """Classify Python lines with the tokenizer; docstrings are comments. Unparsable code falls back to `#`."""
    try:
        tree = ast.parse(text)
        tokens = list(tokenize.generate_tokens(io.StringIO(text).readline))
    except (SyntaxError, tokenize.TokenError, ValueError):
        return classify_hash(text)
    docstring_lines = set()
    for node in ast.walk(tree):
        if isinstance(node, (ast.Module, ast.ClassDef, ast.FunctionDef, ast.AsyncFunctionDef)) and node.body:
            first = node.body[0]
            if isinstance(first, ast.Expr) and isinstance(first.value, ast.Constant) \
                    and isinstance(first.value.value, str):
                docstring_lines.add(first.value.lineno)
    skip = {tokenize.NL, tokenize.NEWLINE, tokenize.INDENT, tokenize.DEDENT, tokenize.ENDMARKER, tokenize.ENCODING}
    code, comment = set(), set()
    for tok in tokens:
        lines = range(tok.start[0], tok.end[0] + 1)
        if tok.type == tokenize.COMMENT or (tok.type == tokenize.STRING and tok.start[0] in docstring_lines):
            comment.update(lines)
        elif tok.type not in skip:
            code.update(lines)
    return [_kind(line, number in code, number in comment) for number, line in enumerate(text.split("\n"), 1)]


LANGUAGES = {
    **dict.fromkeys([".rs"], lambda text: classify_c_like(text, rust=True)),
    **dict.fromkeys([".js", ".mjs", ".cjs", ".jsx", ".ts", ".tsx"], lambda text: classify_c_like(text, rust=False)),
    **dict.fromkeys([".py"], classify_python),
    **dict.fromkeys([".ps1", ".psm1", ".psd1"], lambda text: classify_hash(text, powershell=True)),
    **dict.fromkeys([".toml", ".yml", ".yaml", ".sh", ".bash"], classify_hash),
}


def language_of(path: str):
    name = path.rsplit("/", 1)[-1]
    dot = name.rfind(".")
    return LANGUAGES.get(name[dot:].lower()) if dot > 0 else None


def classify(path: str, text: str):
    """Kind of every line of `text` (BLANK, CODE or COMMENT), by the language of `path`."""
    return language_of(path)(text)


def count_added(kinds, line_numbers):
    """(code, comments) among the given 1-based line numbers."""
    picked = [kinds[n - 1] for n in line_numbers if n <= len(kinds)]
    return picked.count(CODE), picked.count(COMMENT)


# --- Added lines from the patch ------------------------------------------------------------------------

HUNK = re.compile(r"^@@ -\d+(?:,(\d+))? \+(\d+)(?:,(\d+))? @@")
ESCAPES = {"a": 7, "b": 8, "t": 9, "n": 10, "v": 11, "f": 12, "r": 13, '"': 34, "\\": 92}


def unquote_path(text: str) -> str:
    """Undo git's C-style quoting of a path (octal escapes are UTF-8 bytes)."""
    if not (len(text) >= 2 and text[0] == text[-1] == '"'):
        return text
    out = bytearray()
    body, i = text[1:-1], 0
    while i < len(body):
        if body[i] == "\\" and i + 1 < len(body):
            if body[i + 1] in "01234567":
                out.append(int(body[i + 1:i + 4], 8))
                i += 4
            else:
                out.append(ESCAPES.get(body[i + 1], ord(body[i + 1])))
                i += 2
        else:
            out.extend(body[i].encode("utf-8"))
            i += 1
    return out.decode("utf-8", errors="replace")


def added_lines(patch: str):
    """Map each destination path of a `git diff -U0 --no-prefix` patch to its added line numbers."""
    result = defaultdict(list)
    path = None
    old_left = new_left = 0  # lines still to read in the current hunk
    for line in patch.split("\n"):
        if old_left or new_left:
            if line.startswith("-"):
                old_left -= 1
            elif line.startswith("+"):
                new_left -= 1
            continue
        if line.startswith("+++ "):
            target = line[4:].rstrip("\t")
            path = None if target == "/dev/null" else unquote_path(target)
            continue
        hunk = HUNK.match(line)
        if hunk and path is not None:
            old_left = 1 if hunk.group(1) is None else int(hunk.group(1))
            start = int(hunk.group(2))
            new_left = 1 if hunk.group(3) is None else int(hunk.group(3))
            result[path].extend(range(start, start + new_left))
    return dict(result)


# --- Summary -------------------------------------------------------------------------------------------


def is_apart(added: str, path: str, apart_globs) -> bool:
    name = path.rsplit("/", 1)[-1]
    # fnmatchcase: exact case on every OS; note that `*` also matches across `/`.
    return added == "-" or name in LOCKFILES or any(fnmatch.fnmatchcase(path, g) for g in apart_globs)


def summarize(entries, apart_globs, breakdown=None):
    """Counts by area; `breakdown` maps a code file to the (code, comments) of its added lines."""
    breakdown = breakdown or {}
    areas = defaultdict(Counts)
    apart = []
    for added, removed, path in entries:
        if is_apart(added, path, apart_globs):
            apart.append((path, added, removed))
            continue
        counts = areas[area_of(path)]
        counts.added += int(added)
        counts.removed += int(removed)
        counts.files += 1
        if language_of(path):
            code, comments = breakdown.get(path, (0, 0))
            counts.code += code
            counts.comments += comments
            counts.code_files += 1
    return areas, apart


def thousands(n: int, lang: str) -> str:
    text = f"{n:,}"
    return text.replace(",", ".") if lang == "es" else text


def render(areas, apart, lang):
    t = LABELS[lang]

    def split(n, c):
        return thousands(n, lang) if c.code_files else "—"

    lines = [
        f"| {t['area']} | + | − | {t['code']} | {t['comments']} | {t['files']} |",
        "|---|---|---|---|---|---|",
    ]
    total = Counts()
    for area, c in sorted(areas.items(), key=lambda kv: -(kv[1].added + kv[1].removed)):
        lines.append(
            f"| `{area}` | {thousands(c.added, lang)} | {thousands(c.removed, lang)} "
            f"| {split(c.code, c)} | {split(c.comments, c)} | {c.files} |"
        )
        for name in ("added", "removed", "files", "code", "comments", "code_files"):
            setattr(total, name, getattr(total, name) + getattr(c, name))
    lines.append(
        f"| **{t['total']}** | **{thousands(total.added, lang)}** | **{thousands(total.removed, lang)}** "
        f"| **{split(total.code, total)}** | **{split(total.comments, total)}** | **{total.files}** |"
    )
    lines += ["", t["legend"]]
    if apart:
        lines.append("")
        lines.append(f"{t['apart']}: " + ", ".join(f"`{p}` (+{a} −{r})" for p, a, r in apart))
    return "\n".join(lines)


def git(*args):
    return subprocess.run(["git", *args], capture_output=True)


def main(argv=None) -> int:
    sys.stdout.reconfigure(encoding="utf-8")  # Windows consoles default to cp1252
    args = parse_args(sys.argv[1:] if argv is None else argv)
    result = subprocess.run(
        ["git", "diff", "--numstat", "-z", "--end-of-options", f"{args.base}..{args.head}"],
        capture_output=True, text=True, encoding="utf-8", errors="replace",
    )
    if result.returncode != 0:
        print(f"git diff failed: {result.stderr.strip()}", file=sys.stderr)
        return 2
    entries = parse_numstat_z(result.stdout)
    patch = git("-c", "core.quotePath=false", "diff", "-U0", "--no-color", "--no-ext-diff", "--no-textconv",
                "--no-prefix", "--end-of-options", f"{args.base}..{args.head}")
    if patch.returncode != 0:
        print(f"git diff failed: {patch.stderr.decode('utf-8', 'replace').strip()}", file=sys.stderr)
        return 2
    added = added_lines(patch.stdout.decode("utf-8", errors="replace"))
    breakdown = {}
    for count, _, path in entries:
        if is_apart(count, path, args.apart) or not language_of(path) or path not in added:
            continue
        blob = git("show", f"{args.head}:{path}")
        if blob.returncode != 0:
            print(f"git show failed: {blob.stderr.decode('utf-8', 'replace').strip()}", file=sys.stderr)
            return 2
        breakdown[path] = count_added(classify(path, blob.stdout.decode("utf-8", errors="replace")), added[path])
    areas, apart = summarize(entries, args.apart, breakdown)
    print(render(areas, apart, args.lang))
    return 0


if __name__ == "__main__":
    sys.exit(main())
