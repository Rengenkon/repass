use super::{
    DEFAULT_SEPARATOR, PartCount, SeparationRequirement, Separator, find_content_chars,
    validate_parts,
};
use crate::error::SeparatorError;

#[derive(Debug)]
pub struct FixedIntervalSeparator<'a> {
    separator: &'a str,
    separator_chars: usize,
    interval_chars: usize,
}

impl<'a> FixedIntervalSeparator<'a> {
    pub fn new(separator: &'a str, interval_chars: usize) -> Result<Self, SeparatorError> {
        let separator = Self {
            separator,
            separator_chars: separator.chars().count(),
            interval_chars,
        };
        separator.validate()?;
        Ok(separator)
    }
}

impl Default for FixedIntervalSeparator<'_> {
    fn default() -> Self {
        Self {
            separator: DEFAULT_SEPARATOR,
            separator_chars: DEFAULT_SEPARATOR.chars().count(),
            interval_chars: 5,
        }
    }
}

impl Separator for FixedIntervalSeparator<'_> {
    fn validate(&self) -> Result<(), SeparatorError> {
        if self.separator.is_empty() {
            return Err(SeparatorError::EmptySeparator);
        }
        if self.interval_chars == 0 {
            return Err(SeparatorError::ZeroInterval);
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
        let mut consumed = 0usize;
        for part in parts {
            for ch in part.chars() {
                output.push(ch);
                consumed += 1;
                if consumed % self.interval_chars == 0 && consumed < content_chars {
                    output.push_str(self.separator);
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

impl FixedIntervalSeparator<'_> {
    fn output_chars_validated(&self, content_chars: usize) -> Result<usize, SeparatorError> {
        let count = content_chars.saturating_sub(1) / self.interval_chars;
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
    fn inserts_at_unicode_scalar_boundaries() {
        let separator = FixedIntervalSeparator::new("🟠", 2).unwrap();
        let parts = ["é🦀", "猫x"];
        let result = separator.separate(&parts).unwrap();
        assert_eq!(result, "é🦀🟠猫x");
        assert_eq!(
            result.chars().count(),
            separator.separated_chars(&parts).unwrap()
        );
    }

    #[test]
    fn invalid_interval_is_reported() {
        assert!(matches!(
            FixedIntervalSeparator::new("-", 0),
            Err(SeparatorError::ZeroInterval)
        ));
    }

    #[test]
    fn output_chars_matches_rendered_output_across_boundaries() {
        for interval in [1, 2, 3, 5] {
            let separator = FixedIntervalSeparator::new("::", interval).unwrap();
            for input in ["a", "abc", "abcdef", "é🦀猫x"] {
                let parts = [input];
                assert_eq!(
                    separator.separate(&parts).unwrap().chars().count(),
                    separator.separated_chars(&parts).unwrap()
                );
            }
        }
    }

    #[test]
    fn writes_into_existing_buffer_without_replacing_prefix() {
        let separator = FixedIntervalSeparator::new("-", 2).unwrap();
        let mut output = String::from("prefix:");
        separator.write_separated(&["abcd"], &mut output).unwrap();
        assert_eq!(output, "prefix:ab-cd");
    }
}
