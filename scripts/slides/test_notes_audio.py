"""Tests for notes_audio.py: run with `python -m unittest discover -s scripts/slides`."""

import asyncio
import tempfile
import unittest
from pathlib import Path
from unittest import mock

import notes_audio

DECK = """---
marp: true
---

<!-- _class: lead -->

# Primera slide

<!--
Cómo leerla:

Primera slide.

Texto que se
lee en voz alta.

▸ Mostrar: docs/prd/chicago-crime/.

⏱ Al terminar esta slide: hacia el minuto 1.
-->

---

## Segunda **slide**

- Un punto

<!--
Cómo leerla:

Segunda slide. Se dice con git.
-->
"""


class SpokenTest(unittest.TestCase):
    def test_presenter_lines_are_not_read(self):
        note = "\nPrimera.\n\n▸ Mostrar: odd/tasks/crime-query.md.\n\n⏱ Al terminar esta slide: hacia el minuto 3.\n"
        self.assertEqual(notes_audio.spoken(note), "Primera.")

    def test_wrapped_lines_join_and_paragraphs_stay_apart(self):
        note = "Una línea\npartida en dos.\n\nOtro párrafo."
        self.assertEqual(notes_audio.spoken(note), "Una línea partida en dos.\n\nOtro párrafo.")

    def test_a_wrapped_direction_is_not_read(self):
        note = "Texto.\n\n▸ Mostrar: una indicación larga\nque sigue en otra línea.\n\nMás texto."
        self.assertEqual(notes_audio.spoken(note), "Texto.\n\nMás texto.")

    def test_a_direction_inside_a_paragraph_is_not_read(self):
        note = "Texto que se lee.\n▸ Mostrar: algo\nque sigue.\n⏱ Al terminar esta slide: hacia el minuto 3.\n\nMás texto."
        self.assertEqual(notes_audio.spoken(note), "Texto que se lee.\n\nMás texto.")


class PronounceTest(unittest.TestCase):
    def test_table_words_are_spelled_as_spoken(self):
        cases = {
            "git worktree": "guit uórktri",
            "Git y los worktrees": "guit y los uórktris",
            "un commit, dos commits": "un cómit, dos cómits",
            "93 MiB de pico": "93 mebibytes de pico",
            "el skill y los skills": "el esquil y los esquils",
            "un hook, dos hooks": "un juk, dos juks",
            "una SSTable, dos SSTables": "una ese ese table, dos ese ese tables",
            "hace auditable el trabajo": "hace auditáble el trabajo",
            "CLAUDE.md y AGENTS.md": "Claude punto eme de y Agents punto eme de",
        }
        for written, said in cases.items():
            with self.subTest(written=written):
                self.assertEqual(notes_audio.pronounce(written), said)

    def test_words_that_only_contain_a_table_word_stay(self):
        text = "GitHub, gitignore, recommit y hookshot"
        self.assertEqual(notes_audio.pronounce(text), text)


class NotesTest(unittest.TestCase):
    def test_every_slide_gives_its_number_title_and_spoken_text(self):
        self.assertEqual(
            notes_audio.notes(DECK),
            [
                (1, "Primera slide", "Primera slide.\n\nTexto que se lee en voz alta."),
                (2, "Segunda slide", "Segunda slide. Se dice con git."),
            ],
        )

    def test_a_slide_without_notes_is_left_out(self):
        deck = DECK + "\n---\n\n## Sin notas\n"
        self.assertEqual([number for number, _, _ in notes_audio.notes(deck)], [1, 2])

    def test_a_byte_order_mark_does_not_shift_the_numbers(self):
        self.assertEqual(notes_audio.notes("\ufeff" + DECK), notes_audio.notes(DECK))


class GenerateTest(unittest.TestCase):
    ONLY_DIRECTIONS = "\n---\n\n## Solo indicaciones\n\n<!--\nCómo leerla:\n\n▸ Mostrar: algo.\n-->\n"

    def run_generate(self, deck: str, out: Path) -> list[str]:
        """Runs generate() with a stub voice that writes one second of audio; returns the texts it was given."""
        said = []

        async def one_second(text, voice, path):
            said.append(text)
            path.write_bytes(b"\0" * (notes_audio.BITRATE // 8))

        slides = out / "slides.md"
        slides.write_text(deck, encoding="utf-8")
        with mock.patch.object(notes_audio, "synthesize", one_second), mock.patch("builtins.print"):
            asyncio.run(notes_audio.generate(slides, out / "audio", "voz"))
        return said

    def test_one_file_per_slide_with_notes_and_the_durations_table(self):
        with tempfile.TemporaryDirectory() as tmp:
            out = Path(tmp)
            (out / "audio").mkdir()
            (out / "audio" / "slide-09.mp3").write_bytes(b"old")
            said = self.run_generate(DECK, out)
            files = sorted(p.name for p in (out / "audio").iterdir())
            table = (out / "audio" / "duraciones.md").read_text(encoding="utf-8")
        self.assertEqual(said, ["Primera slide.\n\nTexto que se lee en voz alta.", "Segunda slide. Se dice con guit."])
        self.assertEqual(files, ["duraciones.md", "slide-01.mp3", "slide-02.mp3"])
        self.assertIn("| 1 | Primera slide | 9 | 1 s | 0.0 min | 540 |", table)
        self.assertIn("| 2 | Segunda slide | 6 | 1 s | 0.0 min | 360 |", table)

    def test_a_note_with_only_directions_is_skipped(self):
        with tempfile.TemporaryDirectory() as tmp:
            said = self.run_generate(DECK + self.ONLY_DIRECTIONS, Path(tmp))
            files = sorted(p.name for p in (Path(tmp) / "audio").iterdir())
        self.assertEqual(len(said), 2)
        self.assertNotIn("slide-03.mp3", files)


if __name__ == "__main__":
    unittest.main()
