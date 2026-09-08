use crate::dictionary::Dictionary;
use crate::query::Query;
use crate::separator::Separator;

fn generate_parts_unknown<'a>(
    dictionary: &Box<&'a dyn Dictionary>,
    separator: &Box<&dyn Separator>,
    target_length: usize,
) -> Vec<&'a str> {
    let len = dictionary.len();
    let mut result = Vec::new();
    let mut none_count = 0;
    while separator.length_after_separate(&result) < target_length {
        let index = rand::random_range(0..len);
        match dictionary.get(index) {
            None => none_count += 1,
            Some(value) => result.push(value),
        }
        if none_count > 100 {
            panic!("Not valid dictionary: attempts to get element is unsuccessful")
        }
    }
    result
}

/// Generate sequence of parts from `dictionary`
/// not guarantee that summary length of returned value equals `space`
fn generate_parts<'a>(dictionary: &Box<&'a dyn Dictionary>, space: usize) -> Vec<&'a str> {
    let len = dictionary.len();
    let mut result = Vec::new();
    let mut none_count = 0;
    let mut current_len = 0;
    while current_len < space {
        let index = rand::random_range(0..len);
        match dictionary.get(index) {
            None => none_count += 1,
            Some(value) => {
                result.push(value);
                current_len += value.len();
            }
        }
        if none_count > 100 {
            panic!("Not valid dictionary: attempts to get element is unsuccessful")
        }
    }
    result
}

fn generate(
    dictionary: &Box<&dyn Dictionary>,
    separator: &Box<&dyn Separator>,
    space: Option<usize>,
    target_length: usize,
) -> String {
    let parts = match space {
        None => generate_parts_unknown(&dictionary, &separator, target_length),
        Some(value) => generate_parts(&dictionary, value),
    };
    separator.add_separator(parts.as_slice())
}

pub fn generate_once(query: &Query) -> String {
    let target_length = query.length.to_one_size();
    let space = query.separator.try_compute_free_space(target_length);
    generate(&query.dictionary, &query.separator, space, target_length)
}

pub fn generate_multi(query: &Query, count: usize) -> Vec<String> {
    let target_length = query.length.to_one_size();
    let space = query.separator.try_compute_free_space(target_length);
    let mut result = Vec::new();
    for _ in 0..count {
        let generate = generate(&query.dictionary, &query.separator, space, target_length);
        result.push(generate);
    }
    result
}
