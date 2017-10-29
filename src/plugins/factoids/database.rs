use std::fmt;

use std::collections::HashMap;

pub trait Database: Send + Sync + fmt::Debug {
    fn insert(&mut self, name: String, content: String) -> Option<String>;
    fn get(&self, name: &str) -> Option<&str>;
}

impl Database for HashMap<String, String> {
    fn insert(&mut self, name: String, content: String) -> Option<String> {
        self.insert(name, content)
    }

    fn get(&self, name: &str) -> Option<&str> {
        self.get(name).map(String::as_str)
    }
}
