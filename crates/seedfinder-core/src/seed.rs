//! Shattered Pixel Dungeon's seed codes and UTC daily-run identities.

use std::fmt;

/// There are exactly 26^9 user-enterable seed codes.
pub const TOTAL_SEEDS: u64 = 5_429_503_678_976;

/// A validated dungeon seed: a user-enterable code or a UTC daily run.
/// Numeric construction remains restricted to the searchable code space.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct DungeonSeed(u64);

impl DungeonSeed {
    /// The first seed, represented by `AAA-AAA-AAA`.
    pub const MIN: Self = Self(0);

    /// The final seed, represented by `ZZZ-ZZZ-ZZZ`.
    pub const MAX: Self = Self(TOTAL_SEEDS - 1);

    /// Creates a seed from its numeric representation.
    ///
    /// # Errors
    ///
    /// Returns [`SeedError::OutOfRange`] when `value >= 26^9`.
    pub const fn new(value: u64) -> Result<Self, SeedError> {
        if value < TOTAL_SEEDS {
            Ok(Self(value))
        } else {
            Err(SeedError::OutOfRange)
        }
    }

    /// Returns the numeric representation used by the game.
    #[must_use]
    pub const fn value(self) -> u64 {
        self.0
    }

    /// Parses a scout identity: a seed code or an ISO UTC daily date.
    ///
    /// # Errors
    /// Rejects malformed codes and dates outside 1970-01-01..=9999-12-31.
    pub fn from_scout_input(input: &str) -> Result<Self, SeedError> {
        if input.as_bytes().first().is_some_and(u8::is_ascii_digit) {
            Self::from_daily_date(input)
        } else {
            Self::from_code(input)
        }
    }

    /// The game's daily seed is UTC midnight in Unix milliseconds plus 26^9.
    /// See `Dungeon.initSeed` in the pinned Shattered Pixel Dungeon source.
    /// This must never be reduced modulo the user-enterable seed space.
    ///
    /// # Errors
    /// Requires a real Gregorian date in 1970-01-01..=9999-12-31, as YYYY-MM-DD.
    pub fn from_daily_date(date: &str) -> Result<Self, SeedError> {
        let bytes = date.as_bytes();
        if bytes.len() != 10
            || bytes[4] != b'-'
            || bytes[7] != b'-'
            || bytes
                .iter()
                .enumerate()
                .any(|(i, b)| i != 4 && i != 7 && !b.is_ascii_digit())
        {
            return Err(SeedError::InvalidDate);
        }
        let number = |part: &[u8]| part.iter().fold(0_u64, |n, b| n * 10 + u64::from(b - b'0'));
        let year = number(&bytes[..4]);
        let month = number(&bytes[5..7]);
        let day = number(&bytes[8..]);
        if !(1970..=9999).contains(&year)
            || !(1..=12).contains(&month)
            || !(1..=month_days(year, month)).contains(&day)
        {
            return Err(SeedError::InvalidDate);
        }
        let days = days_before_year(year) - days_before_year(1970)
            + (1..month).map(|m| month_days(year, m)).sum::<u64>()
            + day
            - 1;
        Ok(Self(TOTAL_SEEDS + days * 86_400_000))
    }

    /// Selects the UTC daily run containing a Unix timestamp in seconds.
    ///
    /// # Errors
    /// Rejects timestamps after 9999-12-31.
    pub fn daily_at_unix_seconds(seconds: u64) -> Result<Self, SeedError> {
        let days = seconds / 86_400;
        if days >= days_before_year(10_000) - days_before_year(1970) {
            return Err(SeedError::InvalidDate);
        }
        Ok(Self(TOTAL_SEEDS + days * 86_400_000))
    }

    /// Whether this seed identifies a daily run rather than a seed code.
    #[must_use]
    pub const fn is_daily(self) -> bool {
        self.0 >= TOTAL_SEEDS
    }

    fn daily_date(self) -> String {
        let days = (self.0 - TOTAL_SEEDS) / 86_400_000 + days_before_year(1970);
        let (mut low, mut high) = (1970, 10_000);
        while low + 1 < high {
            let middle = u64::midpoint(low, high);
            if days_before_year(middle) <= days {
                low = middle;
            } else {
                high = middle;
            }
        }
        let mut remaining = days - days_before_year(low);
        let mut month = 1;
        while remaining >= month_days(low, month) {
            remaining -= month_days(low, month);
            month += 1;
        }
        format!("{low:04}-{month:02}-{:02}", remaining + 1)
    }

    /// Parses the game's `XXX-XXX-XXX` form.
    ///
    /// Dashes and Unicode whitespace are ignored, matching the Java helper.
    /// Properly dashed eleven-character codes are accepted case-insensitively;
    /// undashed codes must already use uppercase ASCII, matching the upstream
    /// implementation's somewhat unusual behavior.
    ///
    /// # Errors
    ///
    /// Returns [`SeedError::InvalidCode`] unless exactly nine `A..=Z` digits
    /// remain after separators are removed.
    pub fn from_code(input: &str) -> Result<Self, SeedError> {
        let utf16: Vec<u16> = input.encode_utf16().collect();
        let properly_dashed = utf16.len() == 11
            && utf16.get(3) == Some(&u16::from(b'-'))
            && utf16.get(7) == Some(&u16::from(b'-'));

        let normalized = if properly_dashed {
            input.to_uppercase()
        } else {
            input.to_owned()
        };
        let digits: Vec<char> = normalized
            .chars()
            .filter(|character| *character != '-' && !java_regex_whitespace(*character))
            .collect();

        if digits.len() != 9 || digits.iter().any(|digit| !digit.is_ascii_uppercase()) {
            return Err(SeedError::InvalidCode);
        }

        let mut value = 0_u64;
        for digit in digits {
            value = value * 26 + u64::from(u32::from(digit) - u32::from('A'));
        }
        Ok(Self(value))
    }

    /// Returns its canonical identity: `XXX-XXX-XXX` or a daily `YYYY-MM-DD`.
    #[must_use]
    pub fn to_code(self) -> String {
        if self.is_daily() {
            return self.daily_date();
        }
        let mut value = self.0;
        let mut raw = [b'A'; 9];
        for digit in raw.iter_mut().rev() {
            *digit += u8::try_from(value % 26).unwrap_or_default();
            value /= 26;
        }

        let mut code = String::with_capacity(11);
        for (index, digit) in raw.into_iter().enumerate() {
            if index == 3 || index == 6 {
                code.push('-');
            }
            code.push(char::from(digit));
        }
        code
    }
}

/// Mirrors `DungeonSeed.convertFromText`, including Java UTF-16 hashing and
/// signed remainder behavior. Unlike [`DungeonSeed`], arbitrary text can map
/// to a negative `long` (notably numeric input such as `-1`).
#[must_use]
pub fn from_text(input: &str) -> i64 {
    if input.is_empty() {
        return -1;
    }
    if let Ok(seed) = DungeonSeed::from_code(input) {
        return i64::try_from(seed.value()).unwrap_or_default();
    }

    let numeric: String = input
        .chars()
        .filter(|character| !java_regex_whitespace(*character))
        .collect();
    if let Ok(value) = numeric.parse::<i64>() {
        return value % i64::try_from(TOTAL_SEEDS).unwrap_or(i64::MAX);
    }

    let mut total = 0_i64;
    for code_unit in input.encode_utf16() {
        total = total.wrapping_mul(31).wrapping_add(i64::from(code_unit));
    }
    if total < 0 {
        total = total.wrapping_add(i64::MAX);
    }
    total % i64::try_from(TOTAL_SEEDS).unwrap_or(i64::MAX)
}

/// Canonicalizes valid seed codes and leaves all other input unchanged.
#[must_use]
pub fn format_text(input: &str) -> String {
    DungeonSeed::from_code(input).map_or_else(|_| input.to_owned(), DungeonSeed::to_code)
}

/// Preserves complete UTC daily dates; masks other partial seed input into
/// uppercase groups of three.
///
/// Every byte that is not an ASCII letter is dropped, the first nine of the
/// survivors are kept, and only then are they uppercased — so non-ASCII input
/// contributes nothing, whatever case mapping its own alphabet would use. The
/// result is a prefix of a canonical `XXX-XXX-XXX` code, which
/// [`DungeonSeed::from_code`] accepts once nine letters have arrived.
#[must_use]
pub fn format_input(input: &str) -> String {
    // Date pickers and scout links supply complete dates; keep their identity.
    if DungeonSeed::from_daily_date(input).is_ok() {
        return input.to_owned();
    }
    let mut output = String::with_capacity(11);
    for (index, byte) in input
        .bytes()
        .filter(u8::is_ascii_alphabetic)
        .take(9)
        .enumerate()
    {
        if index == 3 || index == 6 {
            output.push('-');
        }
        output.push(char::from(byte.to_ascii_uppercase()));
    }
    output
}

/// Parses a seed code or UTC daily date into the bridge document
/// `{"code": "XXX-XXX-XXX", "value": <number>}`: the canonical code for
/// display and its full numeric value. Only code values are searchable. Built here so every
/// thin bridge (C, JNI, wasm) hands its frontend the identical document.
///
/// # Errors
///
/// Returns the seed parser's error for an invalid code or daily date.
#[cfg(feature = "json-query")]
pub fn parse_document(input: &str) -> Result<String, SeedError> {
    let seed = DungeonSeed::from_scout_input(input)?;
    Ok(serde_json::json!({ "code": seed.to_code(), "value": seed.value() }).to_string())
}

impl fmt::Display for DungeonSeed {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.to_code())
    }
}

/// Validation failures for seed input.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SeedError {
    /// Seed number is outside `[0, 26^9)`.
    OutOfRange,
    /// Code is not nine uppercase base-26 digits after separators are removed.
    InvalidCode,
    /// Not a real supported UTC daily date.
    InvalidDate,
}

impl fmt::Display for SeedError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::OutOfRange => formatter.write_str("seed must be in the range [0, 26^9)"),
            Self::InvalidCode => formatter.write_str("seed code must contain nine A-Z characters"),
            Self::InvalidDate => formatter.write_str(
                "daily date must be a real YYYY-MM-DD date between 1970-01-01 and 9999-12-31",
            ),
        }
    }
}

impl std::error::Error for SeedError {}

const fn days_before_year(year: u64) -> u64 {
    let previous = year - 1;
    previous * 365 + previous / 4 - previous / 100 + previous / 400
}

const fn month_days(year: u64, month: u64) -> u64 {
    match month {
        4 | 6 | 9 | 11 => 30,
        2 if year % 4 == 0 && (year % 100 != 0 || year % 400 == 0) => 29,
        2 => 28,
        _ => 31,
    }
}

// Java Pattern's default `\\s` is the ASCII set unless UNICODE_CHARACTER_CLASS
// is enabled. This is deliberately not Rust's broader `char::is_whitespace`.
const fn java_regex_whitespace(character: char) -> bool {
    matches!(
        character,
        '\t' | '\n' | '\u{000B}' | '\u{000C}' | '\r' | ' '
    )
}

#[cfg(test)]
mod tests {
    use super::{DungeonSeed, SeedError, TOTAL_SEEDS, format_input, format_text, from_text};

    #[test]
    fn daily_dates_use_utc_milliseconds_without_reducing_to_code_space() {
        // Independent Gregorian/Unix timestamp fixtures, then Dungeon.initSeed's +26^9.
        for (date, value) in [
            ("1970-01-01", 5_429_503_678_976),
            ("2000-02-29", 6_381_286_078_976),
            ("2024-02-29", 7_138_668_478_976),
            ("2026-09-25", 7_219_798_078_976),
            ("2100-03-01", 9_537_046_078_976),
            ("2400-02-29", 19_004_066_878_976),
            ("9999-12-31", 258_831_718_078_976),
        ] {
            let seed = DungeonSeed::from_daily_date(date).unwrap();
            assert_eq!(seed.value(), value, "{date}");
            assert_eq!(seed.to_code(), date);
            assert_eq!(DungeonSeed::from_scout_input(date), Ok(seed));
            assert_eq!(format_input(date), date);
            assert!(DungeonSeed::new(value).is_err());
            assert!(DungeonSeed::from_code(date).is_err());
            assert_ne!(from_text(date), i64::try_from(value).unwrap());
        }
    }

    #[test]
    fn daily_dates_reject_invalid_calendar_dates_and_noncanonical_input() {
        for date in [
            "",
            "2026-2-03",
            "2026-02-30",
            "2025-02-29",
            "2100-02-29",
            "2026-00-01",
            "2026-13-01",
            "2026-01-00",
            "2026-04-31",
            "1969-12-31",
            "10000-01-01",
            "2026-09-25Z",
            " 2026-09-25",
            "２０２６-09-25",
            "202x-09-25",
        ] {
            assert_eq!(
                DungeonSeed::from_daily_date(date),
                Err(SeedError::InvalidDate),
                "{date}"
            );
        }
    }

    #[test]
    fn daily_clock_changes_only_at_utc_midnight() {
        let date = DungeonSeed::from_daily_date("2026-09-25").unwrap();
        let midnight = (date.value() - TOTAL_SEEDS) / 1000;
        assert_eq!(
            DungeonSeed::daily_at_unix_seconds(midnight - 1)
                .unwrap()
                .to_code(),
            "2026-09-24"
        );
        assert_eq!(DungeonSeed::daily_at_unix_seconds(midnight), Ok(date));
        assert_eq!(
            DungeonSeed::daily_at_unix_seconds(midnight + 86_399),
            Ok(date)
        );
        assert_eq!(
            DungeonSeed::daily_at_unix_seconds(midnight + 86_400)
                .unwrap()
                .to_code(),
            "2026-09-26"
        );
        assert!(DungeonSeed::daily_at_unix_seconds(u64::MAX).is_err());
    }

    #[test]
    fn known_code_boundaries_match_upstream() {
        assert_eq!(DungeonSeed::MIN.to_code(), "AAA-AAA-AAA");
        assert_eq!(DungeonSeed::MAX.to_code(), "ZZZ-ZZZ-ZZZ");
        assert_eq!(DungeonSeed::from_code("AAA-AAA-AAB").unwrap().value(), 1);
        assert_eq!(
            DungeonSeed::from_code("ZZZ-ZZZ-ZZZ").unwrap().value(),
            TOTAL_SEEDS - 1
        );
    }

    #[test]
    fn code_round_trips_representative_values() {
        for value in [0, 1, 25, 26, 17_576, 123_456_789, TOTAL_SEEDS - 1] {
            let seed = DungeonSeed::new(value).unwrap();
            assert_eq!(DungeonSeed::from_code(&seed.to_code()), Ok(seed));
        }
    }

    #[test]
    fn parsing_matches_upstream_case_and_separator_rules() {
        assert_eq!(DungeonSeed::from_code("aaa-aaa-aab").unwrap().value(), 1);
        assert_eq!(
            DungeonSeed::from_code("A A A-A A A-A A B").unwrap().value(),
            1
        );
        assert_eq!(
            DungeonSeed::from_code("aaaaaaaaa"),
            Err(SeedError::InvalidCode)
        );
        assert_eq!(
            DungeonSeed::from_code("AAA-AAA-AA0"),
            Err(SeedError::InvalidCode)
        );
    }

    #[test]
    fn rejects_numeric_values_outside_user_seed_space() {
        assert_eq!(DungeonSeed::new(TOTAL_SEEDS), Err(SeedError::OutOfRange));
        assert_eq!(DungeonSeed::new(u64::MAX), Err(SeedError::OutOfRange));
    }

    #[test]
    fn arbitrary_text_matches_java_utf16_hash_fixtures() {
        assert_eq!(from_text("abc-def-ghi"), 8_687_205_886);
        assert_eq!(from_text("abcdefghi"), 4_074_933_826_149);
        assert_eq!(from_text("123 456"), 123_456);
        assert_eq!(from_text("-1"), -1);
        assert_eq!(from_text("Shattered Pixel Dungeon"), 2_149_886_743_767);
        assert_eq!(from_text("😀"), 1_772_899);
    }

    #[test]
    fn formatting_only_canonicalizes_codes() {
        assert_eq!(format_text("abc-def-ghi"), "ABC-DEF-GHI");
        assert_eq!(format_text("abcdefghi"), "abcdefghi");
    }

    #[test]
    fn masking_groups_the_first_nine_ascii_letters() {
        assert_eq!(format_input(""), "");
        assert_eq!(format_input("a"), "A");
        assert_eq!(format_input("abcD"), "ABC-D");
        assert_eq!(format_input("abc-def-ghi"), "ABC-DEF-GHI");
        assert_eq!(format_input(" 1a!b@c#d$e%f^g&h*i extra"), "ABC-DEF-GHI");
        // Every masked prefix of nine letters is a parseable canonical code.
        assert_eq!(
            DungeonSeed::from_code(&format_input("aaa aaa aab")).unwrap(),
            DungeonSeed::from_code("AAA-AAA-AAB").unwrap()
        );

        // Filtering happens before uppercasing, so non-ASCII letters are
        // dropped whatever their own case mapping would produce: Turkish
        // dotless i (U+0131) uppercases to ASCII 'I' but never reaches the
        // mask, unlike a port that uppercases the string first.
        assert_eq!(format_input("åa😀b"), "AB");
        assert_eq!(format_input("\u{131}ab"), "AB");
        assert_eq!(format_input("\u{131}"), "");
    }

    #[test]
    fn parse_documents_carry_the_canonical_code_and_value() {
        let parsed: serde_json::Value =
            serde_json::from_str(&super::parse_document("AAA-AAA-AAB").unwrap()).unwrap();
        assert_eq!(parsed["code"], "AAA-AAA-AAB");
        assert_eq!(parsed["value"], 1);

        // Non-canonical but parseable input round-trips to the canonical
        // code, and so does every nine-letter masked prefix.
        for input in ["aaa-aaa-aab", &format_input("aaaaaaaab")] {
            let document: serde_json::Value =
                serde_json::from_str(&super::parse_document(input).unwrap()).unwrap();
            assert_eq!(document, parsed, "{input}");
        }

        // Undashed lowercase is not a code by the game's own rules.
        assert!(super::parse_document("aaaaaaaab").is_err());
        assert!(super::parse_document("AAA-AAA-AA0").is_err());
    }
}
