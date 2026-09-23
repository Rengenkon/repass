mod planner;

use crate::dictionary::cache::DictionaryCache;
use crate::error::GeneratorError;
use crate::query::{GenerationLimits, Length, Query};
use crate::separator::Separator;
use rand::{Rng, RngExt};

fn choose_target(length: &Length, rng: &mut impl Rng) -> Result<usize, GeneratorError> {
    length.validate()?;
    let target = match length {
        Length::Exact(value) => *value,
        Length::Range { min, max } => rng.random_range(*min..=*max),
    };
    Ok(target)
}

fn generate_for_target(
    dictionary: &DictionaryCache<'_>,
    separator: &dyn Separator,
    target: usize,
    rng: &mut impl Rng,
    limits: GenerationLimits,
) -> Result<String, GeneratorError> {
    separator
        .validate()
        .map_err(GeneratorError::InvalidSeparator)?;
    let mut shapes = separator
        .inputs_for_total(target)
        .map_err(GeneratorError::InvalidSeparator)?;
    let mut planner = planner::CombinationPlanner::new(dictionary, limits);
    let mut found_shape = false;
    for shape_index in 0..=limits.max_shapes() {
        let Some(shape) = shapes.next() else {
            break;
        };
        if shape_index == limits.max_shapes() {
            return Err(GeneratorError::SearchLimitExceeded {
                target,
                limit: limits.max_shapes(),
            });
        }
        found_shape = true;
        let Some(plan) = planner.find_plan(shape, rng)? else {
            continue;
        };
        let mut parts = Vec::new();
        parts
            .try_reserve_exact(plan.len())
            .map_err(|_| GeneratorError::ResourceLimit { target })?;
        for index in plan {
            parts.push(
                dictionary
                    .entry(index)
                    .ok_or(GeneratorError::InvalidDictionaryEntry { index })?,
            );
        }
        let output = separator
            .separate(&parts)
            .map_err(GeneratorError::InvalidSeparator)?;
        let actual = output.chars().count();
        if actual != target {
            return Err(GeneratorError::SeparatorLengthMismatch {
                expected: target,
                actual,
            });
        }
        return Ok(output);
    }
    if found_shape {
        Err(GeneratorError::NoDictionaryCombination { target })
    } else {
        Err(GeneratorError::NoSeparationShape { target })
    }
}

/// Generate one password. Length means Unicode scalar values, not bytes.
pub fn generate_once(query: &Query) -> Result<String, GeneratorError> {
    let mut rng = rand::rng();
    generate_once_with_rng(query, &mut rng)
}

/// Deterministic injection point for callers and tests that provide their own RNG.
pub fn generate_once_with_rng(query: &Query, rng: &mut impl Rng) -> Result<String, GeneratorError> {
    let target = choose_target(query.length(), rng)?;
    generate_for_target(
        query.dictionary(),
        query.separator(),
        target,
        rng,
        query.limits(),
    )
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
    (0..count)
        .map(|_| generate_once_with_rng(query, rng))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::dictionary::{cache::DictionaryCache, file_dictionary::FileDictionary};
    use crate::query::Length;
    use crate::separator::no_split::WithoutSeparator;
    use crate::separator::{
        fix_count::FixCountSeparator, interval::FixIntervalSeparator,
        on_parts::BetweenPartsSeparator,
    };
    use rand::{SeedableRng, rngs::StdRng};

    #[test]
    fn generates_exact_unicode_length_with_multichar_entries() {
        let dictionary =
            DictionaryCache::new(FileDictionary::from_entries(["é", "🦀", "猫"]).unwrap()).unwrap();
        let separator = WithoutSeparator;
        let query = Query::new(Length::Exact(7), &dictionary, &separator).unwrap();
        let mut rng = StdRng::seed_from_u64(7);
        let password = generate_once_with_rng(&query, &mut rng).unwrap();
        assert_eq!(password.chars().count(), 7);
    }

    #[test]
    fn impossible_length_is_reported_without_retrying_forever() {
        let dictionary =
            DictionaryCache::new(FileDictionary::from_entries(["ab"]).unwrap()).unwrap();
        let separator = WithoutSeparator;
        let query = Query::new(Length::Exact(3), &dictionary, &separator).unwrap();
        let mut rng = StdRng::seed_from_u64(1);
        assert_eq!(
            generate_once_with_rng(&query, &mut rng),
            Err(GeneratorError::NoDictionaryCombination { target: 3 })
        );
    }

    #[test]
    fn query_rejects_invalid_length_before_generation() {
        let dictionary =
            DictionaryCache::new(FileDictionary::from_entries(["x"]).unwrap()).unwrap();
        let separator = WithoutSeparator;
        assert!(Query::new(Length::Exact(0), &dictionary, &separator).is_err());
    }

    #[test]
    fn zero_count_returns_empty_collection() {
        let dictionary =
            DictionaryCache::new(FileDictionary::from_entries(["x"]).unwrap()).unwrap();
        let separator = WithoutSeparator;
        let query = Query::new(Length::Exact(2), &dictionary, &separator).unwrap();
        let mut rng = StdRng::seed_from_u64(1);
        assert!(
            generate_multi_with_rng(&query, 0, &mut rng)
                .unwrap()
                .is_empty()
        );
    }

    #[test]
    fn exact_length_is_respected_for_each_separator_strategy() {
        let dictionary =
            DictionaryCache::new(FileDictionary::from_entries(["a", "é", "xy"]).unwrap()).unwrap();
        let separators: [&dyn Separator; 4] = [
            &WithoutSeparator,
            &BetweenPartsSeparator::new("--"),
            &FixIntervalSeparator::new("🟠", 3),
            &FixCountSeparator::new("::", 2),
        ];
        for (index, separator) in separators.into_iter().enumerate() {
            let query = Query::new(Length::Exact(13), &dictionary, separator).unwrap();
            let mut rng = StdRng::seed_from_u64(13);
            let result = generate_once_with_rng(&query, &mut rng)
                .unwrap_or_else(|error| panic!("separator {index}: {error}"));
            assert_eq!(result.chars().count(), 13);
        }
    }
}
