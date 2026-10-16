"""Turn the speaker notes of the Marp deck into one MP3 per slide, to rehearse by listening.

Usage (from the repository root, after `python -m pip install edge-tts`):

    python scripts/slides/notes_audio.py [--slides docs/methodology/slides.md]
        [--out docs/methodology/audio] [--voice es-ES-AlvaroNeural]

Only the spoken text of each "Cómo leerla" note is read: paragraphs that start with "▸" (directions for the
presenter) or "⏱" (timing marks) are skipped whole, even when they wrap. Words the voice gets wrong are respelled first (PRONUNCIATION);
the notes themselves keep the normal spelling. The output folder is git-ignored. It gets slide-NN.mp3 for every
slide with notes and duraciones.md with the length of each one.
"""

import argparse
import asyncio
import re
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
BITRATE = 48_000  # edge-tts writes audio-24khz-48kbitrate-mono-mp3: constant bitrate, so size gives length
NOTE = re.compile(r"<!--\s*Cómo leerla:(.*?)-->", re.S)
SKIPPED = ("▸", "⏱")

# Written form -> how the voice must see it. Whole words only, so GitHub or gitignore stay as they are.
PRONUNCIATION = [
    (r"\bCLAUDE\.md\b", "Claude punto eme de"),
    (r"\bAGENTS\.md\b", "Agents punto eme de"),
    (r"(?i)\bgit\b", "guit"),
    (r"(?i)\bworktree(s?)\b", r"uórktri\1"),
    (r"(?i)\bcommit(s?)\b", r"cómit\1"),
    (r"\bMiB\b", "mebibytes"),
    (r"(?i)\bskill(s?)\b", r"esquil\1"),
    (r"(?i)\bhook(s?)\b", r"juk\1"),
    (r"\bSSTable(s?)\b", r"ese ese table\1"),
    (r"(?i)\bauditable(s?)\b", r"auditáble\1"),
]


def spoken(note: str) -> str:
    """The text of a note that is read aloud: wrapped lines joined, paragraphs kept apart. A paragraph whose
    first line starts with a SKIPPED mark is a direction for the presenter and is left out whole, wrapped or not."""
    paragraphs, current = [], []
    for line in [*note.splitlines(), ""]:
        text = line.strip()
        if text:
            current.append(text)
        elif current:
            if not current[0].startswith(SKIPPED):
                paragraphs.append(" ".join(current))
            current = []
    return "\n\n".join(paragraphs)


def pronounce(text: str) -> str:
    for pattern, said in PRONUNCIATION:
        text = re.sub(pattern, said, text)
    return text


def notes(markdown: str) -> list[tuple[int, str, str]]:
    """(slide number, title, spoken text) for every slide that has a "Cómo leerla" note."""
    markdown = markdown.lstrip("﻿")  # an editor's byte order mark would hide the front matter
    body = re.sub(r"\A---\n.*?\n---\n", "", markdown, count=1, flags=re.S)  # Marp front matter
    slides = [s for s in re.split(r"^---\s*$", body, flags=re.M) if s.strip()]
    found = []
    for number, slide in enumerate(slides, 1):
        note = NOTE.search(slide)
        if note:
            heading = re.search(r"^#{1,2}\s+(.+)$", slide, re.M)
            title = re.sub(r"[*_`]", "", heading.group(1)).strip() if heading else ""
            found.append((number, title, spoken(note.group(1))))
    return found


async def synthesize(text: str, voice: str, path: Path) -> None:
    import edge_tts  # only needed to generate, not to test the text handling

    for attempt in range(3):
        try:
            await edge_tts.Communicate(text, voice).save(str(path))
            return
        except Exception:  # the online voice can drop a request: retry twice, then fail
            if attempt == 2:
                raise
            await asyncio.sleep(2)


async def generate(slides: Path, out: Path, voice: str) -> None:
    out.mkdir(parents=True, exist_ok=True)
    for old in out.glob("slide-*.mp3"):
        old.unlink()
    rows, total = [], 0.0
    for number, title, text in notes(slides.read_text(encoding="utf-8")):
        if not text:  # only directions for the presenter: nothing to read, and the voice rejects empty text
            print(f"slide {number:02}: sin texto para leer, se omite")
            continue
        path = out / f"slide-{number:02}.mp3"
        await synthesize(pronounce(text), voice, path)
        seconds = path.stat().st_size * 8 / BITRATE
        total += seconds
        words = len(re.findall(r"\w+", text))
        rows.append(f"| {number} | {title} | {words} | {seconds:.0f} s | {total / 60:.1f} min | {words / seconds * 60:.0f} |")
        print(f"slide {number:02} {seconds:6.1f} s  total {total / 60:5.1f} min  {title}")
    table = ["| Slide | Título | Palabras | Duración | Acumulado | Palabras/min |", "|---|---|---|---|---|---|"]
    (out / "duraciones.md").write_text("\n".join([f"Voz: {voice}", "", *table, *rows]) + "\n", encoding="utf-8")
    print(f"total {total / 60:.1f} min")


def main() -> None:
    sys.stdout.reconfigure(encoding="utf-8")  # slide titles have accents; the Windows console default mangles them
    parser = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    parser.add_argument("--slides", type=Path, default=ROOT / "docs/methodology/slides.md")
    parser.add_argument("--out", type=Path, default=ROOT / "docs/methodology/audio")
    parser.add_argument("--voice", default="es-ES-AlvaroNeural")
    args = parser.parse_args()
    asyncio.run(generate(args.slides, args.out, args.voice))


if __name__ == "__main__":
    main()
