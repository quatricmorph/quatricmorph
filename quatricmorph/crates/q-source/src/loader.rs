//! Source code loader module

use std::path::Path;

pub struct Loader;

impl Loader {
    pub fn new() -> Self {
        Self
    }

    pub fn load<P: AsRef<Path>>(&self, _path: P) -> Result<String, Box<dyn std::error::Error>> {
        todo!("Implement loader")
    }
}
