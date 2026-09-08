pub mod file_dictionary;
pub mod static_dictionary;
pub mod services;

pub trait Dictionary {
    fn get(self: &Self, index: usize) -> Option<&str>;

    fn len(self: &Self) -> usize;

    /// Simple dictionary - dictionary where every element is one char
    fn is_simple(self: &Self) -> bool;
}