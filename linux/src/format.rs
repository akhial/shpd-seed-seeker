// SPDX-License-Identifier: GPL-3.0-or-later

//! Small display-formatting helpers.

/// Groups a count into thousands separated by thin non-breaking spaces.
pub fn group_digits(value: u64) -> String {
    let digits = value.to_string();
    let mut output = String::with_capacity(digits.len() + digits.len() / 3);
    let offset = digits.len() % 3;
    for (index, character) in digits.chars().enumerate() {
        if index != 0 && index % 3 == offset {
            output.push('\u{202f}');
        }
        output.push(character);
    }
    output
}

#[cfg(test)]
mod tests {
    use super::group_digits;

    #[test]
    fn groups_thousands() {
        assert_eq!(group_digits(0), "0");
        assert_eq!(group_digits(999), "999");
        assert_eq!(group_digits(1_000), "1\u{202f}000");
        assert_eq!(
            group_digits(5_429_503_678_976),
            "5\u{202f}429\u{202f}503\u{202f}678\u{202f}976"
        );
    }
}
