use crate::dictionary::preset_dictionary::PresetDictionary;

pub fn all_presets() -> PresetDictionary<'static> {
    let mut dictionary = PresetDictionary::new();
    dictionary.add_all_presets();
    dictionary
}
