use super::{GenerateArgs, PresetKind, SeparatorKind, ShapeKind};
use crate::Result;
use repass_generator::dictionary::{
    cache::DictionaryCache, file_dictionary::FileDictionary, preset_dictionary::PresetDictionary,
    presets,
};
use repass_generator::generator::generate_multi;
use repass_generator::query::{PasswordLength, Query, ShapeSelection};
use repass_generator::separator::{
    DEFAULT_SEPARATOR, Separator, between_parts::BetweenPartsSeparator,
    fixed_count::FixedCountSeparator, fixed_interval::FixedIntervalSeparator,
    without_separator::WithoutSeparator,
};

pub fn generate(args: GenerateArgs, default_warnings: bool) -> Result<(Vec<String>, Vec<String>)> {
    let warnings = if args.warnings_enabled(default_warnings) {
        ignored_separator_options(&args)
    } else {
        Vec::new()
    };
    let dictionary = match args.dictionary {
        Some(path) => DictionaryCache::new(FileDictionary::from_path(path)?)?,
        None if args.preset.is_empty() => DictionaryCache::new(presets::all_presets())?,
        None => {
            let mut dictionary = PresetDictionary::new();
            for preset in args.preset {
                match preset {
                    PresetKind::Digits => dictionary.add_digits(),
                    PresetKind::Lowercase => dictionary.add_lowercase_letters(),
                    PresetKind::Uppercase => dictionary.add_uppercase_letters(),
                    PresetKind::Punctuation => dictionary.add_punctuation(),
                    PresetKind::Symbols => dictionary.add_miscellaneous_symbols(),
                    PresetKind::Brackets => dictionary.add_brackets(),
                    PresetKind::Quotes => dictionary.add_quotes_and_ampersand_symbols(),
                    PresetKind::HashDollarPercentCaret => {
                        dictionary.add_hash_dollar_percent_caret_symbols()
                    }
                    PresetKind::BackslashPipeTilde => dictionary.add_backslash_pipe_tilde_symbols(),
                };
            }
            DictionaryCache::new(dictionary)?
        }
    };
    let text = args.separator.as_deref().unwrap_or(DEFAULT_SEPARATOR);
    let separator: Box<dyn Separator> = match args.separator_kind {
        SeparatorKind::None => Box::new(WithoutSeparator),
        SeparatorKind::BetweenParts => Box::new(BetweenPartsSeparator::new(text)?),
        SeparatorKind::FixedInterval => Box::new(FixedIntervalSeparator::new(
            text,
            args.separator_interval.unwrap_or(5),
        )?),
        SeparatorKind::FixedCount => Box::new(FixedCountSeparator::new(
            text,
            args.separator_count.unwrap_or(3),
        )?),
    };
    let length = match (args.length, args.min_length, args.max_length) {
        (Some(n), None, None) => PasswordLength::Exact(n),
        (None, Some(min_chars), Some(max_chars)) => PasswordLength::Range {
            min_chars,
            max_chars,
        },
        _ => return Err("specify --length or both --min-length and --max-length".into()),
    };
    let selection = match args.shape_selection {
        ShapeKind::First => ShapeSelection::First,
        ShapeKind::Random => ShapeSelection::Random,
    };
    let query = Query::new(length, &dictionary, &*separator)?.with_shape_selection(selection);
    Ok((generate_multi(&query, args.count)?, warnings))
}

fn ignored_separator_options(args: &GenerateArgs) -> Vec<String> {
    let mut ignored = Vec::new();
    if args.separator_kind == SeparatorKind::None && args.separator.is_some() {
        ignored.push("--separator");
    }
    if args.separator_interval.is_some() && args.separator_kind != SeparatorKind::FixedInterval {
        ignored.push("--separator-interval");
    }
    if args.separator_count.is_some() && args.separator_kind != SeparatorKind::FixedCount {
        ignored.push("--separator-count");
    }
    ignored
        .into_iter()
        .map(|option| {
            format!(
                "{option} is ignored for separator strategy {}",
                separator_kind_name(args.separator_kind)
            )
        })
        .collect()
}

fn separator_kind_name(kind: SeparatorKind) -> &'static str {
    match kind {
        SeparatorKind::None => "none",
        SeparatorKind::BetweenParts => "between-parts",
        SeparatorKind::FixedInterval => "fixed-interval",
        SeparatorKind::FixedCount => "fixed-count",
    }
}
