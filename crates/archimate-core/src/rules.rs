//! Which relationships ArchiMate 3.2 allows between two concepts.
//!
//! The table is Archi's own `relationships.xml` (MIT licence, see data/ARCHI-LICENSE.txt),
//! embedded at compile time and parsed once.

use crate::metamodel::{ElementType, RelationshipType};
use crate::xml;
use std::collections::HashMap;
use std::sync::OnceLock;

const RELATIONSHIPS_XML: &str = include_str!("../data/relationships.xml");

/// A source or target in the rules table: an element type, or "any relationship".
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Concept {
    Element(ElementType),
    Relationship,
}

pub struct Rules {
    allowed: HashMap<(Concept, Concept), Vec<RelationshipType>>,
}

fn concept(name: &str) -> Option<Concept> {
    if name == "Relationship" {
        Some(Concept::Relationship)
    } else {
        ElementType::from_name(name).map(Concept::Element)
    }
}

impl Rules {
    pub fn get() -> &'static Rules {
        static RULES: OnceLock<Rules> = OnceLock::new();
        RULES.get_or_init(|| {
            Rules::parse(RELATIONSHIPS_XML).expect("embedded relationships.xml is valid")
        })
    }

    fn parse(text: &str) -> Result<Rules, xml::XmlError> {
        let root = xml::parse(text)?;
        let mut allowed = HashMap::new();
        for src in root.children.iter().filter(|c| c.name == "source") {
            let Some(s) = src.attr("concept").and_then(concept) else {
                continue;
            };
            for tgt in src.children.iter().filter(|c| c.name == "target") {
                let Some(t) = tgt.attr("concept").and_then(concept) else {
                    continue;
                };
                let rels: Vec<RelationshipType> = tgt
                    .attr("relations")
                    .unwrap_or_default()
                    .chars()
                    .filter_map(RelationshipType::from_key)
                    .collect();
                allowed.insert((s, t), rels);
            }
        }
        Ok(Rules { allowed })
    }

    pub fn allowed(&self, source: Concept, target: Concept) -> &[RelationshipType] {
        self.allowed
            .get(&(source, target))
            .map(Vec::as_slice)
            .unwrap_or(&[])
    }

    pub fn is_valid(&self, kind: RelationshipType, source: Concept, target: Concept) -> bool {
        self.allowed(source, target).contains(&kind)
    }

    pub fn allowed_between(&self, source: ElementType, target: ElementType) -> &[RelationshipType] {
        self.allowed(Concept::Element(source), Concept::Element(target))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ElementType::*;
    use RelationshipType::*;

    #[test]
    fn known_rules() {
        let r = Rules::get();
        assert!(r
            .allowed_between(ApplicationService, BusinessProcess)
            .contains(&Serving));
        assert!(r
            .allowed_between(BusinessActor, BusinessRole)
            .contains(&Assignment));
        assert!(!r
            .allowed_between(BusinessObject, BusinessActor)
            .contains(&Assignment));
        // Every element may be associated with every element.
        for &a in ElementType::ALL {
            for &b in ElementType::ALL {
                assert!(
                    r.allowed_between(a, b).contains(&Association),
                    "{a:?} -> {b:?}"
                );
            }
        }
    }
}
