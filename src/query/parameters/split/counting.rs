use super::{IllegalArgumentError, SeparatorStrategy, SeparatorInternal};
use core::str;
use std::cmp::Ordering;

#[derive(Debug)]
pub struct FixIntervalSeparator<'a> {
    separator: &'a str,
    length: usize,
}

impl<'a> FixIntervalSeparator<'a> {
    pub fn new(separator: &'a str, length: usize) -> Self {
        Self {
            separator,
            length,
        }
    }
}

impl SeparatorInternal for FixIntervalSeparator<'_> {
    fn add_separator(self: &Self, parts: &[&str]) -> String {
        let mut separated = String::new();
        let mut iter = parts.iter();
        let mut cmp_result = Ordering::Less;
        let mut value = "";
        let mut diff = self.length;

        loop {
            if cmp_result != Ordering::Greater {
                let part = iter.next();
                if part.is_none() {
                    break;
                }
                value = part.unwrap();
                if !value.is_empty() && cmp_result == Ordering::Equal {
                    separated.push_str(self.separator);
                    diff = self.length;
                }
            } else {
                separated.push_str(self.separator);
                diff = self.length;
            }
            cmp_result = value.len().cmp(&diff);
            match cmp_result {
                Ordering::Less | Ordering::Equal => {
                    separated.push_str(value);
                    diff -= value.len();
                }
                Ordering::Greater => {
                    let (left, right) = value.split_at(diff);
                    separated.push_str(left);
                    value = right;
                }
            }
        }
        separated

    }

    fn length_with_separators(self: &Self, parts: &[&str]) -> usize {
        let sum_length = Self::get_summary_length(parts);
        let mut separators_count = sum_length / self.length;
        if separators_count > 0 && sum_length % self.length == 0 {
            separators_count -= 1;
        }
        sum_length + self.separator.len() * separators_count

    }

    fn chack_errors(self: &Self, parts: &[&str]) -> Vec<IllegalArgumentError> {
        let mut errors = Vec::new();
        if self.separator.is_empty() {
            errors.push(IllegalArgumentError::EmptySeparator)
        }
        if self.length == 0 {
            errors.push(IllegalArgumentError::AdditionalParameterIsZero)
        }
        if parts.is_empty() {
            errors.push(IllegalArgumentError::SummaryLengthOfPartsIsZero)
        }
        if Self::get_summary_length(parts) == 0 {
            errors.push(IllegalArgumentError::SummaryLengthOfPartsIsZero)
        }
        errors
    }
}

impl SeparatorStrategy for FixIntervalSeparator<'_> {}

#[cfg(test)]
mod tests {

}
