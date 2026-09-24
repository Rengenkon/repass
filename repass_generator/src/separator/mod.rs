use crate::error::SeparatorError;

pub mod between_parts;
pub mod fixed_count;
pub mod fixed_interval;
pub mod without_separator;

pub const DEFAULT_SEPARATOR: &str = "-";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SeparationShape {
    /// Total Unicode scalar count in dictionary entries, excluding separators.
    content_chars: usize,
    /// Number of dictionary entries that form the content.
    part_count: usize,
}

impl SeparationShape {
    pub fn new(content_chars: usize, part_count: usize) -> Self {
        Self {
            content_chars,
            part_count,
        }
    }

    pub fn content_chars(&self) -> usize {
        self.content_chars
    }

    pub fn part_count(&self) -> usize {
        self.part_count
    }
}

/// Input dimensions used to calculate a separator's output without rendering it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct InputShape {
    /// Unicode scalar count in the content, excluding separators.
    content_chars: usize,
    /// Number of non-empty dictionary entries in the content.
    part_count: usize,
}

impl InputShape {
    pub fn new(content_chars: usize, part_count: usize) -> Self {
        Self {
            content_chars,
            part_count,
        }
    }

    pub fn content_chars(&self) -> usize {
        self.content_chars
    }

    pub fn part_count(&self) -> usize {
        self.part_count
    }
}

/// Exact Unicode scalar counts for a particular separator layout.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Layout {
    input: InputShape,
    separator_chars: usize,
    output_chars: usize,
}

impl Layout {
    pub fn input_shape(&self) -> InputShape {
        self.input
    }

    pub fn separator_chars(&self) -> usize {
        self.separator_chars
    }

    pub fn output_chars(&self) -> usize {
        self.output_chars
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PartCount {
    Exact(usize),
    Range { min: usize, max: usize },
}

impl PartCount {
    fn into_iter(self) -> PartCountIter {
        PartCountIter(match self {
            Self::Exact(value) => PartCountIterKind::Exact(Some(value)),
            Self::Range { min, max } => PartCountIterKind::Range(min..=max),
        })
    }
}

struct PartCountIter(PartCountIterKind);

enum PartCountIterKind {
    Exact(Option<usize>),
    Range(std::ops::RangeInclusive<usize>),
}

impl Iterator for PartCountIter {
    type Item = usize;

    fn next(&mut self) -> Option<Self::Item> {
        match &mut self.0 {
            PartCountIterKind::Exact(value) => value.take(),
            PartCountIterKind::Range(values) => values.next(),
        }
    }
}

pub enum SeparationRequirement {
    Single(SeparationShape),
    FixedContent {
        content_chars: usize,
        part_count: PartCount,
    },
    BetweenParts {
        target_chars: usize,
        separator_chars: usize,
    },
    Impossible {
        target_chars: usize,
    },
}

impl SeparationRequirement {
    pub fn into_shapes(self) -> ShapeIter {
        match self {
            Self::Single(shape) => ShapeIter(ShapeIterKind::Single(Some(shape))),
            Self::FixedContent {
                content_chars,
                part_count,
            } => ShapeIter(ShapeIterKind::FixedContent {
                content_chars,
                part_count: part_count.into_iter(),
            }),
            Self::BetweenParts {
                target_chars,
                separator_chars,
            } if target_chars > 0 && separator_chars > 0 => {
                ShapeIter(ShapeIterKind::BetweenParts {
                    target_chars,
                    separator_chars,
                    next_part_count: Some(1),
                    max_parts: (target_chars - 1) / separator_chars + 1,
                })
            }
            Self::BetweenParts { .. } | Self::Impossible { .. } => ShapeIter(ShapeIterKind::Empty),
        }
    }
}

/// Lazily enumerates feasible input shapes without allocating a boxed iterator.
pub struct ShapeIter(ShapeIterKind);

enum ShapeIterKind {
    Single(Option<SeparationShape>),
    FixedContent {
        content_chars: usize,
        part_count: PartCountIter,
    },
    BetweenParts {
        target_chars: usize,
        separator_chars: usize,
        next_part_count: Option<usize>,
        max_parts: usize,
    },
    Empty,
}

impl Iterator for ShapeIter {
    type Item = SeparationShape;

    fn next(&mut self) -> Option<Self::Item> {
        match &mut self.0 {
            ShapeIterKind::Single(shape) => shape.take(),
            ShapeIterKind::FixedContent {
                content_chars,
                part_count,
            } => part_count
                .next()
                .map(|part_count| SeparationShape::new(*content_chars, part_count)),
            ShapeIterKind::BetweenParts {
                target_chars,
                separator_chars,
                next_part_count,
                max_parts,
            } => {
                while let Some(part_count) = *next_part_count {
                    if part_count > *max_parts {
                        return None;
                    }
                    *next_part_count = part_count.checked_add(1);
                    let separator_chars = separator_chars.checked_mul(part_count - 1)?;
                    let content_chars = target_chars.checked_sub(separator_chars)?;
                    if content_chars > 0 {
                        return Some(SeparationShape::new(content_chars, part_count));
                    }
                }
                None
            }
            ShapeIterKind::Empty => None,
        }
    }
}

pub fn content_chars_of(parts: &[&str]) -> usize {
    parts.iter().map(|part| part.chars().count()).sum()
}

pub fn validate_parts(parts: &[&str]) -> Result<(), SeparatorError> {
    if parts.is_empty() {
        return Err(SeparatorError::EmptyParts);
    }
    if parts.iter().any(|part| part.is_empty()) {
        return Err(SeparatorError::EmptyPart);
    }
    Ok(())
}

pub(super) fn find_content_chars(
    target_chars: usize,
    mut output_chars: impl FnMut(usize) -> Result<usize, SeparatorError>,
) -> Result<Option<usize>, SeparatorError> {
    if target_chars == 0 {
        return Ok(None);
    }
    let (mut low, mut high) = (1, target_chars);
    while low <= high {
        let candidate = low + (high - low) / 2;
        match output_chars(candidate) {
            Ok(output) if output == target_chars => return Ok(Some(candidate)),
            Ok(output) if output < target_chars => low = candidate + 1,
            Ok(_) | Err(SeparatorError::CharacterCountOverflow) => {
                if candidate == 1 {
                    return Ok(None);
                }
                high = candidate - 1;
            }
            Err(error) => return Err(error),
        }
    }
    Ok(None)
}

pub trait Separator {
    fn validate(&self) -> Result<(), SeparatorError>;

    /// Enumerates finite input shapes that can produce the requested output character count.
    fn requirement_for_output(
        &self,
        target_chars: usize,
    ) -> Result<SeparationRequirement, SeparatorError>;

    /// Lazily yields feasible content/part shapes for the requested output character count.
    fn shapes_for_output(&self, target_chars: usize) -> Result<ShapeIter, SeparatorError> {
        self.requirement_for_output(target_chars)
            .map(SeparationRequirement::into_shapes)
    }

    /// Separates parts. Character counts are measured in Unicode scalar values.
    fn separate(&self, parts: &[&str]) -> Result<String, SeparatorError>;

    /// Appends the separated parts to a caller-owned buffer.
    fn write_separated(&self, parts: &[&str], output: &mut String) -> Result<(), SeparatorError> {
        output.push_str(&self.separate(parts)?);
        Ok(())
    }

    /// Computes output character count in Unicode scalar values for given content character count
    /// and number of dictionary entries.
    fn output_chars(
        &self,
        content_chars: usize,
        part_count: usize,
    ) -> Result<usize, SeparatorError>;

    /// Calculates the complete layout for named input dimensions.
    fn layout_for(&self, input: InputShape) -> Result<Layout, SeparatorError> {
        let output_chars = self.output_chars(input.content_chars(), input.part_count())?;
        let separator_chars = output_chars
            .checked_sub(input.content_chars())
            .ok_or(SeparatorError::CharacterCountOverflow)?;
        Ok(Layout {
            input,
            separator_chars,
            output_chars,
        })
    }

    fn separated_chars(&self, parts: &[&str]) -> Result<usize, SeparatorError> {
        validate_parts(parts)?;
        self.output_chars(content_chars_of(parts), parts.len())
    }
}

#[cfg(test)]
mod requirement_tests {
    use super::*;
    use crate::separator::{
        between_parts::BetweenPartsSeparator, fixed_count::FixedCountSeparator,
        fixed_interval::FixedIntervalSeparator, without_separator::WithoutSeparator,
    };

    #[test]
    fn fixed_content_requirement_is_iterated_lazily() {
        let requirement = WithoutSeparator.requirement_for_output(3).unwrap();
        let shapes: Vec<_> = requirement.into_shapes().collect();
        assert_eq!(shapes[0].content_chars(), 3);
        assert_eq!(shapes[0].part_count(), 1);
        assert_eq!(
            shapes,
            vec![
                SeparationShape::new(3, 1),
                SeparationShape::new(3, 2),
                SeparationShape::new(3, 3),
            ]
        );
    }

    #[test]
    fn between_parts_shapes_are_generated_on_demand() {
        let separator = BetweenPartsSeparator::new("--");
        let requirement = separator.requirement_for_output(8).unwrap();
        let mut shapes = requirement.into_shapes();
        assert_eq!(shapes.next(), Some(SeparationShape::new(8, 1)));
        assert_eq!(shapes.next(), Some(SeparationShape::new(6, 2)));
    }

    #[test]
    fn interval_and_fixed_count_detect_unreachable_target_chars() {
        assert!(matches!(
            FixedIntervalSeparator::new("-", 3)
                .requirement_for_output(12)
                .unwrap(),
            SeparationRequirement::Impossible { target_chars: 12 }
        ));
        assert!(matches!(
            FixedCountSeparator::new("::", 2)
                .requirement_for_output(3)
                .unwrap(),
            SeparationRequirement::Impossible { target_chars: 3 }
        ));
    }

    #[test]
    fn layout_reports_content_separator_and_output_char_counts() {
        let separator = BetweenPartsSeparator::new("🟠-");
        let layout = separator.layout_for(InputShape::new(5, 3)).unwrap();
        assert_eq!(layout.input_shape().content_chars(), 5);
        assert_eq!(layout.input_shape().part_count(), 3);
        assert_eq!(layout.separator_chars(), 4);
        assert_eq!(layout.output_chars(), 9);
    }

    #[test]
    fn between_parts_cursor_handles_maximum_target_without_overflowing() {
        let requirement = SeparationRequirement::BetweenParts {
            target_chars: usize::MAX,
            separator_chars: 1,
        };
        let mut shapes = requirement.into_shapes();
        assert_eq!(shapes.next(), Some(SeparationShape::new(usize::MAX, 1)));
        assert_eq!(shapes.next(), Some(SeparationShape::new(usize::MAX - 1, 2)));
    }
}
