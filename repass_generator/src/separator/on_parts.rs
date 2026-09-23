use super::{DEFAULT_SEPARATOR, SeparationRequirement, Separator, validate_parts};
use crate::error::SeparatorError;

#[derive(Debug)]
pub struct BetweenPartsSeparator<'a> {
    separator: &'a str,
    separator_length: usize,
}

impl<'a> BetweenPartsSeparator<'a> {
    pub fn new(separator: &'a str) -> Self {
        Self {
            separator,
            separator_length: separator.chars().count(),
        }
    }
}

impl Default for BetweenPartsSeparator<'_> {
    fn default() -> Self {
        Self {
            separator: DEFAULT_SEPARATOR,
            separator_length: DEFAULT_SEPARATOR.chars().count(),
        }
    }
}

impl Separator for BetweenPartsSeparator<'_> {
    fn validate(&self) -> Result<(), SeparatorError> {
        if self.separator.is_empty() {
            Err(SeparatorError::EmptySeparator)
        } else {
            Ok(())
        }
    }

    fn requirement_for(
        &self,
        target_length: usize,
    ) -> Result<SeparationRequirement, SeparatorError> {
        self.validate()?;
        if target_length == 0 {
            return Ok(SeparationRequirement::Impossible { target_length });
        }
        Ok(SeparationRequirement::BetweenParts {
            target_length,
            separator_length: self.separator_length,
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
        for (index, part) in parts.iter().enumerate() {
            if index > 0 {
                output.push_str(self.separator);
            }
            output.push_str(part);
        }
        Ok(())
    }

    fn output_length(
        &self,
        input_length: usize,
        parts_count: usize,
    ) -> Result<usize, SeparatorError> {
        self.validate()?;
        input_length
            .checked_add(
                self.separator_length
                    .checked_mul(parts_count.saturating_sub(1))
                    .ok_or(SeparatorError::LengthOverflow)?,
            )
            .ok_or(SeparatorError::LengthOverflow)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn computes_between_part_length_for_multichar_separator() {
        let separator = BetweenPartsSeparator::new("🟠-");
        let parts = ["猫", "é"];
        assert_eq!(separator.separate(&parts).unwrap(), "猫🟠-é");
        assert_eq!(separator.length_after_separate(&parts).unwrap(), 4);
    }

    #[test]
    fn rejects_empty_separator_without_panicking() {
        assert_eq!(
            BetweenPartsSeparator::new("").validate(),
            Err(SeparatorError::EmptySeparator)
        );
    }
}
