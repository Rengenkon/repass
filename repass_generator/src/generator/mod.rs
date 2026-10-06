mod planner;

use crate::error::GeneratorError;
use crate::query::{PasswordLength, Query, ShapeSelection};
use crate::separator::{PartCount, SeparationRequirement, SeparationShape};
use rand::{Rng, RngExt};
use std::collections::HashMap;

fn choose_target_chars(
    password_length: &PasswordLength,
    rng: &mut impl Rng,
) -> Result<usize, GeneratorError> {
    password_length.validate()?;
    let target_chars = match password_length {
        PasswordLength::Exact(value) => *value,
        PasswordLength::Range {
            min_chars,
            max_chars,
        } => rng.random_range(*min_chars..=*max_chars),
    };
    Ok(target_chars)
}

/// Reuses reachability and feasible shapes while sampling fresh dictionary entries.
pub struct PasswordGenerator<'q, 'a, 'entries> {
    query: &'q Query<'a, 'entries>,
    planner: planner::CombinationPlanner<'a, 'entries>,
    shapes: HashMap<usize, Vec<SeparationShape>>,
    cached_shape_count: usize,
}

impl<'q, 'a, 'entries> PasswordGenerator<'q, 'a, 'entries> {
    pub fn new(query: &'q Query<'a, 'entries>) -> Self {
        Self {
            query,
            planner: planner::CombinationPlanner::new(query.dictionary(), query.limits()),
            shapes: HashMap::new(),
            cached_shape_count: 0,
        }
    }

    fn prepare(&mut self, target_chars: usize) -> Result<(), GeneratorError> {
        if self.shapes.contains_key(&target_chars) {
            return Ok(());
        }
        self.planner.reset_budget();
        let dictionary = self.query.dictionary();
        let requirement = self
            .query
            .separator()
            .requirement_for_output(target_chars)
            .map_err(GeneratorError::InvalidSeparator)?;
        let requirement = match requirement {
            SeparationRequirement::FixedContent {
                content_chars,
                part_count,
            } => {
                let max_entry = dictionary
                    .available_entry_chars()
                    .last()
                    .copied()
                    .unwrap_or(1);
                let minimum = content_chars.div_ceil(max_entry);
                let maximum = content_chars / dictionary.min_entry_chars();
                let (min, max) = match part_count {
                    PartCount::Exact(n) => (n.max(minimum), n.min(maximum)),
                    PartCount::Range { min, max } => (min.max(minimum), max.min(maximum)),
                };
                SeparationRequirement::FixedContent {
                    content_chars,
                    part_count: PartCount::Range { min, max },
                }
            }
            other => other,
        };
        let mut feasible = Vec::new();
        let mut any = false;
        let candidates: Box<dyn Iterator<Item = SeparationShape>> = match requirement {
            SeparationRequirement::BetweenParts {
                target_chars,
                separator_chars,
            } if target_chars > 0 && separator_chars > 0 => {
                let total = target_chars as u128 + separator_chars as u128;
                let max_entry = dictionary
                    .available_entry_chars()
                    .last()
                    .copied()
                    .unwrap_or(1);
                let min = total.div_ceil(max_entry as u128 + separator_chars as u128) as usize;
                let max = (total / (dictionary.min_entry_chars() as u128 + separator_chars as u128))
                    as usize;
                Box::new((min..=max).map(move |parts| {
                    SeparationShape::new(target_chars - separator_chars * (parts - 1), parts)
                }))
            }
            other => Box::new(other.into_shapes()),
        };
        for (index, shape) in candidates.enumerate() {
            any = true;
            if index >= self.query.limits().max_shapes() {
                return Err(GeneratorError::SearchLimitExceeded {
                    target_chars,
                    limit: self.query.limits().max_shapes(),
                });
            }
            if self.planner.is_feasible(shape)? {
                feasible
                    .try_reserve(1)
                    .map_err(|_| GeneratorError::ResourceLimit { target_chars })?;
                feasible.push(shape);
                if self.query.shape_selection() == ShapeSelection::First {
                    break;
                }
            }
        }
        if feasible.is_empty() {
            // A pruned fixed-content range can be empty even though the separator
            // has layouts: distinguish that from an impossible separator length.
            let separator_has_shape = self
                .query
                .separator()
                .shapes_for_output(target_chars)
                .map_err(GeneratorError::InvalidSeparator)?
                .next()
                .is_some();
            return Err(if any || separator_has_shape {
                GeneratorError::NoDictionaryCombination { target_chars }
            } else {
                GeneratorError::NoSeparationShape { target_chars }
            });
        }
        if self.cached_shape_count.saturating_add(feasible.len()) > self.query.limits().max_shapes()
        {
            self.shapes.clear();
            self.cached_shape_count = 0;
        }
        self.shapes
            .try_reserve(1)
            .map_err(|_| GeneratorError::ResourceLimit { target_chars })?;
        self.cached_shape_count += feasible.len();
        self.shapes.insert(target_chars, feasible);
        Ok(())
    }

    pub fn next_with_rng(&mut self, rng: &mut impl Rng) -> Result<String, GeneratorError> {
        let target_chars = choose_target_chars(self.query.password_length(), rng)?;
        self.prepare(target_chars)?;
        let shapes = &self.shapes[&target_chars];
        let shape = if shapes.len() == 1 {
            shapes[0]
        } else {
            shapes[rng.random_range(0..shapes.len())]
        };
        self.planner.reset_budget();
        let plan = self
            .planner
            .find_combination(shape, rng)?
            .ok_or(GeneratorError::NoDictionaryCombination { target_chars })?;
        let mut parts = Vec::new();
        parts
            .try_reserve_exact(plan.len())
            .map_err(|_| GeneratorError::ResourceLimit { target_chars })?;
        for index in plan {
            parts.push(
                self.query
                    .dictionary()
                    .entry(index)
                    .ok_or(GeneratorError::InvalidDictionaryEntry { index })?,
            );
        }
        let mut output = String::new();
        let bytes = target_chars
            .checked_mul(4)
            .ok_or(GeneratorError::ResourceLimit { target_chars })?;
        output
            .try_reserve_exact(bytes)
            .map_err(|_| GeneratorError::ResourceLimit { target_chars })?;
        self.query
            .separator()
            .write_separated(&parts, &mut output)
            .map_err(GeneratorError::InvalidSeparator)?;
        let actual_chars = output.chars().count();
        if actual_chars != target_chars {
            return Err(GeneratorError::SeparatorCharacterCountMismatch {
                expected_chars: target_chars,
                actual_chars,
            });
        }
        Ok(output)
    }
}

/// Generate one password. Its character count is measured in Unicode scalar values, not bytes.
pub fn generate_once(query: &Query) -> Result<String, GeneratorError> {
    let mut rng = rand::rng();
    generate_once_with_rng(query, &mut rng)
}

/// Deterministic injection point for callers and tests that provide their own RNG.
pub fn generate_once_with_rng(query: &Query, rng: &mut impl Rng) -> Result<String, GeneratorError> {
    PasswordGenerator::new(query).next_with_rng(rng)
}

pub fn generate_multi(query: &Query, count: usize) -> Result<Vec<String>, GeneratorError> {
    let mut rng = rand::rng();
    generate_multi_with_rng(query, count, &mut rng)
}

pub fn generate_multi_with_rng(
    query: &Query,
    count: usize,
    rng: &mut impl Rng,
) -> Result<Vec<String>, GeneratorError> {
    validate_batch(query, count, true)?;
    let mut generator = PasswordGenerator::new(query);
    let mut passwords = Vec::new();
    passwords
        .try_reserve_exact(count)
        .map_err(|_| GeneratorError::BatchLimitExceeded { count })?;
    for _ in 0..count {
        passwords.push(generator.next_with_rng(rng)?);
    }
    Ok(passwords)
}

fn validate_batch(
    query: &Query<'_, '_>,
    count: usize,
    collected: bool,
) -> Result<(), GeneratorError> {
    let maximum = match query.password_length() {
        PasswordLength::Exact(n) => *n,
        PasswordLength::Range { max_chars, .. } => *max_chars,
    };
    if count > query.limits().max_passwords()
        || (collected
            && maximum
                .checked_mul(count)
                .is_none_or(|total| total > query.limits().max_total_chars()))
    {
        return Err(GeneratorError::BatchLimitExceeded { count });
    }
    Ok(())
}

/// Generates lazily, stops after the first failure, and retains no output batch.
pub fn generate_stream<'q, 'a, 'entries>(
    query: &'q Query<'a, 'entries>,
    count: usize,
) -> Result<impl Iterator<Item = Result<String, GeneratorError>> + 'q, GeneratorError>
where
    'a: 'q,
    'entries: 'q,
{
    validate_batch(query, count, false)?;
    let mut generator = PasswordGenerator::new(query);
    let mut rng = rand::rng();
    let mut remaining = count;
    Ok(std::iter::from_fn(move || {
        if remaining == 0 {
            return None;
        }
        remaining -= 1;
        let result = generator.next_with_rng(&mut rng);
        if result.is_err() {
            remaining = 0;
        }
        Some(result)
    }))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::dictionary::{cache::DictionaryCache, file_dictionary::FileDictionary};
    use crate::query::PasswordLength;
    use crate::separator::Separator;
    use crate::separator::without_separator::WithoutSeparator;
    use crate::separator::{
        between_parts::BetweenPartsSeparator, fixed_count::FixedCountSeparator,
        fixed_interval::FixedIntervalSeparator,
    };
    use rand::{SeedableRng, rngs::StdRng};

    #[test]
    fn generates_exact_unicode_char_count_with_multichar_entries() {
        let dictionary =
            DictionaryCache::new(FileDictionary::from_entries(["é", "🦀", "猫"]).unwrap()).unwrap();
        let separator = WithoutSeparator;
        let query = Query::new(PasswordLength::Exact(7), &dictionary, &separator).unwrap();
        let mut rng = StdRng::seed_from_u64(7);
        let password = generate_once_with_rng(&query, &mut rng).unwrap();
        assert_eq!(password.chars().count(), 7);
    }

    #[test]
    fn impossible_char_count_is_reported_without_retrying_forever() {
        let dictionary =
            DictionaryCache::new(FileDictionary::from_entries(["ab"]).unwrap()).unwrap();
        let separator = WithoutSeparator;
        let query = Query::new(PasswordLength::Exact(3), &dictionary, &separator).unwrap();
        let mut rng = StdRng::seed_from_u64(1);
        assert_eq!(
            generate_once_with_rng(&query, &mut rng),
            Err(GeneratorError::NoDictionaryCombination { target_chars: 3 })
        );
    }

    #[test]
    fn query_rejects_invalid_password_char_count_before_generation() {
        let dictionary =
            DictionaryCache::new(FileDictionary::from_entries(["x"]).unwrap()).unwrap();
        let separator = WithoutSeparator;
        assert!(Query::new(PasswordLength::Exact(0), &dictionary, &separator).is_err());
    }

    #[test]
    fn zero_count_returns_empty_collection() {
        let dictionary =
            DictionaryCache::new(FileDictionary::from_entries(["x"]).unwrap()).unwrap();
        let separator = WithoutSeparator;
        let query = Query::new(PasswordLength::Exact(2), &dictionary, &separator).unwrap();
        let mut rng = StdRng::seed_from_u64(1);
        assert!(
            generate_multi_with_rng(&query, 0, &mut rng)
                .unwrap()
                .is_empty()
        );
    }

    #[test]
    fn exact_char_count_is_respected_for_each_separator_strategy() {
        let dictionary =
            DictionaryCache::new(FileDictionary::from_entries(["a", "é", "xy"]).unwrap()).unwrap();
        let separators: [&dyn Separator; 4] = [
            &WithoutSeparator,
            &BetweenPartsSeparator::new("--").unwrap(),
            &FixedIntervalSeparator::new("🟠", 3).unwrap(),
            &FixedCountSeparator::new("::", 2).unwrap(),
        ];
        for (index, separator) in separators.into_iter().enumerate() {
            let query = Query::new(PasswordLength::Exact(13), &dictionary, separator).unwrap();
            let mut rng = StdRng::seed_from_u64(13);
            let result = generate_once_with_rng(&query, &mut rng)
                .unwrap_or_else(|error| panic!("separator {index}: {error}"));
            assert_eq!(result.chars().count(), 13);
        }
    }

    #[test]
    fn long_equal_length_passwords_do_not_exhaust_search_budget() {
        let dictionary =
            DictionaryCache::new(FileDictionary::from_entries(["é", "猫"]).unwrap()).unwrap();
        let none = WithoutSeparator;
        let between = BetweenPartsSeparator::new("::").unwrap();
        for separator in [&none as &dyn Separator, &between] {
            let query = Query::with_limits(
                PasswordLength::Exact(10_000),
                &dictionary,
                separator,
                crate::query::GenerationLimits::new(1, 1),
            )
            .unwrap();
            let mut rng = StdRng::seed_from_u64(1);
            let passwords = generate_multi_with_rng(&query, 3, &mut rng).unwrap();
            assert!(passwords.iter().all(|p| p.chars().count() == 10_000));
            assert_ne!(passwords[0], passwords[1]);
        }
    }

    #[test]
    fn exact_lengths_match_reachability_for_sparse_word_lengths() {
        // Independent one-dimensional reachability oracle for unrestricted parts.
        for entries in [
            vec!["aa", "bbbbb"],
            vec!["猫猫猫", "ééééééé"],
            vec!["x", "yyyy"],
        ] {
            let dictionary =
                DictionaryCache::new(FileDictionary::from_entries(entries.clone()).unwrap())
                    .unwrap();
            let mut reachable = [false; 81];
            reachable[0] = true;
            for n in 1..reachable.len() {
                reachable[n] = entries.iter().any(|entry| {
                    let length = entry.chars().count();
                    length <= n && reachable[n - length]
                });
                let query =
                    Query::new(PasswordLength::Exact(n), &dictionary, &WithoutSeparator).unwrap();
                let result = generate_once(&query);
                assert_eq!(
                    result.is_ok(),
                    reachable[n],
                    "target {n}, entries {entries:?}"
                );
                if let Ok(password) = result {
                    assert_eq!(password.chars().count(), n);
                }
            }
        }
    }

    #[test]
    fn random_shapes_include_single_words_and_combinations() {
        let dictionary =
            DictionaryCache::new(FileDictionary::from_entries(["abcd", "a"]).unwrap()).unwrap();
        let separator = WithoutSeparator;
        let first = Query::new(PasswordLength::Exact(4), &dictionary, &separator).unwrap();
        assert_eq!(generate_once(&first).unwrap(), "abcd");
        let random = first.with_shape_selection(ShapeSelection::Random);
        let mut rng = StdRng::seed_from_u64(2);
        let passwords = generate_multi_with_rng(&random, 40, &mut rng).unwrap();
        assert!(passwords.iter().any(|p| p == "abcd"));
        assert!(passwords.iter().any(|p| p == "aaaa"));
    }

    #[test]
    fn output_limits_reject_oversized_batches_but_stream_without_collecting() {
        let dictionary =
            DictionaryCache::new(FileDictionary::from_entries(["a"]).unwrap()).unwrap();
        let limits = crate::query::GenerationLimits::new(1, 1).with_output_limits(8, 10, 12);
        assert!(
            Query::with_limits(
                PasswordLength::Exact(9),
                &dictionary,
                &WithoutSeparator,
                limits
            )
            .is_err()
        );
        assert!(
            Query::with_limits(
                PasswordLength::Exact(1),
                &dictionary,
                &WithoutSeparator,
                crate::query::GenerationLimits::new(0, 1)
            )
            .is_err()
        );
        let query = Query::with_limits(
            PasswordLength::Exact(8),
            &dictionary,
            &WithoutSeparator,
            limits,
        )
        .unwrap();
        assert!(matches!(
            generate_multi(&query, 2),
            Err(GeneratorError::BatchLimitExceeded { .. })
        ));
        assert_eq!(
            generate_stream(&query, 10)
                .unwrap()
                .filter_map(Result::ok)
                .count(),
            10
        );
        assert!(generate_stream(&query, 11).is_err());
    }
}
