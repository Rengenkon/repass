use crate::dictionary::static_dictionary::SymbolicDictionary;

pub fn google<'a>() -> SymbolicDictionary<'a> {
    let mut s = SymbolicDictionary::new();
    s.add_all();
    s
}

pub fn yandex<'a>() -> SymbolicDictionary<'a> {
    let mut s = SymbolicDictionary::new();
    s.add_all();
    s
}