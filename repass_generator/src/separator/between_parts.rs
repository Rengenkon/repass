use super::{DEFAULT_SEPARATOR, SeparationRequirement, Separator, validate_parts};
use crate::error::SeparatorError;

#[derive(Debug)]
pub struct BetweenPartsSeparator<'a> {
    separator: &'a str,
    separator_chars: usize,
}

impl<'a> BetweenPartsSeparator<'a> {
    pub fn new(separator: &'a str) -> Self {
        Self {
            separator,
            separator_chars: separator.chars().count(),
        }
    }
}

impl Default for BetweenPartsSeparator<'_> {
    fn default() -> Self {
        Self {
            separator: DEFAULT_SEPARATOR,
            separator_chars: DEFAULT_SEPARATOR.chars().count(),
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

    fn requirement_for_output(
        &self,
        target_chars: usize,
    ) -> Result<SeparationRequirement, SeparatorError> {
        self.validate()?;
        if target_chars == 0 {
            return Ok(SeparationRequirement::Impossible { target_chars });
        }
        Ok(SeparationRequirement::BetweenParts {
            target_chars,
            separator_chars: self.separator_chars,
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

    fn output_chars(
        &self,
        content_chars: usize,
        part_count: usize,
    ) -> Result<usize, SeparatorError> {
        self.validate()?;
        content_chars
            .checked_add(
                self.separator_chars
                    .checked_mul(part_count.saturating_sub(1))
                    .ok_or(SeparatorError::CharacterCountOverflow)?,
            )
            .ok_or(SeparatorError::CharacterCountOverflow)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn computes_between_part_chars_for_multichar_separator() {
        let separator = BetweenPartsSeparator::new("🟠-");
        let parts = ["猫", "é"];
        assert_eq!(separator.separate(&parts).unwrap(), "猫🟠-é");
        assert_eq!(separator.separated_chars(&parts).unwrap(), 4);
    }

    #[test]
    fn rejects_empty_separator_without_panicking() {
        assert_eq!(
            BetweenPartsSeparator::new("").validate(),
            Err(SeparatorError::EmptySeparator)
        );
    }
}
