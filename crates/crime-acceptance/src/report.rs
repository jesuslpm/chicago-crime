//! The outcome of a verification: one line per check, PASS, FAIL or SKIP.

use std::fmt::Write as _;

/// What one check concluded.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Outcome {
    Pass,
    Fail,
    /// The CSV has nothing for the check to look at, such as no row in the spring-forward hour.
    /// It counts as neither PASS nor FAIL.
    Skip,
}

impl Outcome {
    fn label(self) -> &'static str {
        match self {
            Self::Pass => "PASS",
            Self::Fail => "FAIL",
            Self::Skip => "SKIP",
        }
    }
}

/// One check of the verification.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Check {
    /// What the check proves, such as `AC-G1` or `AC-G4-date`; stable, so scripts can look for it.
    pub criterion: String,
    pub outcome: Outcome,
    /// One line with the figures behind the outcome, or why the check was skipped.
    pub summary: String,
    /// Further lines, such as the first mismatches found.
    pub details: Vec<String>,
}

impl Check {
    /// A check that passed or failed.
    pub fn new(criterion: impl Into<String>, passed: bool, summary: impl Into<String>) -> Self {
        let outcome = if passed { Outcome::Pass } else { Outcome::Fail };
        Self::with_outcome(criterion, outcome, summary)
    }

    /// A check with nothing to look at; `reason` says why.
    pub fn skipped(criterion: impl Into<String>, reason: impl Into<String>) -> Self {
        Self::with_outcome(criterion, Outcome::Skip, reason)
    }

    fn with_outcome(
        criterion: impl Into<String>,
        outcome: Outcome,
        summary: impl Into<String>,
    ) -> Self {
        Self {
            criterion: criterion.into(),
            outcome,
            summary: summary.into(),
            details: Vec::new(),
        }
    }

    pub fn with_details(mut self, details: Vec<String>) -> Self {
        self.details = details;
        self
    }

    pub fn passed(&self) -> bool {
        self.outcome == Outcome::Pass
    }

    pub fn failed(&self) -> bool {
        self.outcome == Outcome::Fail
    }
}

/// Every check, in the order they ran.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Report {
    pub checks: Vec<Check>,
}

impl Report {
    /// Whether no check failed and at least one passed: a skipped check counts as neither.
    pub fn passed(&self) -> bool {
        self.checks.iter().any(Check::passed) && !self.checks.iter().any(Check::failed)
    }

    /// The check named `criterion`, if it ran.
    pub fn check(&self, criterion: &str) -> Option<&Check> {
        self.checks
            .iter()
            .find(|check| check.criterion == criterion)
    }

    /// One line per check (`PASS  <criterion>  <summary>`, or `FAIL` or `SKIP`), its details
    /// indented below it, and a final result line.
    pub fn render(&self) -> String {
        let mut text = String::new();
        for check in &self.checks {
            // Writing to a String cannot fail.
            let _ = writeln!(
                text,
                "{}  {}  {}",
                check.outcome.label(),
                check.criterion,
                check.summary
            );
            for detail in &check.details {
                let _ = writeln!(text, "        {detail}");
            }
        }
        let count = |outcome: Outcome| {
            self.checks
                .iter()
                .filter(|check| check.outcome == outcome)
                .count()
        };
        let (failed, skipped) = (count(Outcome::Fail), count(Outcome::Skip));
        let skipped_text = if skipped > 0 {
            format!(", {skipped} skipped")
        } else {
            String::new()
        };
        let _ = if self.passed() {
            writeln!(
                text,
                "RESULT: PASS ({} checks{skipped_text})",
                self.checks.len()
            )
        } else {
            writeln!(
                text,
                "RESULT: FAIL ({failed} of {} checks failed{skipped_text})",
                self.checks.len()
            )
        };
        text
    }
}

/// A count with English thousands separators: `8656904` becomes `8,656,904`.
pub fn thousands(value: u64) -> String {
    let digits = value.to_string();
    let mut formatted = String::with_capacity(digits.len() + digits.len() / 3);
    for (index, digit) in digits.chars().enumerate() {
        if index > 0 && (digits.len() - index).is_multiple_of(3) {
            formatted.push(',');
        }
        formatted.push(digit);
    }
    formatted
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn counts_use_english_thousands_separators() {
        let cases = [
            (0, "0"),
            (999, "999"),
            (1_000, "1,000"),
            (8_656_904, "8,656,904"),
        ];
        for (value, expected) in cases {
            assert_eq!(thousands(value), expected);
        }
    }

    #[test]
    fn the_report_passes_only_when_every_check_passes() {
        let mut report = Report::default();
        assert!(!report.passed(), "an empty report proves nothing");

        report
            .checks
            .push(Check::new("AC-G1", true, "8,656,904 entries"));
        assert!(report.passed());
        assert_eq!(
            report.render(),
            "PASS  AC-G1  8,656,904 entries\nRESULT: PASS (1 checks)\n"
        );

        report
            .checks
            .push(Check::new("AC-G3", false, "1 mismatch").with_details(vec!["ID 5".to_owned()]));
        assert!(!report.passed());
        assert_eq!(
            report.render(),
            "PASS  AC-G1  8,656,904 entries\nFAIL  AC-G3  1 mismatch\n        ID 5\nRESULT: FAIL (1 \
             of 2 checks failed)\n"
        );
        assert!(report.check("AC-G3").is_some_and(Check::failed));
        assert!(report.check("AC-G9").is_none());
    }

    #[test]
    fn a_skipped_check_renders_as_skip_and_neither_passes_nor_fails_the_report() {
        let mut report = Report::default();
        report.checks.push(Check::skipped(
            "AC-G3-DST",
            "the CSV has no row in the spring-forward hour",
        ));
        assert!(
            !report.passed(),
            "a report with nothing checked proves nothing"
        );

        report
            .checks
            .insert(0, Check::new("AC-G1", true, "3 entries"));
        assert!(report.passed());
        assert_eq!(
            report.render(),
            "PASS  AC-G1  3 entries\nSKIP  AC-G3-DST  the CSV has no row in the spring-forward \
             hour\nRESULT: PASS (2 checks, 1 skipped)\n"
        );

        report.checks.push(Check::new("AC-G3", false, "1 mismatch"));
        assert!(!report.passed());
        assert!(
            report
                .render()
                .ends_with("RESULT: FAIL (1 of 3 checks failed, 1 skipped)\n")
        );
    }
}
