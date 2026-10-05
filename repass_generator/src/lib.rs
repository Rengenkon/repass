pub mod dictionary;
pub mod error;
pub mod generator;
pub mod query;
pub mod separator;

#[cfg(test)]
mod tests {
    use crate::dictionary::{cache::DictionaryCache, file_dictionary::FileDictionary};
    use crate::error::GeneratorError;
    use crate::generator::{
        generate_multi, generate_multi_with_rng, generate_once, generate_once_with_rng,
    };
    use crate::query::{GenerationLimits, PasswordLength, Query};
    use crate::separator::Separator;
    use crate::separator::between_parts::BetweenPartsSeparator;
    use crate::separator::fixed_count::FixedCountSeparator;
    use crate::separator::fixed_interval::FixedIntervalSeparator;
    use crate::separator::without_separator::WithoutSeparator;
    use rand::{SeedableRng, rngs::StdRng};

    struct PrefixSeparator;

    impl Separator for PrefixSeparator {
        fn validate(&self) -> Result<(), crate::error::SeparatorError> {
            Ok(())
        }

        fn requirement_for_output(
            &self,
            target_chars: usize,
        ) -> Result<crate::separator::SeparationRequirement, crate::error::SeparatorError> {
            if target_chars == 0 {
                return Ok(crate::separator::SeparationRequirement::Impossible { target_chars });
            }
            let content_chars = target_chars - 1;
            Ok(crate::separator::SeparationRequirement::FixedContent {
                content_chars,
                part_count: crate::separator::PartCount::Range {
                    min: 1,
                    max: content_chars,
                },
            })
        }

        fn separate(&self, parts: &[&str]) -> Result<String, crate::error::SeparatorError> {
            crate::separator::validate_parts(parts)?;
            let mut output = String::from("~");
            for part in parts {
                output.push_str(part);
            }
            Ok(output)
        }

        fn output_chars(
            &self,
            content_chars: usize,
            _part_count: usize,
        ) -> Result<usize, crate::error::SeparatorError> {
            content_chars
                .checked_add(1)
                .ok_or(crate::error::SeparatorError::CharacterCountOverflow)
        }
    }

    #[test]
    fn exact_char_count_works_with_every_separator_strategy() {
        let words = ["a", "bc", "dé", "word", "🦀"];
        let dictionary =
            DictionaryCache::new(FileDictionary::from_entries(words).unwrap()).unwrap();
        let no_separator = WithoutSeparator;
        let between_parts = BetweenPartsSeparator::new("--").unwrap();
        let interval = FixedIntervalSeparator::new("🟠", 3).unwrap();
        let fixed_count = FixedCountSeparator::new("::", 2).unwrap();
        let separators: [&dyn Separator; 4] =
            [&no_separator, &between_parts, &interval, &fixed_count];

        for (index, separator) in separators.into_iter().enumerate() {
            let query = Query::new(PasswordLength::Exact(13), &dictionary, separator).unwrap();
            let mut rng = StdRng::seed_from_u64(index as u64);
            let password = generate_once_with_rng(&query, &mut rng).unwrap();
            assert_eq!(password.chars().count(), 13, "separator {index}");
        }
    }

    #[test]
    fn password_char_count_range_and_multi_generation_work_end_to_end() {
        let dictionary = DictionaryCache::new(
            FileDictionary::from_entries(["alpha", "β", "🦀", "deux"]).unwrap(),
        )
        .unwrap();
        let separator = BetweenPartsSeparator::new("·").unwrap();
        let query = Query::new(
            PasswordLength::Range {
                min_chars: 7,
                max_chars: 15,
            },
            &dictionary,
            &separator,
        )
        .unwrap();
        let mut rng = StdRng::seed_from_u64(42);

        let passwords = generate_multi_with_rng(&query, 20, &mut rng).unwrap();
        assert_eq!(passwords.len(), 20);
        assert!(
            passwords
                .iter()
                .all(|password| (7..=15).contains(&password.chars().count()))
        );
    }

    #[test]
    fn public_generation_entrypoints_work_with_a_query() {
        let dictionary =
            DictionaryCache::new(FileDictionary::from_entries(["alpha", "beta"]).unwrap()).unwrap();
        let separator = WithoutSeparator;
        let query = Query::new(PasswordLength::Exact(10), &dictionary, &separator).unwrap();

        assert_eq!(generate_once(&query).unwrap().chars().count(), 10);
        let passwords = generate_multi(&query, 3).unwrap();
        assert_eq!(passwords.len(), 3);
        assert!(
            passwords
                .iter()
                .all(|password| password.chars().count() == 10)
        );
    }

    #[test]
    fn default_password_char_count_and_separator_work_end_to_end() {
        let dictionary = DictionaryCache::new(
            FileDictionary::from_entries(["one", "two", "three", "four"]).unwrap(),
        )
        .unwrap();
        let separator = BetweenPartsSeparator::default();
        let query = Query::new(PasswordLength::default(), &dictionary, &separator).unwrap();
        let mut rng = StdRng::seed_from_u64(3);

        let password = generate_once_with_rng(&query, &mut rng).unwrap();
        assert!((12..=20).contains(&password.chars().count()));
    }

    #[test]
    fn custom_generation_limits_are_carried_by_query() {
        let dictionary =
            DictionaryCache::new(FileDictionary::from_entries(["a", "b", "c"]).unwrap()).unwrap();
        let separator = WithoutSeparator;
        let limits = GenerationLimits::new(100, 100);
        let query =
            Query::with_limits(PasswordLength::Exact(5), &dictionary, &separator, limits).unwrap();
        assert_eq!(query.limits(), limits);
        assert_eq!(query.limits().max_shapes(), 100);
        assert_eq!(query.limits().max_planner_states(), 100);

        let mut rng = StdRng::seed_from_u64(5);
        assert_eq!(
            generate_once_with_rng(&query, &mut rng)
                .unwrap()
                .chars()
                .count(),
            5
        );
    }

    #[test]
    fn custom_separator_implementations_work_through_the_public_trait() {
        let dictionary =
            DictionaryCache::new(FileDictionary::from_entries(["alpha", "beta"]).unwrap()).unwrap();
        let separator = PrefixSeparator;
        let query = Query::new(PasswordLength::Exact(11), &dictionary, &separator).unwrap();
        let mut rng = StdRng::seed_from_u64(1);
        let generated = generate_once_with_rng(&query, &mut rng).unwrap();
        assert!(generated.starts_with('~'));
        assert_eq!(generated.chars().count(), 11);
    }

    #[test]
    fn query_rejects_invalid_password_char_counts_empty_dictionaries_and_separators() {
        let dictionary =
            DictionaryCache::new(FileDictionary::from_entries(["entry"]).unwrap()).unwrap();
        let separator = WithoutSeparator;

        assert!(matches!(
            Query::new(PasswordLength::Exact(0), &dictionary, &separator),
            Err(GeneratorError::ZeroPasswordLength)
        ));
        assert!(matches!(
            Query::new(
                PasswordLength::Range {
                    min_chars: 0,
                    max_chars: 2,
                },
                &dictionary,
                &separator,
            ),
            Err(GeneratorError::ZeroPasswordLength)
        ));
        assert!(matches!(
            Query::new(
                PasswordLength::Range {
                    min_chars: 3,
                    max_chars: 2,
                },
                &dictionary,
                &separator,
            ),
            Err(GeneratorError::InvalidPasswordLengthRange {
                min_chars: 3,
                max_chars: 2,
            })
        ));

        let empty_dictionary = FileDictionary::from_entries(std::iter::empty::<&str>());
        assert!(matches!(
            empty_dictionary,
            Err(GeneratorError::EmptyDictionary)
        ));
        let empty_entry = FileDictionary::from_entries(["", "valid"]);
        assert!(matches!(
            empty_entry,
            Err(GeneratorError::EmptyDictionaryEntry)
        ));

        assert!(matches!(
            BetweenPartsSeparator::new(""),
            Err(crate::error::SeparatorError::EmptySeparator)
        ));
    }

    #[test]
    fn impossible_dictionary_combination_is_reported() {
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
}
