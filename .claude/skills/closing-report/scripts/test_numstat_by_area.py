"""Tests for numstat_by_area. Run: python -m unittest discover -s .claude/skills/closing-report/scripts"""

import unittest

import numstat_by_area as nba


class AreaTests(unittest.TestCase):
    def test_crate_files_group_by_crate(self):
        self.assertEqual(nba.area_of("crates/crime-query/src/main.rs"), "crates/crime-query")

    def test_other_folders_group_by_first_segment(self):
        self.assertEqual(nba.area_of("docs/methodology/slides.md"), "docs")

    def test_root_files_are_their_own_area(self):
        self.assertEqual(nba.area_of("README.md"), "README.md")


class ParseTests(unittest.TestCase):
    def test_plain_entry(self):
        self.assertEqual(nba.parse_numstat_z("3\t1\tdocs/a.md\0"), [("3", "1", "docs/a.md")])

    def test_rename_keeps_the_full_destination_path(self):
        # With -z git writes renames as an empty path field followed by old and new paths.
        data = "5\t2\t\0crates/x/src/old.rs\0crates/x/src/new.rs\0"
        self.assertEqual(nba.parse_numstat_z(data), [("5", "2", "crates/x/src/new.rs")])

    def test_non_ascii_path_is_not_quoted(self):
        data = "1\t0\tdocs/methodology/generación.md\0"
        self.assertEqual(nba.parse_numstat_z(data), [("1", "0", "docs/methodology/generación.md")])

    def test_several_entries(self):
        data = "1\t0\ta.md\0" "-\t-\timg.png\0" "2\t2\t\0x/old.md\0x/new.md\0"
        self.assertEqual(
            nba.parse_numstat_z(data),
            [("1", "0", "a.md"), ("-", "-", "img.png"), ("2", "2", "x/new.md")],
        )


class SummaryTests(unittest.TestCase):
    def test_lockfiles_binaries_and_apart_globs_stay_out_of_totals(self):
        entries = [
            ("10", "2", "crates/a/src/lib.rs"),
            ("4", "0", "Cargo.lock"),
            ("-", "-", "docs/logo.png"),
            ("900", "0", "docs/methodology/slides.html"),
            ("3", "1", "docs/methodology/slides.md"),
        ]
        areas, apart = nba.summarize(entries, ["docs/methodology/slides.html"])
        self.assertEqual(areas["crates/a"], nba.Counts(added=10, removed=2, files=1))
        self.assertEqual(areas["docs"], nba.Counts(added=3, removed=1, files=1))
        self.assertEqual(
            [path for path, _, _ in apart],
            ["Cargo.lock", "docs/logo.png", "docs/methodology/slides.html"],
        )


class ArgsTests(unittest.TestCase):
    def test_defaults(self):
        args = nba.parse_args(["abc123"])
        self.assertEqual((args.base, args.head, args.lang, args.apart), ("abc123", "HEAD", "es", []))

    def test_a_ref_named_like_the_language_is_kept(self):
        args = nba.parse_args(["es", "en", "--lang", "en"])
        self.assertEqual((args.base, args.head, args.lang), ("es", "en", "en"))

    def test_lang_without_value_is_a_usage_error(self):
        with self.assertRaises(SystemExit) as ctx:
            nba.parse_args(["abc123", "--lang"])
        self.assertEqual(ctx.exception.code, 2)

    def test_unknown_lang_is_a_usage_error(self):
        with self.assertRaises(SystemExit) as ctx:
            nba.parse_args(["abc123", "--lang", "fr"])
        self.assertEqual(ctx.exception.code, 2)


if __name__ == "__main__":
    unittest.main()
