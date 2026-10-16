"""Tests for notes_audio.py: run with `python -m unittest discover -s scripts/slides`."""

import unittest

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


if __name__ == "__main__":
    unittest.main()
