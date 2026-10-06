use crate::dictionary::cache::DictionaryCache;
use crate::error::GeneratorError;
use crate::query::GenerationLimits;
use crate::separator::SeparationShape;
use rand::{Rng, RngExt};
use std::collections::HashMap;

type PlannerState = (usize, usize); // (remaining content characters, remaining parts)
pub(super) struct CombinationPlanner<'dictionary, 'entries> {
    dictionary: &'dictionary DictionaryCache<'entries>,
    reachable: HashMap<PlannerState, bool>,
    visited: usize,
    max_states: usize,
    entry_chars_gcd: usize,
    min_entry_chars: usize,
    max_entry_chars: usize,
    stack: Vec<(usize, usize, usize)>,
    viable_entry_chars: Vec<usize>,
}

impl<'dictionary, 'entries> CombinationPlanner<'dictionary, 'entries> {
    pub(super) fn new(
        dictionary: &'dictionary DictionaryCache<'entries>,
        limits: GenerationLimits,
    ) -> Self {
        let entry_chars_gcd = dictionary
            .available_entry_chars()
            .iter()
            .copied()
            .reduce(gcd)
            .unwrap_or(0);
        let min_entry_chars = dictionary.min_entry_chars();
        let max_entry_chars = dictionary
            .available_entry_chars()
            .last()
            .copied()
            .unwrap_or(0);
        Self {
            dictionary,
            reachable: HashMap::new(),
            visited: 0,
            max_states: limits.max_planner_states(),
            entry_chars_gcd,
            min_entry_chars,
            max_entry_chars,
            stack: Vec::new(),
            viable_entry_chars: Vec::new(),
        }
    }

    pub(super) fn reset_budget(&mut self) {
        self.visited = 0;
        if self.reachable.len() >= self.max_states {
            self.reachable.clear();
        }
    }

    pub(super) fn find_combination(
        &mut self,
        shape: SeparationShape,
        rng: &mut impl Rng,
    ) -> Result<Option<Vec<usize>>, GeneratorError> {
        if !self.is_feasible(shape)? {
            return Ok(None);
        }
        self.choose_combination(shape, rng)
    }

    pub(super) fn is_feasible(&mut self, shape: SeparationShape) -> Result<bool, GeneratorError> {
        if shape.part_count() == 0 || shape.content_chars() == 0 {
            return Ok(false);
        }
        let min_entry_chars = self.min_entry_chars;
        let max_entry_chars = self.max_entry_chars;
        if max_entry_chars == 0 {
            return Ok(false);
        }
        let Some(min_required) = min_entry_chars.checked_mul(shape.part_count()) else {
            return Ok(false);
        };
        let max_possible = max_entry_chars.saturating_mul(shape.part_count());
        if shape.content_chars() < min_required || shape.content_chars() > max_possible {
            return Ok(false);
        }
        if self.entry_chars_gcd == 0 || !shape.content_chars().is_multiple_of(self.entry_chars_gcd)
        {
            return Ok(false);
        }

        self.can_complete(
            shape.content_chars(),
            shape.part_count(),
            shape.content_chars(),
        )
    }

    fn choose_combination(
        &mut self,
        shape: SeparationShape,
        rng: &mut impl Rng,
    ) -> Result<Option<Vec<usize>>, GeneratorError> {
        let mut remaining_chars = shape.content_chars();
        let mut remaining_parts = shape.part_count();
        let mut result = Vec::new();
        self.viable_entry_chars
            .try_reserve(self.dictionary.available_entry_chars().len())
            .map_err(|_| GeneratorError::ResourceLimit {
                target_chars: shape.content_chars(),
            })?;
        result.try_reserve_exact(shape.part_count()).map_err(|_| {
            GeneratorError::ResourceLimit {
                target_chars: shape.content_chars(),
            }
        })?;
        while remaining_parts > 0 {
            self.viable_entry_chars.clear();
            for &entry_chars in self.dictionary.available_entry_chars() {
                if entry_chars > remaining_chars {
                    break;
                }
                if self.can_complete(
                    remaining_chars - entry_chars,
                    remaining_parts - 1,
                    shape.content_chars(),
                )? {
                    self.viable_entry_chars.push(entry_chars);
                }
            }
            if self.viable_entry_chars.is_empty() {
                return Ok(None);
            }
            let entry_chars =
                self.viable_entry_chars[rng.random_range(0..self.viable_entry_chars.len())];
            let indexes = self.dictionary.entries_with_chars(entry_chars);
            let Some(index) = indexes.get(rng.random_range(0..indexes.len())).copied() else {
                return Ok(None);
            };
            result.push(index);
            remaining_chars -= entry_chars;
            remaining_parts -= 1;
        }
        Ok((remaining_chars == 0).then_some(result))
    }

    fn can_complete(
        &mut self,
        remaining_chars: usize,
        remaining_parts: usize,
        target_chars: usize,
    ) -> Result<bool, GeneratorError> {
        let available_entry_chars = self.dictionary.available_entry_chars();
        // Equal-length dictionaries need no search, regardless of output length.
        if self.min_entry_chars == self.max_entry_chars {
            return Ok(self.min_entry_chars.checked_mul(remaining_parts) == Some(remaining_chars));
        }
        self.stack.clear();
        self.stack
            .try_reserve(1)
            .map_err(|_| GeneratorError::ResourceLimit { target_chars })?;
        self.stack.push((remaining_chars, remaining_parts, 0usize));
        while let Some(&(chars_left, parts_left, next_entry_chars_index)) = self.stack.last() {
            let state = (chars_left, parts_left);
            let known = if parts_left == 0 {
                Some(chars_left == 0)
            } else if chars_left == 0
                || self
                    .min_entry_chars
                    .checked_mul(parts_left)
                    .is_none_or(|minimum| chars_left < minimum)
                || self
                    .max_entry_chars
                    .checked_mul(parts_left)
                    .is_some_and(|maximum| chars_left > maximum)
                || chars_left % self.entry_chars_gcd != 0
            {
                Some(false)
            } else {
                self.reachable.get(&state).copied()
            };
            if let Some(reachable) = known {
                if reachable {
                    self.reachable
                        .try_reserve(self.stack.len())
                        .map_err(|_| GeneratorError::ResourceLimit { target_chars })?;
                    for &(chars, parts, _) in &self.stack {
                        self.reachable.insert((chars, parts), true);
                    }
                    return Ok(true);
                }
                self.stack.pop();
                continue;
            }
            if next_entry_chars_index == 0 {
                if self.visited >= self.max_states {
                    return Err(GeneratorError::SearchLimitExceeded {
                        target_chars,
                        limit: self.max_states,
                    });
                }
                self.visited += 1;
                self.reachable
                    .try_reserve(1)
                    .map_err(|_| GeneratorError::ResourceLimit { target_chars })?;
                self.stack
                    .try_reserve(1)
                    .map_err(|_| GeneratorError::ResourceLimit { target_chars })?;
            }
            if next_entry_chars_index >= available_entry_chars.len()
                || available_entry_chars[next_entry_chars_index] > chars_left
            {
                self.reachable.insert(state, false);
                self.stack.pop();
                continue;
            }
            if let Some(frame) = self.stack.last_mut() {
                frame.2 += 1;
            }
            let entry_chars = available_entry_chars[next_entry_chars_index];
            self.stack
                .push((chars_left - entry_chars, parts_left - 1, 0));
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
    fn finds_combination_using_sparse_entry_char_counts() {
        let dictionary =
            DictionaryCache::new(FileDictionary::from_entries(["a", "xy", "猫"]).unwrap()).unwrap();
        let mut rng = rand::rng();
        let mut planner = CombinationPlanner::new(&dictionary, GenerationLimits::default());
        let plan = planner
            .find_combination(SeparationShape::new(4, 3), &mut rng)
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
                .find_combination(SeparationShape::new(5, 2), &mut rng,)
                .unwrap()
                .is_none()
        );
    }

    #[test]
    fn repeated_successful_subproblems_use_the_same_search_budget() {
        let dictionary =
            DictionaryCache::new(FileDictionary::from_entries(["a", "aa"]).unwrap()).unwrap();
        let mut planner = CombinationPlanner::new(&dictionary, GenerationLimits::new(10, 2_000));
        let mut rng = rand::rng();
        let plan = planner
            .find_combination(SeparationShape::new(1_000, 1_000), &mut rng)
            .unwrap()
            .unwrap();
        assert_eq!(plan.len(), 1_000);
    }
}
