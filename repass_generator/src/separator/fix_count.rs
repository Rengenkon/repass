use super::{
    DEFAULT_SEPARATOR, PartCount, SeparationRequirement, Separator, find_content_length,
    validate_parts,
};
use crate::error::SeparatorError;

#[derive(Debug, PartialEq, Eq)]
pub enum Align {
    Left,
    Right,
}

#[derive(Debug)]
pub struct FixCountSeparator<'a> {
    separator: &'a str,
    separator_length: usize,
    count: usize,
}

impl<'a> FixCountSeparator<'a> {
    pub fn new(separator: &'a str, count: usize) -> Self {
        Self {
            separator,
            separator_length: separator.chars().count(),
            count,
        }
    }
}

impl Default for FixCountSeparator<'_> {
    fn default() -> Self {
        Self {
            separator: DEFAULT_SEPARATOR,
            separator_length: DEFAULT_SEPARATOR.chars().count(),
            count: 3,
        }
    }
}

impl Separator for FixCountSeparator<'_> {
    fn validate(&self) -> Result<(), SeparatorError> {
        if self.separator.is_empty() {
            return Err(SeparatorError::EmptySeparator);
        }
        if self.count == 0 {
            return Err(SeparatorError::ZeroCount);
        }
        Ok(())
    }

    fn requirement_for(
        &self,
        target_length: usize,
    ) -> Result<SeparationRequirement, SeparatorError> {
        self.validate()?;
        let Some(content_length) =
            find_content_length(target_length, |length| self.output_length_validated(length))?
        else {
            return Ok(SeparationRequirement::Impossible { target_length });
        };
        Ok(SeparationRequirement::FixedContent {
            content_length,
            part_count: PartCount::Range {
                min: 1,
                max: content_length,
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
        let content_length = super::get_summary_length(parts);
        let separator_count = self.count.min(content_length.saturating_sub(1));
        let mut next_separator = 1;
        let mut consumed = 0usize;
        for part in parts {
            for ch in part.chars() {
                output.push(ch);
                consumed += 1;
                if next_separator <= separator_count
                    && consumed
                        == ((next_separator as u128 * content_length as u128)
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

    fn output_length(
        &self,
        input_length: usize,
        _parts_count: usize,
    ) -> Result<usize, SeparatorError> {
        self.validate()?;
        self.output_length_validated(input_length)
    }
}

impl FixCountSeparator<'_> {
    fn output_length_validated(&self, input_length: usize) -> Result<usize, SeparatorError> {
        let count = self.count.min(input_length.saturating_sub(1));
        input_length
            .checked_add(
                self.separator_length
                    .checked_mul(count)
                    .ok_or(SeparatorError::LengthOverflow)?,
            )
            .ok_or(SeparatorError::LengthOverflow)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fixed_count_handles_unicode_and_multichar_separator() {
        let separator = FixCountSeparator::new("🟠-", 2);
        let parts = ["猫abcé"];
        let output = separator.separate(&parts).unwrap();
        assert_eq!(output, "猫🟠-ab🟠-cé");
        assert_eq!(
            output.chars().count(),
            separator.length_after_separate(&parts).unwrap()
        );
    }

    #[test]
    fn invalid_count_is_an_error() {
        assert_eq!(
            FixCountSeparator::new("-", 0).validate(),
            Err(SeparatorError::ZeroCount)
        );
    }

    #[test]
    fn output_length_matches_rendered_length_for_short_and_long_inputs() {
        for count in [1, 2, 4, 9] {
            let separator = FixCountSeparator::new("::", count);
            for input in ["a", "abc", "abcdef", "é🦀猫x"] {
                let parts = [input];
                assert_eq!(
                    separator.separate(&parts).unwrap().chars().count(),
                    separator.length_after_separate(&parts).unwrap()
                );
            }
        }
    }
}
