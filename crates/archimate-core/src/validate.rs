//! Model checks in the spirit of Archi's validator ("Hammer").

use crate::model::{DiagramNodeKind, Id, Model};
use crate::rules::{Concept, Rules};
use std::collections::{HashMap, HashSet};
use std::fmt;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Severity {
    Error,
    Warning,
    Advice,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Issue {
    pub severity: Severity,
    pub code: &'static str,
    pub message: String,
    /// The concept or view the issue is about.
    pub object: Id,
}

impl fmt::Display for Issue {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{:?} [{}] {}", self.severity, self.code, self.message)
    }
}

fn concept_of(m: &Model, id: &str) -> Option<Concept> {
    if let Some(e) = m.elements.get(id) {
        Some(Concept::Element(e.kind))
    } else if m.relationships.contains_key(id) {
        Some(Concept::Relationship)
    } else {
        None
    }
}

fn label(m: &Model, id: &str) -> String {
    match m.elements.get(id) {
        Some(e) if !e.name.is_empty() => format!("{} '{}'", e.kind.label(), e.name),
        Some(e) => format!("unnamed {}", e.kind.label()),
        None => format!("'{id}'"),
    }
}

pub fn validate(m: &Model) -> Vec<Issue> {
    let rules = Rules::get();
    let mut issues = Vec::new();
    let mut push = |severity, code, object: &str, message: String| {
        issues.push(Issue {
            severity,
            code,
            message,
            object: object.to_string(),
        })
    };

    for r in m.relationships.values() {
        match (concept_of(m, &r.source), concept_of(m, &r.target)) {
            (Some(s), Some(t)) => {
                if !rules.is_valid(r.kind, s, t) {
                    push(
                        Severity::Error,
                        "invalid-relationship",
                        &r.id,
                        format!(
                            "{} relationship is not allowed from {} to {}",
                            r.kind,
                            label(m, &r.source),
                            label(m, &r.target)
                        ),
                    );
                }
            }
            _ => push(
                Severity::Error,
                "dangling-relationship",
                &r.id,
                format!("{} relationship points at a missing concept", r.kind),
            ),
        }
    }

    let usage = m.element_usage();
    for e in m.elements.values() {
        if !usage.contains_key(e.id.as_str()) {
            push(
                Severity::Warning,
                "unused-element",
                &e.id,
                format!("{} is not used in any view", label(m, &e.id)),
            );
        }
        if e.name.trim().is_empty() {
            push(
                Severity::Warning,
                "unnamed-element",
                &e.id,
                format!("A {} has no name", e.kind.label()),
            );
        }
    }

    let mut in_views: HashSet<&str> = HashSet::new();
    for d in m.diagrams.values() {
        if d.nodes.is_empty() {
            push(
                Severity::Warning,
                "empty-view",
                &d.id,
                format!("View '{}' is empty", d.name),
            );
        }
        let node_ids: HashSet<&str> = d.flatten().iter().map(|(n, _, _)| n.id.as_str()).collect();
        for (n, _, _) in d.flatten() {
            if let DiagramNodeKind::Element(e) = &n.kind {
                if !m.elements.contains_key(e) {
                    push(
                        Severity::Error,
                        "dangling-view-object",
                        &d.id,
                        format!("View '{}' shows a missing element", d.name),
                    );
                }
            }
        }
        for c in &d.connections {
            if !node_ids.contains(c.source.as_str()) || !node_ids.contains(c.target.as_str()) {
                push(
                    Severity::Error,
                    "dangling-connection",
                    &d.id,
                    format!("View '{}' has a connection with a missing end", d.name),
                );
            }
            if let Some(r) = &c.relationship {
                in_views.insert(r.as_str());
            }
        }
    }
    for r in m.relationships.values() {
        if !in_views.contains(r.id.as_str()) {
            push(
                Severity::Advice,
                "unused-relationship",
                &r.id,
                format!(
                    "{} relationship {} -> {} is not shown in any view",
                    r.kind,
                    label(m, &r.source),
                    label(m, &r.target)
                ),
            );
        }
    }

    let mut by_name: HashMap<(crate::metamodel::ElementType, String), Vec<&str>> = HashMap::new();
    for e in m.elements.values().filter(|e| !e.name.trim().is_empty()) {
        by_name
            .entry((e.kind, e.name.trim().to_lowercase()))
            .or_default()
            .push(&e.id);
    }
    let mut dups: Vec<_> = by_name
        .into_iter()
        .filter(|(_, ids)| ids.len() > 1)
        .collect();
    dups.sort_by(|a, b| a.0.cmp(&b.0));
    for ((kind, _), ids) in dups {
        let name = &m.elements[ids[0]].name;
        push(
            Severity::Warning,
            "duplicate-element",
            ids[0],
            format!(
                "{} {} elements are named '{}'",
                ids.len(),
                kind.label(),
                name
            ),
        );
    }

    issues.sort_by(|a, b| {
        a.severity
            .cmp(&b.severity)
            .then_with(|| a.message.cmp(&b.message))
    });
    issues
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::metamodel::{ElementType, RelationshipType};

    #[test]
    fn finds_problems() {
        let mut m = Model::new("t");
        let obj = m.add_element(ElementType::BusinessObject, "Invoice");
        let actor = m.add_element(ElementType::BusinessActor, "Clerk");
        m.add_element(ElementType::BusinessActor, "clerk");
        m.add_relationship(RelationshipType::Assignment, &obj, &actor);
        m.add_diagram("Empty");
        let codes: Vec<_> = validate(&m).iter().map(|i| i.code).collect();
        for c in [
            "invalid-relationship",
            "unused-element",
            "empty-view",
            "duplicate-element",
            "unused-relationship",
        ] {
            assert!(codes.contains(&c), "missing {c} in {codes:?}");
        }
    }
}
