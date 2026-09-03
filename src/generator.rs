pub struct Dictionary {
    dictionary: Vec<String>,
}

impl Dictionary {
    pub fn new() -> Self {
        Dictionary {
            dictionary: Vec::new(),
        }
    }

    pub fn add_numbers(self: Self) {
        todo!()
    }

    fn numbers(self: Self) -> &'static [char] {
        &['0', '1', '2', '3', '4', '5', '6', '7', '8', '9']
    }

        
}

struct Generator {}

impl Generator {
    fn generate(dictionary: Dictionary) -> Vec<String> {}
}