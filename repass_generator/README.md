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
    let separator = BetweenPartsSeparator::new("-")?;
    let query = Query::new(
        PasswordLength::Exact(18),
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

Empty dictionaries and empty entries are rejected. Duplicate values from file/entry constructors and custom additions are rejected; overlapping built-in preset selections are combined as a unique set. Every dictionary is validated again when building its cache.

### Query and length

`Query` combines a `PasswordLength`, a `DictionaryCache`, a `Separator`, and `GenerationLimits`. Construction validates the length, dictionary, and separator.

`PasswordLength::Exact(n)` requests exactly `n` characters. `PasswordLength::Range { min_chars, max_chars }` chooses a target inclusively from the range. The default is 12 through 20 characters.

`GenerationLimits` bounds the number of candidate separator shapes and newly searched planner states (100,000 each by default). It also bounds individual output lengths (1,000,000 Unicode scalars), batch counts (100,000), and collected batch lengths (16,000,000 scalars, conservatively computed from the maximum target length). Use `with_output_limits` to customize output bounds. Zero limits are rejected when constructing a query. If a limit is exceeded, generation returns an error.

### Separators

The `Separator` trait defines how selected entries are joined and how output length is calculated.

Custom implementations are supported. A strategy must provide a finite sequence of feasible shapes, and its `output_chars` and rendered `separate` output must agree in Unicode scalar count. The generator checks the rendered count and limits shape search. Built-in configurable separators validate their arguments in `new` and return `Result`.

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

By default, the generator selects the first feasible shape and randomizes feasible entry lengths and entries within it. Use `Query::with_shape_selection(ShapeSelection::Random)` to choose uniformly among feasible shapes. Neither mode samples uniformly among all possible passwords. Bounds from dictionary entry lengths prune impossible shapes, and both successful and failed reachability checks are memoized. Equal-length dictionaries use a direct feasibility calculation.

## Generating multiple passwords

Use `generate_multi(query, count)` to generate multiple passwords. For reproducible output in tests or other controlled contexts, use `generate_once_with_rng` or `generate_multi_with_rng` and provide an RNG.

Batch generation reuses feasible shapes and reachability instead of rebuilding the search for every password. `PasswordGenerator` provides the same reusable state for callers that supply an RNG. `generate_stream(query, count)` returns a lazy iterator, keeps no output collection, and stops after its first error. Streaming applies individual-length and count limits but not the collected-batch length limit. Cached target shapes and search states are periodically discarded to bound retained search data.

For ranged lengths, a target is sampled from the entire inclusive range. An unreachable sampled length is an error; the generator does not silently retry with another length. The quick-start example uses a reachable exact length so that it does not fail randomly.

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
