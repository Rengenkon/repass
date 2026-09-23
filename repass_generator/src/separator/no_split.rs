use crate::error::SeparatorError;
use crate::separator::{PartCount, SeparationRequirement, Separator, validate_parts};

#[derive(Debug, Default)]
pub struct WithoutSeparator;

impl WithoutSeparator {
    pub fn new() -> Self {
        Self
    }
}

impl Separator for WithoutSeparator {
    fn validate(&self) -> Result<(), SeparatorError> {
        Ok(())
    }

    fn requirement_for(
        &self,
        target_length: usize,
    ) -> Result<SeparationRequirement, SeparatorError> {
        if target_length == 0 {
            return Ok(SeparationRequirement::Impossible { target_length });
        }
        Ok(SeparationRequirement::FixedContent {
            content_length: target_length,
            part_count: PartCount::Range {
                min: 1,
                max: target_length,
            },
        })
    }

    fn separate(&self, parts: &[&str]) -> Result<String, SeparatorError> {
        let mut output = String::new();
        self.write_separated(parts, &mut output)?;
        Ok(output)
    }

    fn write_separated(&self, parts: &[&str], output: &mut String) -> Result<(), SeparatorError> {
        validate_parts(parts)?;
        for part in parts {
            output.push_str(part);
        }
        Ok(())
    }

    fn output_length(
        &self,
        input_length: usize,
        _parts_count: usize,
    ) -> Result<usize, SeparatorError> {
        Ok(input_length)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn joins_unicode_parts_without_changing_content() {
        let separator = WithoutSeparator;
        assert_eq!(separator.separate(&["é", "🦀"]).unwrap(), "é🦀");
        assert_eq!(separator.length_after_separate(&["é", "🦀"]).unwrap(), 2);
    }

    #[test]
    fn reports_empty_parts_as_an_error() {
        assert_eq!(
            WithoutSeparator.separate(&[]),
            Err(SeparatorError::EmptyParts)
        );
    }
}
