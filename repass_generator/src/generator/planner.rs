use crate::dictionary::cache::DictionaryCache;
use crate::error::GeneratorError;
use crate::query::GenerationLimits;
use crate::separator::SeparationShape;
use rand::{Rng, RngExt};
use std::collections::HashSet;

type SearchState = (usize, usize); // (remaining content length, remaining parts)
pub(super) struct CombinationPlanner<'dictionary, 'entries> {
    dictionary: &'dictionary DictionaryCache<'entries>,
    failed: HashSet<SearchState>,
    visited: usize,
    max_states: usize,
    lengths_gcd: usize,
    stack: Vec<(usize, usize, usize)>,
}

impl<'dictionary, 'entries> CombinationPlanner<'dictionary, 'entries> {
    pub(super) fn new(
        dictionary: &'dictionary DictionaryCache<'entries>,
        limits: GenerationLimits,
    ) -> Self {
        let lengths_gcd = dictionary
            .available_lengths()
            .iter()
            .copied()
            .reduce(gcd)
            .unwrap_or(0);
        Self {
            dictionary,
            failed: HashSet::new(),
            visited: 0,
            max_states: limits.max_planner_states,
            lengths_gcd,
            stack: Vec::new(),
        }
    }

    pub(super) fn find_plan(
        &mut self,
        shape: SeparationShape,
        rng: &mut impl Rng,
    ) -> Result<Option<Vec<usize>>, GeneratorError> {
        if shape.part_count == 0 || shape.content_length == 0 {
            return Ok(None);
        }
        let min_length = self.dictionary.min_length();
        let Some(&max_length) = self.dictionary.available_lengths().last() else {
            return Ok(None);
        };
        let Some(min_required) = min_length.checked_mul(shape.part_count) else {
            return Ok(None);
        };
        let max_possible = max_length
            .checked_mul(shape.part_count)
            .unwrap_or(usize::MAX);
        if shape.content_length < min_required || shape.content_length > max_possible {
            return Ok(None);
        }
        if self.lengths_gcd == 0 || shape.content_length % self.lengths_gcd != 0 {
            return Ok(None);
        }

        if !self.can_complete(shape.content_length, shape.part_count, shape.content_length)? {
            return Ok(None);
        }

        let mut remaining_length = shape.content_length;
        let mut remaining_parts = shape.part_count;
        let mut result = Vec::new();
        result
            .try_reserve_exact(shape.part_count)
            .map_err(|_| GeneratorError::ResourceLimit {
                target: shape.content_length,
            })?;
        while remaining_parts > 0 {
            let mut viable_count = 0usize;
            for &length in self.dictionary.available_lengths() {
                if length > remaining_length {
                    break;
                }
                if self.can_complete(
                    remaining_length - length,
                    remaining_parts - 1,
                    shape.content_length,
                )? {
                    viable_count += 1;
                }
            }
            if viable_count == 0 {
                return Ok(None);
            }
            let selected_length = rng.random_range(0..viable_count);
            let mut viable_index = 0;
            let mut selected_length_value = None;
            for &length in self.dictionary.available_lengths() {
                if length > remaining_length {
                    break;
                }
                if self.can_complete(
                    remaining_length - length,
                    remaining_parts - 1,
                    shape.content_length,
                )? {
                    if viable_index == selected_length {
                        selected_length_value = Some(length);
                        break;
                    }
                    viable_index += 1;
                }
            }
            let Some(length) = selected_length_value else {
                return Ok(None);
            };
            let indexes = self.dictionary.entries_with_length(length);
            let Some(index) = indexes.get(rng.random_range(0..indexes.len())).copied() else {
                return Ok(None);
            };
            result.push(index);
            remaining_length -= length;
            remaining_parts -= 1;
        }
        Ok((remaining_length == 0).then_some(result))
    }

    fn can_complete(
        &mut self,
        remaining_length: usize,
        remaining_parts: usize,
        target: usize,
    ) -> Result<bool, GeneratorError> {
        let min_length = self.dictionary.min_length();
        let Some(&max_length) = self.dictionary.available_lengths().last() else {
            return Ok(false);
        };
        let lengths = self.dictionary.available_lengths();
        self.stack.clear();
        self.stack.push((remaining_length, remaining_parts, 0usize));
        while let Some((length_left, parts_left, next_length_index)) = self.stack.pop() {
            self.visited += 1;
            if self.visited > self.max_states {
                return Err(GeneratorError::SearchLimitExceeded {
                    target,
                    limit: self.max_states,
                });
            }
            if parts_left == 0 {
                if length_left == 0 {
                    return Ok(true);
                }
                continue;
            }
            if length_left == 0
                || min_length
                    .checked_mul(parts_left)
                    .is_none_or(|minimum| length_left < minimum)
                || max_length
                    .checked_mul(parts_left)
                    .is_some_and(|maximum| length_left > maximum)
            {
                continue;
            }

            let state = (length_left, parts_left);
            if self.failed.contains(&state) {
                continue;
            }
            if next_length_index >= lengths.len() || lengths[next_length_index] > length_left {
                self.failed.insert(state);
                continue;
            }

            self.stack
                .push((length_left, parts_left, next_length_index + 1));
            let entry_length = lengths[next_length_index];
            let child_state = (length_left - entry_length, parts_left - 1);
            if child_state.1 == 0 {
                if child_state.0 == 0 {
                    return Ok(true);
                }
                continue;
            }
            if !self.failed.contains(&child_state) {
                self.stack.push((child_state.0, child_state.1, 0));
            }
        }
        Ok(false)
    }
}

fn gcd(mut left: usize, mut right: usize) -> usize {
    while right != 0 {
        (left, right) = (right, left % right);
    }
    left
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::dictionary::file_dictionary::FileDictionary;

    #[test]
    fn finds_combination_using_sparse_dictionary_lengths() {
        let dictionary =
            DictionaryCache::new(FileDictionary::from_entries(["a", "xy", "猫"]).unwrap()).unwrap();
        let mut rng = rand::rng();
        let mut planner = CombinationPlanner::new(&dictionary, GenerationLimits::default());
        let plan = planner
            .find_plan(
                SeparationShape {
                    content_length: 4,
                    part_count: 3,
                },
                &mut rng,
            )
            .unwrap()
            .unwrap();
        assert_eq!(plan.len(), 3);
        assert_eq!(
            plan.iter()
                .map(|index| dictionary.entry(*index).unwrap().chars().count())
                .sum::<usize>(),
            4
        );
    }

    #[test]
    fn returns_none_when_shape_cannot_be_formed() {
        let dictionary =
            DictionaryCache::new(FileDictionary::from_entries(["ab"]).unwrap()).unwrap();
        let mut rng = rand::rng();
        assert!(
            CombinationPlanner::new(&dictionary, GenerationLimits::default())
                .find_plan(
                    SeparationShape {
                        content_length: 5,
                        part_count: 2,
                    },
                    &mut rng,
                )
                .unwrap()
                .is_none()
        );
    }
}
