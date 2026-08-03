//! Source code parsing and management for Quatricmorph

pub mod parser;
pub mod loader;

pub use parser::Parser;
pub use loader::Loader;

#[derive(Debug)]
pub struct Source {
    pub id: String,
    pub content: String,
}
