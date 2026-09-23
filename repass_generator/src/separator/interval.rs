use super::{
    DEFAULT_SEPARATOR, PartCount, SeparationRequirement, Separator, find_content_length,
    validate_parts,
};
use crate::error::SeparatorError;

#[derive(Debug)]
pub struct FixIntervalSeparator<'a> {
    separator: &'a str,
    interval: usize,
}

impl<'a> FixIntervalSeparator<'a> {
    pub fn new(separator: &'a str, interval: usize) -> Self {
        Self {
            separator,
            interval,
        }
    }
}

impl Default for FixIntervalSeparator<'_> {
    fn default() -> Self {
        Self {
            separator: DEFAULT_SEPARATOR,
            interval: 5,
        }
    }
}

impl Separator for FixIntervalSeparator<'_> {
    fn validate(&self) -> Result<(), SeparatorError> {
        if self.separator.is_empty() {
            return Err(SeparatorError::EmptySeparator);
        }
        if self.interval == 0 {
            return Err(SeparatorError::ZeroInterval);
        }
        Ok(())
    }

    fn requirement_for(
        &self,
        target_length: usize,
    ) -> Result<SeparationRequirement, SeparatorError> {
        self.validate()?;
        let Some(content_length) =
            find_content_length(target_length, |length| self.output_length(length, 1))?
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
        let mut consumed = 0usize;
        for part in parts {
            for ch in part.chars() {
                output.push(ch);
                consumed += 1;
                if consumed % self.interval == 0 && consumed < content_length {
                    output.push_str(self.separator);
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
        let count = input_length.saturating_sub(1) / self.interval;
        input_length
            .checked_add(
                self.separator
                    .chars()
                    .count()
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
    fn inserts_at_unicode_scalar_boundaries() {
        let separator = FixIntervalSeparator::new("🟠", 2);
        let parts = ["é🦀", "猫x"];
        let result = separator.separate(&parts).unwrap();
        assert_eq!(result, "é🦀🟠猫x");
        assert_eq!(
            result.chars().count(),
            separator.length_after_separate(&parts).unwrap()
        );
    }

    #[test]
    fn invalid_interval_is_reported() {
        assert_eq!(
            FixIntervalSeparator::new("-", 0).validate(),
            Err(SeparatorError::ZeroInterval)
        );
    }

    #[test]
    fn output_length_matches_rendered_length_across_boundaries() {
        for interval in [1, 2, 3, 5] {
            let separator = FixIntervalSeparator::new("::", interval);
            for input in ["a", "abc", "abcdef", "é🦀猫x"] {
                let parts = [input];
                assert_eq!(
                    separator.separate(&parts).unwrap().chars().count(),
                    separator.length_after_separate(&parts).unwrap()
                );
            }
        }
    }

    #[test]
    fn writes_into_existing_buffer_without_replacing_prefix() {
        let separator = FixIntervalSeparator::new("-", 2);
        let mut output = String::from("prefix:");
        separator.write_separated(&["abcd"], &mut output).unwrap();
        assert_eq!(output, "prefix:ab-cd");
    }
}
