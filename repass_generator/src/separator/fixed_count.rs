use super::{
    DEFAULT_SEPARATOR, PartCount, SeparationRequirement, Separator, find_content_chars,
    validate_parts,
};
use crate::error::SeparatorError;

#[derive(Debug)]
pub struct FixedCountSeparator<'a> {
    separator: &'a str,
    separator_chars: usize,
    separator_count: usize,
}

impl<'a> FixedCountSeparator<'a> {
    pub fn new(separator: &'a str, separator_count: usize) -> Result<Self, SeparatorError> {
        let separator = Self {
            separator,
            separator_chars: separator.chars().count(),
            separator_count,
        };
        separator.validate()?;
        Ok(separator)
    }
}

impl Default for FixedCountSeparator<'_> {
    fn default() -> Self {
        Self {
            separator: DEFAULT_SEPARATOR,
            separator_chars: DEFAULT_SEPARATOR.chars().count(),
            separator_count: 3,
        }
    }
}

impl Separator for FixedCountSeparator<'_> {
    fn validate(&self) -> Result<(), SeparatorError> {
        if self.separator.is_empty() {
            return Err(SeparatorError::EmptySeparator);
        }
        if self.separator_count == 0 {
            return Err(SeparatorError::ZeroSeparatorCount);
        }
        Ok(())
    }

    fn requirement_for_output(
        &self,
        target_chars: usize,
    ) -> Result<SeparationRequirement, SeparatorError> {
        self.validate()?;
        let Some(content_chars) = find_content_chars(target_chars, |content_chars| {
            self.output_chars_validated(content_chars)
        })?
        else {
            return Ok(SeparationRequirement::Impossible { target_chars });
        };
        Ok(SeparationRequirement::FixedContent {
            content_chars,
            part_count: PartCount::Range {
                min: 1,
                max: content_chars,
            },
        })
    }

    fn separate(&self, parts: &[&str]) -> Result<String, SeparatorError> {
        let mut output = String::new();
        self.write_separated(parts, &mut output)?;
        Ok(output)
    }

    fn write_separated(&self, parts: &[&str], output: &mut String) -> Result<(), SeparatorError> {
        self.validate()?;
        validate_parts(parts)?;
        let content_chars = super::content_chars_of(parts);
        let separator_count = self.separator_count.min(content_chars.saturating_sub(1));
        let mut next_separator = 1;
        let mut consumed = 0usize;
        for part in parts {
            for ch in part.chars() {
                output.push(ch);
                consumed += 1;
                if next_separator <= separator_count
                    && consumed
                        == ((next_separator as u128 * content_chars as u128)
                            / (separator_count as u128 + 1))
                            .max(next_separator as u128) as usize
                {
                    output.push_str(self.separator);
                    next_separator += 1;
                }
            }
        }
        Ok(())
    }

    fn output_chars(
        &self,
        content_chars: usize,
        _part_count: usize,
    ) -> Result<usize, SeparatorError> {
        self.validate()?;
        self.output_chars_validated(content_chars)
    }
}

impl FixedCountSeparator<'_> {
    fn output_chars_validated(&self, content_chars: usize) -> Result<usize, SeparatorError> {
        let count = self.separator_count.min(content_chars.saturating_sub(1));
        content_chars
            .checked_add(
                self.separator_chars
                    .checked_mul(count)
                    .ok_or(SeparatorError::CharacterCountOverflow)?,
            )
            .ok_or(SeparatorError::CharacterCountOverflow)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fixed_count_handles_unicode_and_multichar_separator() {
        let separator = FixedCountSeparator::new("🟠-", 2).unwrap();
        let parts = ["猫abcé"];
        let output = separator.separate(&parts).unwrap();
        assert_eq!(output, "猫🟠-ab🟠-cé");
        assert_eq!(
            output.chars().count(),
            separator.separated_chars(&parts).unwrap()
        );
    }

    #[test]
    fn invalid_count_is_an_error() {
        assert!(matches!(
            FixedCountSeparator::new("-", 0),
            Err(SeparatorError::ZeroSeparatorCount)
        ));
    }

    #[test]
    fn output_chars_matches_rendered_output_for_short_and_long_inputs() {
        for count in [1, 2, 4, 9] {
            let separator = FixedCountSeparator::new("::", count).unwrap();
            for input in ["a", "abc", "abcdef", "é🦀猫x"] {
                let parts = [input];
                assert_eq!(
                    separator.separate(&parts).unwrap().chars().count(),
                    separator.separated_chars(&parts).unwrap()
                );
            }
        }
    }
}
