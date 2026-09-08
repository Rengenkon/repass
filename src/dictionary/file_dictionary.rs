use crate::dictionary::Dictionary;
use std::fs::File;
use std::io::{BufRead, BufReader};
use std::path::Path;

pub struct FileDictionary {
    dictionary: Vec<String>,
    case_change: bool, // todo
    is_simple: bool,
}

impl FileDictionary {
    pub fn new(path: Box<Path>) -> Self {
        let file = File::open(path);
        let mut reader = BufReader::new(file.unwrap());
        let mut dictionary = Vec::new();
        let mut is_simple = true;
        loop {
            let mut buf = String::new();
            let n = reader.read_line(&mut buf).unwrap();
            if is_simple {
                if buf.len() > 1 {
                    is_simple = false;
                }
            }
            if n == 0 {
                break;
            }
            dictionary.push(buf);
        }
        FileDictionary {
            dictionary,
            case_change: false,
            is_simple,
        }
    }
}

impl Dictionary for FileDictionary {
    fn get(self: &Self, index: usize) -> Option<&str> {
        let x = self.dictionary.get(index)?;
        Some(x.as_str())
    }

    fn len(self: &Self) -> usize {
        self.dictionary.len()
    }

    fn is_simple(self: &Self) -> bool {
        self.is_simple
    }
}
