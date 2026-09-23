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
    use crate::query::{GenerationLimits, Length, Query};
    use crate::separator::Separator;
    use crate::separator::fix_count::FixCountSeparator;
    use crate::separator::interval::FixIntervalSeparator;
    use crate::separator::no_split::WithoutSeparator;
    use crate::separator::on_parts::BetweenPartsSeparator;
    use rand::{SeedableRng, rngs::StdRng};

    #[test]
    fn exact_length_works_with_every_separator_strategy() {
        let words = ["a", "bc", "dé", "word", "🦀"];
        let dictionary =
            DictionaryCache::new(FileDictionary::from_entries(words).unwrap()).unwrap();
        let no_separator = WithoutSeparator;
        let between_parts = BetweenPartsSeparator::new("--");
        let interval = FixIntervalSeparator::new("🟠", 3);
        let fixed_count = FixCountSeparator::new("::", 2);
        let separators: [&dyn Separator; 4] =
            [&no_separator, &between_parts, &interval, &fixed_count];

        for (index, separator) in separators.into_iter().enumerate() {
            let query = Query::new(Length::Exact(13), &dictionary, separator).unwrap();
            let mut rng = StdRng::seed_from_u64(index as u64);
            let password = generate_once_with_rng(&query, &mut rng).unwrap();
            assert_eq!(password.chars().count(), 13, "separator {index}");
        }
    }

    #[test]
    fn range_length_and_multi_generation_work_end_to_end() {
        let dictionary = DictionaryCache::new(
            FileDictionary::from_entries(["alpha", "β", "🦀", "deux"]).unwrap(),
        )
        .unwrap();
        let separator = BetweenPartsSeparator::new("·");
        let query = Query::new(Length::Range { min: 7, max: 15 }, &dictionary, &separator).unwrap();
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
        let query = Query::new(Length::Exact(10), &dictionary, &separator).unwrap();

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
    fn default_length_and_default_separator_work_end_to_end() {
        let dictionary = DictionaryCache::new(
            FileDictionary::from_entries(["one", "two", "three", "four"]).unwrap(),
        )
        .unwrap();
        let separator = BetweenPartsSeparator::default();
        let query = Query::new(Length::default(), &dictionary, &separator).unwrap();
        let mut rng = StdRng::seed_from_u64(3);

        let password = generate_once_with_rng(&query, &mut rng).unwrap();
        assert!((12..=20).contains(&password.chars().count()));
    }

    #[test]
    fn custom_generation_limits_are_carried_by_query() {
        let dictionary =
            DictionaryCache::new(FileDictionary::from_entries(["a", "b", "c"]).unwrap()).unwrap();
        let separator = WithoutSeparator;
        let limits = GenerationLimits {
            max_shapes: 100,
            max_planner_states: 100,
        };
        let query = Query::with_limits(Length::Exact(5), &dictionary, &separator, limits).unwrap();
        assert_eq!(query.limits(), limits);

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
    fn query_rejects_invalid_lengths_empty_dictionaries_and_separators() {
        let dictionary =
            DictionaryCache::new(FileDictionary::from_entries(["entry"]).unwrap()).unwrap();
        let separator = WithoutSeparator;

        assert!(matches!(
            Query::new(Length::Exact(0), &dictionary, &separator),
            Err(GeneratorError::ZeroLength)
        ));
        assert!(matches!(
            Query::new(Length::Range { min: 0, max: 2 }, &dictionary, &separator),
            Err(GeneratorError::ZeroLength)
        ));
        assert!(matches!(
            Query::new(Length::Range { min: 3, max: 2 }, &dictionary, &separator),
            Err(GeneratorError::InvalidLengthRange { min: 3, max: 2 })
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

        let empty_separator = BetweenPartsSeparator::new("");
        assert!(matches!(
            Query::new(Length::Exact(5), &dictionary, &empty_separator),
            Err(GeneratorError::InvalidSeparator(_))
        ));
    }

    #[test]
    fn impossible_dictionary_combination_is_reported() {
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
}
