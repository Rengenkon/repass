use crate::dictionary::static_dictionary::SymbolicDictionary;

pub fn google() -> SymbolicDictionary<'static> {
    let mut s = SymbolicDictionary::new();
    s.add_all();
    s
}

pub fn yandex() -> SymbolicDictionary<'static> {
    let mut s = SymbolicDictionary::new();
    s.add_all();
    s
}
