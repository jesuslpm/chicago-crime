//! The outcome of a verification: one line per check, PASS or FAIL.

use std::fmt::Write as _;

/// One check of the verification.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Check {
    /// What the check proves, such as `AC-G1` or `AC-G4-date`; stable, so scripts can look for it.
    pub criterion: String,
    pub passed: bool,
    /// One line with the figures behind the outcome.
    pub summary: String,
    /// Further lines, such as the first mismatches found.
    pub details: Vec<String>,
}

impl Check {
    pub fn new(criterion: impl Into<String>, passed: bool, summary: impl Into<String>) -> Self {
        Self {
            criterion: criterion.into(),
            passed,
            summary: summary.into(),
            details: Vec::new(),
        }
    }

    pub fn with_details(mut self, details: Vec<String>) -> Self {
        self.details = details;
        self
    }
}

/// Every check, in the order they ran.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Report {
    pub checks: Vec<Check>,
}

impl Report {
    /// Whether there is at least one check and every check passed.
    pub fn passed(&self) -> bool {
        !self.checks.is_empty() && self.checks.iter().all(|check| check.passed)
    }

    /// The check named `criterion`, if it ran.
    pub fn check(&self, criterion: &str) -> Option<&Check> {
        self.checks
            .iter()
            .find(|check| check.criterion == criterion)
    }

    /// One line per check (`PASS  <criterion>  <summary>`), its details indented below it, and a
    /// final result line.
    pub fn render(&self) -> String {
        let mut text = String::new();
        for check in &self.checks {
            let outcome = if check.passed { "PASS" } else { "FAIL" };
            // Writing to a String cannot fail.
            let _ = writeln!(text, "{outcome}  {}  {}", check.criterion, check.summary);
            for detail in &check.details {
                let _ = writeln!(text, "        {detail}");
            }
        }
        let failed = self.checks.iter().filter(|check| !check.passed).count();
        let _ = if self.passed() {
            writeln!(text, "RESULT: PASS ({} checks)", self.checks.len())
        } else {
            writeln!(
                text,
                "RESULT: FAIL ({failed} of {} checks failed)",
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
        assert!(report.check("AC-G3").is_some_and(|check| !check.passed));
        assert!(report.check("AC-G9").is_none());
    }
}
