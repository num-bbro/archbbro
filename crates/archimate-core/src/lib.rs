//! Core of an ArchiMate modelling tool: the ArchiMate 3.2 metamodel, a model that
//! round-trips Archi's `.archimate` files, the relationship rules and a validator.

pub mod archi_format;
pub mod csv_export;
pub mod example;
pub mod metamodel;
pub mod model;
pub mod rules;
pub mod validate;
pub mod xml;

pub use metamodel::{ElementType, Layer, RelationshipType};
pub use model::Model;
