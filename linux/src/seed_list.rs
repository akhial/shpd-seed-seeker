// SPDX-License-Identifier: GPL-3.0-or-later

//! Portable plain-text seed-list parsing, formatting, and code filtering.

use std::collections::HashSet;
use std::fmt;

use shpd_seedfinder_core::seed::DungeonSeed;

pub const MAX_SEEDS: usize = 1_024;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SeedListErrorKind {
    InvalidCode,
    InvalidLineEnding,
    TooManySeeds,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SeedListError {
    pub line: usize,
    pub kind: SeedListErrorKind,
}

impl fmt::Display for SeedListError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self.kind {
            SeedListErrorKind::InvalidCode => write!(
                formatter,
                "line {} is not a canonical seed code (expected AAA-AAA-AAA)",
                self.line
            ),
            SeedListErrorKind::InvalidLineEnding => write!(
                formatter,
                "line {} contains a bare carriage return; use LF or CRLF line endings",
                self.line
            ),
            SeedListErrorKind::TooManySeeds => write!(
                formatter,
                "line {} exceeds the limit of {MAX_SEEDS} unique seeds",
                self.line
            ),
        }
    }
}

impl std::error::Error for SeedListError {}

/// Parses one canonical seed per line. A UTF-8 BOM is accepted only at the
/// beginning of the document; lowercase codes are normalized to uppercase.
/// Blank lines are ignored and duplicate seeds keep their first position.
///
/// # Errors
///
/// Returns the offending line when an ending is malformed, a nonblank line is
/// not canonical, or the document contains more than [`MAX_SEEDS`] unique
/// seeds.
pub fn parse(contents: &str) -> Result<Vec<DungeonSeed>, SeedListError> {
    let mut seeds = Vec::new();
    let mut seen = HashSet::new();

    for (index, original) in contents.lines().enumerate() {
        let line_number = index + 1;
        let line = if index == 0 {
            original.strip_prefix('\u{feff}').unwrap_or(original)
        } else {
            original
        };
        // `str::lines` removes CR only when it is paired with LF, so any CR
        // left here is a bare carriage return and violates the file format.
        if line.contains('\r') {
            return Err(SeedListError {
                line: line_number,
                kind: SeedListErrorKind::InvalidLineEnding,
            });
        }
        if line.trim().is_empty() {
            continue;
        }
        if !is_seed_code(line) {
            return Err(SeedListError {
                line: line_number,
                kind: SeedListErrorKind::InvalidCode,
            });
        }

        let normalized = line.to_ascii_uppercase();
        let dungeon_seed = DungeonSeed::from_code(&normalized).map_err(|_| SeedListError {
            line: line_number,
            kind: SeedListErrorKind::InvalidCode,
        })?;
        if seen.insert(dungeon_seed) {
            if seeds.len() == MAX_SEEDS {
                return Err(SeedListError {
                    line: line_number,
                    kind: SeedListErrorKind::TooManySeeds,
                });
            }
            seeds.push(dungeon_seed);
        }
    }
    Ok(seeds)
}

/// Formats canonical codes with one LF-terminated seed per line.
#[must_use]
pub fn format(seeds: &[DungeonSeed]) -> String {
    let mut output = seeds
        .iter()
        .map(|seed| seed.to_code())
        .collect::<Vec<_>>()
        .join("\n");
    if !output.is_empty() {
        output.push('\n');
    }
    output
}

/// Case-insensitive substring matching that treats dashes and ASCII whitespace
/// as presentation-only in both the seed and the user's search text.
#[must_use]
pub fn matches_search(seed: DungeonSeed, search: &str) -> bool {
    let needle = normalized_search(search);
    needle.is_empty() || normalized_search(&seed.to_code()).contains(&needle)
}

fn is_seed_code(value: &str) -> bool {
    let bytes = value.as_bytes();
    bytes.len() == 11
        && bytes[3] == b'-'
        && bytes[7] == b'-'
        && bytes
            .iter()
            .enumerate()
            .all(|(index, byte)| index == 3 || index == 7 || byte.is_ascii_alphabetic())
}

fn normalized_search(value: &str) -> String {
    value
        .chars()
        .filter(|character| *character != '-' && !character.is_ascii_whitespace())
        .flat_map(char::to_uppercase)
        .collect()
}

#[cfg(test)]
mod tests {
    use shpd_seedfinder_core::seed::DungeonSeed;

    use super::{MAX_SEEDS, SeedListErrorKind, format, matches_search, parse};

    #[test]
    fn parses_bom_crlf_lowercase_blanks_and_duplicates() {
        let seeds = parse("\u{feff}aaa-aaa-aaa\r\n\r\nABC-def-GHI\r\naaa-aaa-aaa\r\n").unwrap();
        assert_eq!(
            seeds.iter().map(|seed| seed.to_code()).collect::<Vec<_>>(),
            ["AAA-AAA-AAA", "ABC-DEF-GHI"]
        );
        assert_eq!(format(&seeds), "AAA-AAA-AAA\nABC-DEF-GHI\n");
    }

    #[test]
    fn rejects_noncanonical_lines_transactionally() {
        let error = parse("AAA-AAA-AAA\nABCDEFGHI\nBBB-BBB-BBB\n").unwrap_err();
        assert_eq!(error.line, 2);
        assert_eq!(error.kind, SeedListErrorKind::InvalidCode);
        assert!(parse(" AAA-AAA-AAA\n").is_err());
        assert!(parse("AAA-AAA-AA0\n").is_err());
        assert!(parse("AAA-AAA-AAA\rBBB-BBB-BBB").is_err());
        assert!(parse("AAA-AAA-AAA\r").is_err());
        assert_eq!(
            parse("\r").unwrap_err().kind,
            SeedListErrorKind::InvalidLineEnding
        );
    }

    #[test]
    fn enforces_the_unique_seed_limit() {
        let mut input = String::new();
        for value in 0..=MAX_SEEDS {
            input.push_str(
                &DungeonSeed::new(u64::try_from(value).unwrap())
                    .unwrap()
                    .to_code(),
            );
            input.push('\n');
        }
        let error = parse(&input).unwrap_err();
        assert_eq!(error.line, MAX_SEEDS + 1);
        assert_eq!(error.kind, SeedListErrorKind::TooManySeeds);
    }

    #[test]
    fn code_search_is_case_insensitive_and_dash_agnostic() {
        let seed = DungeonSeed::from_code("ABC-DEF-GHI").unwrap();
        assert!(matches_search(seed, "c-de"));
        assert!(matches_search(seed, "abcdef"));
        assert!(matches_search(seed, " G H I "));
        assert!(!matches_search(seed, "XYZ"));
    }
}
