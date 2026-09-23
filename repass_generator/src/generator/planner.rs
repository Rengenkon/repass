use crate::dictionary::cache::DictionaryCache;
use crate::error::GeneratorError;
use crate::separator::SeparationShape;
use rand::{Rng, RngExt};
use std::collections::HashSet;

type SearchState = (usize, usize); // (remaining content length, remaining parts)
const MAX_PLANNER_STATES: usize = 100_000;

pub(super) struct CombinationPlanner<'a> {
    dictionary: &'a DictionaryCache<'a>,
    failed: HashSet<SearchState>,
    visited: usize,
}

impl<'a> CombinationPlanner<'a> {
    pub(super) fn new(dictionary: &'a DictionaryCache<'a>) -> Self {
        Self {
            dictionary,
            failed: HashSet::new(),
            visited: 0,
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
            let mut viable_lengths = Vec::new();
            for &length in self.dictionary.available_lengths() {
                if length > remaining_length {
                    break;
                }
                if self.can_complete(
                    remaining_length - length,
                    remaining_parts - 1,
                    shape.content_length,
                )? {
                    viable_lengths.push(length);
                }
            }
            if viable_lengths.is_empty() {
                return Ok(None);
            }
            let length = viable_lengths[rng.random_range(0..viable_lengths.len())];
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
        let mut stack = vec![(remaining_length, remaining_parts, 0usize)];
        while let Some((length_left, parts_left, next_length_index)) = stack.pop() {
            self.visited += 1;
            if self.visited > MAX_PLANNER_STATES {
                return Err(GeneratorError::SearchLimitExceeded {
                    target,
                    limit: MAX_PLANNER_STATES,
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

            stack.push((length_left, parts_left, next_length_index + 1));
            let entry_length = lengths[next_length_index];
            let child_state = (length_left - entry_length, parts_left - 1);
            if child_state.1 == 0 {
                if child_state.0 == 0 {
                    return Ok(true);
                }
                continue;
            }
            if !self.failed.contains(&child_state) {
                stack.push((child_state.0, child_state.1, 0));
            }
        }
        Ok(false)
    }
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
        let mut planner = CombinationPlanner::new(&dictionary);
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
            CombinationPlanner::new(&dictionary)
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
