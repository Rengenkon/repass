use crate::error::SeparatorError;

pub mod fix_count;
pub mod interval;
pub mod no_split;
pub mod on_parts;

pub static DEFAULT_SEPARATOR: &str = "-";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SeparationShape {
    /// Total Unicode scalar count in dictionary entries, excluding separators.
    content_length: usize,
    /// Number of dictionary entries that form the content.
    part_count: usize,
}

impl SeparationShape {
    pub fn new(content_length: usize, part_count: usize) -> Self {
        Self {
            content_length,
            part_count,
        }
    }

    pub fn content_length(&self) -> usize {
        self.content_length
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
    groups: usize,
}

impl InputShape {
    pub fn new(content_chars: usize, groups: usize) -> Self {
        Self {
            content_chars,
            groups,
        }
    }

    pub fn content_chars(&self) -> usize {
        self.content_chars
    }

    pub fn groups(&self) -> usize {
        self.groups
    }
}

/// Exact character counts for a particular separator layout.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Layout {
    input: InputShape,
    separator_chars: usize,
    total_chars: usize,
}

impl Layout {
    pub fn input(&self) -> InputShape {
        self.input
    }

    pub fn separator_chars(&self) -> usize {
        self.separator_chars
    }

    pub fn total_chars(&self) -> usize {
        self.total_chars
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
        content_length: usize,
        part_count: PartCount,
    },
    BetweenParts {
        target_length: usize,
        separator_length: usize,
    },
    Impossible {
        target_length: usize,
    },
}

impl SeparationRequirement {
    pub fn into_shapes(self) -> ShapeIter {
        match self {
            Self::Single(shape) => ShapeIter(ShapeIterKind::Single(Some(shape))),
            Self::FixedContent {
                content_length,
                part_count,
            } => ShapeIter(ShapeIterKind::FixedContent {
                content_length,
                part_count: part_count.into_iter(),
            }),
            Self::BetweenParts {
                target_length,
                separator_length,
            } if target_length > 0 && separator_length > 0 => {
                ShapeIter(ShapeIterKind::BetweenParts {
                    target_length,
                    separator_length,
                    next_part_count: Some(1),
                    max_parts: (target_length - 1) / separator_length + 1,
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
        content_length: usize,
        part_count: PartCountIter,
    },
    BetweenParts {
        target_length: usize,
        separator_length: usize,
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
                content_length,
                part_count,
            } => part_count
                .next()
                .map(|part_count| SeparationShape::new(*content_length, part_count)),
            ShapeIterKind::BetweenParts {
                target_length,
                separator_length,
                next_part_count,
                max_parts,
            } => {
                while let Some(part_count) = *next_part_count {
                    if part_count > *max_parts {
                        return None;
                    }
                    *next_part_count = part_count.checked_add(1);
                    let separator_chars = separator_length.checked_mul(part_count - 1)?;
                    let content_length = target_length.checked_sub(separator_chars)?;
                    if content_length > 0 {
                        return Some(SeparationShape::new(content_length, part_count));
                    }
                }
                None
            }
            ShapeIterKind::Empty => None,
        }
    }
}

pub fn get_summary_length(parts: &[&str]) -> usize {
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

pub(super) fn find_content_length(
    target_length: usize,
    mut output_length: impl FnMut(usize) -> Result<usize, SeparatorError>,
) -> Result<Option<usize>, SeparatorError> {
    if target_length == 0 {
        return Ok(None);
    }
    let (mut low, mut high) = (1, target_length);
    while low <= high {
        let candidate = low + (high - low) / 2;
        match output_length(candidate) {
            Ok(output) if output == target_length => return Ok(Some(candidate)),
            Ok(output) if output < target_length => low = candidate + 1,
            Ok(_) | Err(SeparatorError::LengthOverflow) => {
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

    /// Enumerates finite input shapes that can produce the requested output length.
    fn requirement_for(
        &self,
        target_length: usize,
    ) -> Result<SeparationRequirement, SeparatorError>;

    /// Lazily yields feasible content/group shapes for a requested total length.
    fn inputs_for_total(&self, total_chars: usize) -> Result<ShapeIter, SeparatorError> {
        self.requirement_for(total_chars)
            .map(SeparationRequirement::into_shapes)
    }

    /// Separates parts. Length is measured in Unicode scalar values.
    fn separate(&self, parts: &[&str]) -> Result<String, SeparatorError>;

    /// Appends the separated parts to a caller-owned buffer.
    fn write_separated(&self, parts: &[&str], output: &mut String) -> Result<(), SeparatorError> {
        output.push_str(&self.separate(parts)?);
        Ok(())
    }

    /// Computes output length in Unicode scalar values for given input length
    /// and number of dictionary entries.
    fn output_length(
        &self,
        input_length: usize,
        parts_count: usize,
    ) -> Result<usize, SeparatorError>;

    /// Calculates the complete layout for named input dimensions.
    fn layout_for(&self, input: InputShape) -> Result<Layout, SeparatorError> {
        let total_chars = self.output_length(input.content_chars(), input.groups())?;
        let separator_chars = total_chars
            .checked_sub(input.content_chars())
            .ok_or(SeparatorError::LengthOverflow)?;
        Ok(Layout {
            input,
            separator_chars,
            total_chars,
        })
    }

    fn length_after_separate(&self, parts: &[&str]) -> Result<usize, SeparatorError> {
        validate_parts(parts)?;
        self.output_length(get_summary_length(parts), parts.len())
    }
}

#[cfg(test)]
mod requirement_tests {
    use super::*;
    use crate::separator::{
        fix_count::FixCountSeparator, interval::FixIntervalSeparator, no_split::WithoutSeparator,
        on_parts::BetweenPartsSeparator,
    };

    #[test]
    fn fixed_content_requirement_is_iterated_lazily() {
        let requirement = WithoutSeparator.requirement_for(3).unwrap();
        let shapes: Vec<_> = requirement.into_shapes().collect();
        assert_eq!(shapes[0].content_length(), 3);
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
        let requirement = separator.requirement_for(8).unwrap();
        let mut shapes = requirement.into_shapes();
        assert_eq!(shapes.next(), Some(SeparationShape::new(8, 1)));
        assert_eq!(shapes.next(), Some(SeparationShape::new(6, 2)));
    }

    #[test]
    fn interval_and_fixed_count_detect_unreachable_target_lengths() {
        assert!(matches!(
            FixIntervalSeparator::new("-", 3)
                .requirement_for(12)
                .unwrap(),
            SeparationRequirement::Impossible { target_length: 12 }
        ));
        assert!(matches!(
            FixCountSeparator::new("::", 2).requirement_for(3).unwrap(),
            SeparationRequirement::Impossible { target_length: 3 }
        ));
    }

    #[test]
    fn layout_reports_content_separator_and_total_lengths() {
        let separator = BetweenPartsSeparator::new("🟠-");
        let layout = separator.layout_for(InputShape::new(5, 3)).unwrap();
        assert_eq!(layout.input().content_chars(), 5);
        assert_eq!(layout.input().groups(), 3);
        assert_eq!(layout.separator_chars(), 4);
        assert_eq!(layout.total_chars(), 9);
    }

    #[test]
    fn between_parts_cursor_handles_maximum_target_without_overflowing() {
        let requirement = SeparationRequirement::BetweenParts {
            target_length: usize::MAX,
            separator_length: 1,
        };
        let mut shapes = requirement.into_shapes();
        assert_eq!(shapes.next(), Some(SeparationShape::new(usize::MAX, 1)));
        assert_eq!(shapes.next(), Some(SeparationShape::new(usize::MAX - 1, 2)));
    }
}
