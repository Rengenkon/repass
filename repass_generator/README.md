# repass_generator

English documentation | [Документация на русском](README.ru.md)

`repass_generator` is a Rust crate for generating passwords from dictionaries. It supports exact or ranged output lengths, Unicode entries, configurable separators, and generation of one or multiple passwords.

## Quick start

```rust
use repass_generator::dictionary::{cache::DictionaryCache, file_dictionary::FileDictionary};
use repass_generator::generator::generate_once;
use repass_generator::query::{PasswordLength, Query};
use repass_generator::separator::between_parts::BetweenPartsSeparator;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let dictionary = FileDictionary::from_entries(["river", "silver", "forest"])?;
    let dictionary = DictionaryCache::new(dictionary)?;
    let separator = BetweenPartsSeparator::new("-");
    let query = Query::new(
        PasswordLength::Range {
            min_chars: 12,
            max_chars: 20,
        },
        &dictionary,
        &separator,
    )?;

    let password = generate_once(&query)?;
    println!("{password}");
    Ok(())
}
```

The output length is counted in Unicode scalar values (`str::chars().count()`), not bytes or grapheme clusters.

## Main concepts

### Dictionary

A `Dictionary` provides the non-empty strings that can be selected during generation. An entry can be a letter, word, or any other string. The crate includes:

- `FileDictionary` — an owned list of strings that can be loaded from or saved to a UTF-8 text file, one entry per line.
- `PresetDictionary` — built-in character sets, such as digits, letters, punctuation, and symbols; custom entries can also be added.
- `DictionaryCache` — a validated, indexed dictionary grouped by entry length. Build it after finishing dictionary changes; the cached dictionary should then remain unchanged.

Empty dictionaries, empty entries, and duplicate entries added through the dictionary API are rejected.

### Query and length

`Query` combines a `PasswordLength`, a `DictionaryCache`, a `Separator`, and `GenerationLimits`. Construction validates the length, dictionary, and separator.

`PasswordLength::Exact(n)` requests exactly `n` characters. `PasswordLength::Range { min_chars, max_chars }` chooses a target inclusively from the range. The default is 12 through 20 characters.

`GenerationLimits` bounds the number of separator shapes and planner states examined. Its defaults are 100,000 for each limit. If a limit is reached, generation returns an error rather than searching indefinitely.

### Separators

The `Separator` trait defines how selected entries are joined and how output length is calculated.

| Strategy | Behavior |
| --- | --- |
| `WithoutSeparator` | Concatenates entries without inserting characters. |
| `BetweenPartsSeparator` | Inserts a string between each adjacent pair of entries. |
| `FixedIntervalSeparator` | Inserts a string after each configured interval of content characters. |
| `FixedCountSeparator` | Distributes a configured number of separators across the content. |

Separators may contain multiple Unicode scalar values. The default separator for the strategies that provide a default is `-`.

### Shape and combination planning

A `SeparationShape` describes the content length and number of dictionary entries that can fit a target output length under a separator strategy. The separator exposes possible shapes lazily. The internal combination planner then looks for dictionary entries whose lengths match a shape, and the separator renders the selected entries.

This separation of responsibilities lets the generator account for separator characters when honoring an exact output length.

## Generating multiple passwords

Use `generate_multi(query, count)` to generate multiple passwords. For reproducible output in tests or other controlled contexts, use `generate_once_with_rng` or `generate_multi_with_rng` and provide an RNG.

## Errors and limits

Generation returns `Result` and reports invalid lengths, invalid dictionaries or separators, impossible target lengths, unavailable dictionary combinations, and resource or search limits. A valid query does not guarantee that every requested length can be formed from the chosen dictionary and separator.

## Terminology

| Term | Meaning | Similar terms |
| --- | --- | --- |
| Dictionary entry | One selectable string | item, value, word, fragment |
| Dictionary cache | Length-indexed dictionary used during generation | index, lookup table, prepared dictionary |
| Query | Complete generation configuration | request, specification, settings |
| Target length | Requested output character count | output size, desired length |
| Content length | Character count excluding inserted separators | payload length, raw length |
| Part | One selected dictionary entry in the result | segment, component, fragment |
| Separation shape | Content length and number of parts for a candidate output | layout, pattern, arrangement |
| Separator | Rule for inserting or omitting delimiter strings | delimiter strategy, join rule, formatter |
| Planner state | Intermediate point in the combination search | search state, candidate state |
| Generation limit | Bound on search work | cap, budget, upper bound |

## API documentation

Public modules and items are documented in the Rust API docs generated with `cargo doc --open`.
