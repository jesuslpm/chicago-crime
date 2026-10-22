//! New case numbers for the copies: same first two characters, digits after them, never a value
//! that appears in the original or was already handed out.
//!
//! A case number splits into its *prefix*, the first two characters, and its *rest*. Its *space*
//! is the prefix plus the width of the rest when the rest is only ASCII digits: `JA100001` lives
//! in the space (`JA`, 6), whose values are `JA000000` to `JA999999`. Every original case number
//! is marked as used in its space during the first pass; the copies then take free values.
//!
//! A copy takes a free value in the space of its original, so `JA100001` gives another `JA` and
//! six digits. A rest that is not only digits, or is empty, counts as the standard width of six
//! (`JA000001-B` gives `JA` and six digits); a case number shorter than two characters is padded
//! with `0` to a two-character prefix (`X` gives `X0` and six digits). When the space of that width has no free value left,
//! the copy takes one in the next wider space of the same prefix (`HH` and seven digits), which
//! the original cannot contain unless it already used that width, in which case those values are
//! marked too. Only when every width up to [`MAX_WIDTH`] is full does the build stop.

use std::borrow::Cow;
use std::collections::{BTreeMap, HashMap, HashSet};

/// The width of the digits of a standard case number: two letters and six digits.
pub const STANDARD_WIDTH: u32 = 6;

/// The widest run of digits a copy may get.
pub const MAX_WIDTH: u32 = 9;

/// Spaces up to this width keep a bitset (10,000,000 bits, 1.25 MB); wider ones keep the used
/// values in a set, because the original has few of them.
const MAX_BITSET_WIDTH: u32 = 7;

/// The used values of one space.
enum Used {
    Bits(Vec<u64>),
    Sparse(HashSet<u64>),
}

struct Space {
    width: u32,
    used: Used,
    /// Every value below it is used.
    cursor: u64,
}

impl Space {
    fn new(width: u32) -> Self {
        let used = if width <= MAX_BITSET_WIDTH {
            let words = capacity(width).div_ceil(64);
            Used::Bits(vec![
                0;
                usize::try_from(words)
                    .expect("a bitset of at most 1.25 MB")
            ])
        } else {
            Used::Sparse(HashSet::new())
        };
        Self {
            width,
            used,
            cursor: 0,
        }
    }

    fn mark(&mut self, value: u64) {
        match &mut self.used {
            Used::Bits(bits) => bits[(value / 64) as usize] |= 1 << (value % 64),
            Used::Sparse(set) => {
                set.insert(value);
            }
        }
    }

    fn is_used(&self, value: u64) -> bool {
        match &self.used {
            Used::Bits(bits) => bits[(value / 64) as usize] & (1 << (value % 64)) != 0,
            Used::Sparse(set) => set.contains(&value),
        }
    }

    /// The lowest free value, now marked as used; `None` when the space is full.
    fn take(&mut self) -> Option<u64> {
        let capacity = capacity(self.width);
        while self.cursor < capacity && self.is_used(self.cursor) {
            self.cursor += 1;
        }
        if self.cursor == capacity {
            return None;
        }
        let value = self.cursor;
        self.mark(value);
        self.cursor += 1;
        Some(value)
    }
}

/// The number of values of a space: `10^width`.
fn capacity(width: u32) -> u64 {
    10u64.pow(width)
}

/// The prefix and rest of a case number: the first two characters and everything after them. A
/// case number shorter than two characters is its whole prefix, padded with `0` to two
/// characters, so that every value handed out splits back into the space it came from.
fn split(case_number: &str) -> (Cow<'_, str>, &str) {
    match case_number.char_indices().nth(2) {
        Some((end, _)) => {
            let (prefix, rest) = case_number.split_at(end);
            (Cow::Borrowed(prefix), rest)
        }
        None if case_number.chars().count() == 2 => (Cow::Borrowed(case_number), ""),
        None => {
            let padding = 2 - case_number.chars().count();
            (
                Cow::Owned(format!("{case_number}{}", "0".repeat(padding))),
                "",
            )
        }
    }
}

/// The width and value of a rest made only of ASCII digits, up to [`MAX_WIDTH`] digits.
fn numeric(rest: &str) -> Option<(u32, u64)> {
    let width = u32::try_from(rest.len()).ok()?;
    if !(1..=MAX_WIDTH).contains(&width) || !rest.bytes().all(|byte| byte.is_ascii_digit()) {
        return None;
    }
    Some((width, rest.parse().ok()?))
}

/// Every prefix ran out of free values up to [`MAX_WIDTH`] digits.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Exhausted {
    pub prefix: String,
}

/// The used case numbers, by prefix and width.
pub struct CaseNumbers {
    max_width: u32,
    prefixes: HashMap<String, BTreeMap<u32, Space>>,
}

impl Default for CaseNumbers {
    fn default() -> Self {
        Self::with_max_width(MAX_WIDTH)
    }
}

impl CaseNumbers {
    /// Allocation stops at `max_width` digits; the tests use a small one to run out quickly.
    pub fn with_max_width(max_width: u32) -> Self {
        Self {
            max_width: max_width.min(MAX_WIDTH),
            prefixes: HashMap::new(),
        }
    }

    fn space(&mut self, prefix: &str, width: u32) -> &mut Space {
        if !self.prefixes.contains_key(prefix) {
            self.prefixes.insert(prefix.to_owned(), BTreeMap::new());
        }
        self.prefixes
            .get_mut(prefix)
            .expect("inserted above")
            .entry(width)
            .or_insert_with(|| Space::new(width))
    }

    /// Marks a case number of the original as used. One that cannot collide with any value the
    /// copies get, because its rest is not a run of digits, needs nothing.
    pub fn mark(&mut self, case_number: &str) {
        let (prefix, rest) = split(case_number);
        if let Some((width, value)) = numeric(rest) {
            self.space(&prefix, width).mark(value);
        }
    }

    /// A new case number for a copy of `case_number`, never handed out before.
    pub fn allocate(&mut self, case_number: &str) -> Result<String, Exhausted> {
        let (prefix, rest) = split(case_number);
        let first = numeric(rest).map_or(STANDARD_WIDTH, |(width, _)| width);
        for width in first..=self.max_width {
            if let Some(value) = self.space(&prefix, width).take() {
                return Ok(format!("{prefix}{value:0width$}", width = width as usize));
            }
        }
        Err(Exhausted {
            prefix: prefix.into_owned(),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_copy_keeps_the_prefix_and_width_and_skips_used_values() {
        let mut numbers = CaseNumbers::default();
        numbers.mark("JA000000");
        numbers.mark("JA000002");
        assert_eq!(numbers.allocate("JA000002").unwrap(), "JA000001");
        assert_eq!(numbers.allocate("JA000000").unwrap(), "JA000003");
        assert_eq!(numbers.allocate("HY123").unwrap(), "HY000");
    }

    #[test]
    fn a_rest_that_is_not_digits_gets_the_standard_width() {
        let mut numbers = CaseNumbers::default();
        numbers.mark("JA000001-B");
        assert_eq!(numbers.allocate("JA000001-B").unwrap(), "JA000000");
        assert_eq!(numbers.allocate("X").unwrap(), "X0000000");
        assert_eq!(numbers.allocate("").unwrap(), "00000000");
        assert_eq!(numbers.allocate("ñá-1").unwrap(), "ñá000000");
    }

    #[test]
    fn a_full_space_moves_to_the_next_width_without_colliding() {
        let mut numbers = CaseNumbers::with_max_width(2);
        for value in 0..10 {
            numbers.mark(&format!("AB{value}"));
        }
        numbers.mark("AB00");
        assert_eq!(numbers.allocate("AB5").unwrap(), "AB01");
    }

    #[test]
    fn every_width_full_is_an_error() {
        let mut numbers = CaseNumbers::with_max_width(1);
        for value in 0..9 {
            numbers.mark(&format!("AB{value}"));
        }
        assert_eq!(numbers.allocate("AB1").unwrap(), "AB9");
        assert_eq!(
            numbers.allocate("AB1"),
            Err(Exhausted {
                prefix: "AB".to_owned()
            })
        );
    }

    #[test]
    fn values_wider_than_the_bitsets_are_tracked_too() {
        let mut numbers = CaseNumbers::default();
        numbers.mark("HX00000000");
        assert_eq!(numbers.allocate("HX00000000").unwrap(), "HX00000001");
    }
}
