//! In-memory ArchiMate model, shaped after Archi's own model so files round-trip.

use crate::metamodel::{ElementType, RelationshipType};
use crate::xml::Node;
use std::collections::{BTreeMap, HashMap};

pub type Id = String;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum FolderKind {
    Strategy,
    Business,
    Application,
    Technology,
    Motivation,
    ImplementationMigration,
    Other,
    Relations,
    Diagrams,
    /// A user-created sub-folder.
    User,
}

impl FolderKind {
    pub const TOP_LEVEL: [FolderKind; 9] = [
        FolderKind::Strategy,
        FolderKind::Business,
        FolderKind::Application,
        FolderKind::Technology,
        FolderKind::Motivation,
        FolderKind::ImplementationMigration,
        FolderKind::Other,
        FolderKind::Relations,
        FolderKind::Diagrams,
    ];

    /// Value of Archi's `type` attribute on top-level folders.
    pub fn attr(self) -> Option<&'static str> {
        Some(match self {
            FolderKind::Strategy => "strategy",
            FolderKind::Business => "business",
            FolderKind::Application => "application",
            FolderKind::Technology => "technology",
            FolderKind::Motivation => "motivation",
            FolderKind::ImplementationMigration => "implementation_migration",
            FolderKind::Other => "other",
            FolderKind::Relations => "relations",
            FolderKind::Diagrams => "diagrams",
            FolderKind::User => return None,
        })
    }

    pub fn from_attr(s: Option<&str>) -> FolderKind {
        match s {
            Some("strategy") => FolderKind::Strategy,
            Some("business") => FolderKind::Business,
            Some("application") => FolderKind::Application,
            Some("technology") => FolderKind::Technology,
            Some("motivation") => FolderKind::Motivation,
            Some("implementation_migration") => FolderKind::ImplementationMigration,
            Some("other") => FolderKind::Other,
            Some("relations") => FolderKind::Relations,
            Some("diagrams") => FolderKind::Diagrams,
            _ => FolderKind::User,
        }
    }

    pub fn default_name(self) -> &'static str {
        match self {
            FolderKind::Strategy => "Strategy",
            FolderKind::Business => "Business",
            FolderKind::Application => "Application",
            FolderKind::Technology => "Technology & Physical",
            FolderKind::Motivation => "Motivation",
            FolderKind::ImplementationMigration => "Implementation & Migration",
            FolderKind::Other => "Other",
            FolderKind::Relations => "Relations",
            FolderKind::Diagrams => "Views",
            FolderKind::User => "New Folder",
        }
    }
}

pub type Properties = Vec<(String, String)>;

#[derive(Debug, Clone, PartialEq)]
pub enum FolderItem {
    Element(Id),
    Relationship(Id),
    Diagram(Id),
    /// Something we do not model (sketch or canvas view, ...), kept verbatim.
    Raw(Node),
}

#[derive(Debug, Clone, PartialEq)]
pub struct Folder {
    pub id: Id,
    pub name: String,
    pub kind: FolderKind,
    pub documentation: String,
    pub properties: Properties,
    pub folders: Vec<Folder>,
    pub items: Vec<FolderItem>,
}

impl Folder {
    pub fn new(id: Id, kind: FolderKind) -> Self {
        Folder {
            id,
            name: kind.default_name().to_string(),
            kind,
            documentation: String::new(),
            properties: Vec::new(),
            folders: Vec::new(),
            items: Vec::new(),
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct Element {
    pub id: Id,
    pub kind: ElementType,
    pub name: String,
    pub documentation: String,
    pub properties: Properties,
    /// Attributes we do not interpret (junction `type`, `profiles`, ...).
    pub extra_attrs: Vec<(String, String)>,
    pub extra_children: Vec<Node>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Relationship {
    pub id: Id,
    pub kind: RelationshipType,
    pub name: String,
    pub source: Id,
    pub target: Id,
    pub documentation: String,
    pub properties: Properties,
    /// `accessType`, `strength`, `directed`, ...
    pub extra_attrs: Vec<(String, String)>,
    pub extra_children: Vec<Node>,
}

#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct Bounds {
    pub x: i32,
    pub y: i32,
    /// -1 means "use the default size".
    pub width: i32,
    pub height: i32,
}

impl Bounds {
    pub const DEFAULT_WIDTH: i32 = 120;
    pub const DEFAULT_HEIGHT: i32 = 55;

    pub fn effective_size(&self) -> (i32, i32) {
        let w = if self.width <= 0 {
            Self::DEFAULT_WIDTH
        } else {
            self.width
        };
        let h = if self.height <= 0 {
            Self::DEFAULT_HEIGHT
        } else {
            self.height
        };
        (w, h)
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum DiagramNodeKind {
    /// Shows an ArchiMate element.
    Element(Id),
    Group {
        name: String,
    },
    Note {
        content: String,
    },
    /// A link to another view.
    Reference(Id),
    /// Any other diagram object, kept by its `xsi:type`.
    Other(String),
}

#[derive(Debug, Clone, PartialEq)]
pub struct DiagramNode {
    pub id: Id,
    pub kind: DiagramNodeKind,
    pub bounds: Bounds,
    pub fill_color: Option<String>,
    pub children: Vec<DiagramNode>,
    pub extra_attrs: Vec<(String, String)>,
    pub extra_children: Vec<Node>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Connection {
    pub id: Id,
    pub source: Id,
    pub target: Id,
    /// The relationship shown, or `None` for a plain line (e.g. to a note).
    pub relationship: Option<Id>,
    pub bendpoints: Vec<Node>,
    pub extra_attrs: Vec<(String, String)>,
    pub extra_children: Vec<Node>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Diagram {
    pub id: Id,
    pub name: String,
    pub documentation: String,
    pub properties: Properties,
    pub viewpoint: Option<String>,
    pub nodes: Vec<DiagramNode>,
    pub connections: Vec<Connection>,
    pub extra_attrs: Vec<(String, String)>,
    pub extra_children: Vec<Node>,
}

impl Diagram {
    pub fn new(id: Id, name: impl Into<String>) -> Self {
        Diagram {
            id,
            name: name.into(),
            documentation: String::new(),
            properties: Vec::new(),
            viewpoint: None,
            nodes: Vec::new(),
            connections: Vec::new(),
            extra_attrs: Vec::new(),
            extra_children: Vec::new(),
        }
    }

    /// Every node in the view, depth first, with its absolute position.
    pub fn flatten(&self) -> Vec<(&DiagramNode, i32, i32)> {
        fn walk<'a>(
            nodes: &'a [DiagramNode],
            ox: i32,
            oy: i32,
            out: &mut Vec<(&'a DiagramNode, i32, i32)>,
        ) {
            for n in nodes {
                let (x, y) = (ox + n.bounds.x, oy + n.bounds.y);
                out.push((n, x, y));
                walk(&n.children, x, y, out);
            }
        }
        let mut out = Vec::new();
        walk(&self.nodes, 0, 0, &mut out);
        out
    }

    pub fn node(&self, id: &str) -> Option<&DiagramNode> {
        fn find<'a>(nodes: &'a [DiagramNode], id: &str) -> Option<&'a DiagramNode> {
            nodes.iter().find_map(|n| {
                if n.id == id {
                    Some(n)
                } else {
                    find(&n.children, id)
                }
            })
        }
        find(&self.nodes, id)
    }

    pub fn node_mut(&mut self, id: &str) -> Option<&mut DiagramNode> {
        fn find<'a>(nodes: &'a mut [DiagramNode], id: &str) -> Option<&'a mut DiagramNode> {
            for n in nodes {
                if n.id == id {
                    return Some(n);
                }
                if let Some(f) = find(&mut n.children, id) {
                    return Some(f);
                }
            }
            None
        }
        find(&mut self.nodes, id)
    }

    /// Remove a node (and its children) and every connection touching them.
    pub fn remove_node(&mut self, id: &str) {
        fn collect(n: &DiagramNode, out: &mut Vec<Id>) {
            out.push(n.id.clone());
            n.children.iter().for_each(|c| collect(c, out));
        }
        fn remove(nodes: &mut Vec<DiagramNode>, id: &str) -> Option<DiagramNode> {
            if let Some(pos) = nodes.iter().position(|n| n.id == id) {
                return Some(nodes.remove(pos));
            }
            nodes.iter_mut().find_map(|n| remove(&mut n.children, id))
        }
        if let Some(removed) = remove(&mut self.nodes, id) {
            let mut gone = Vec::new();
            collect(&removed, &mut gone);
            self.connections
                .retain(|c| !gone.contains(&c.source) && !gone.contains(&c.target));
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct Model {
    pub id: Id,
    pub name: String,
    /// Archi's model format version attribute.
    pub version: String,
    pub purpose: String,
    pub properties: Properties,
    pub folders: Vec<Folder>,
    pub elements: BTreeMap<Id, Element>,
    pub relationships: BTreeMap<Id, Relationship>,
    pub diagrams: BTreeMap<Id, Diagram>,
    pub extra_attrs: Vec<(String, String)>,
    pub extra_children: Vec<Node>,
    next_id: u64,
}

/// Archi model format version written by Archi 5.x.
pub const FORMAT_VERSION: &str = "5.0.0";

impl Model {
    /// A new, empty model with Archi's standard top-level folders.
    pub fn new(name: impl Into<String>) -> Self {
        let mut m = Model {
            id: String::new(),
            name: name.into(),
            version: FORMAT_VERSION.to_string(),
            purpose: String::new(),
            properties: Vec::new(),
            folders: Vec::new(),
            elements: BTreeMap::new(),
            relationships: BTreeMap::new(),
            diagrams: BTreeMap::new(),
            extra_attrs: Vec::new(),
            extra_children: Vec::new(),
            next_id: 0,
        };
        m.id = m.new_id();
        m.ensure_top_level_folders();
        m
    }

    pub(crate) fn empty() -> Self {
        Model {
            id: String::new(),
            name: String::new(),
            version: FORMAT_VERSION.to_string(),
            purpose: String::new(),
            properties: Vec::new(),
            folders: Vec::new(),
            elements: BTreeMap::new(),
            relationships: BTreeMap::new(),
            diagrams: BTreeMap::new(),
            extra_attrs: Vec::new(),
            extra_children: Vec::new(),
            next_id: 0,
        }
    }

    pub fn ensure_top_level_folders(&mut self) {
        for kind in FolderKind::TOP_LEVEL {
            if !self.folders.iter().any(|f| f.kind == kind) {
                let id = self.new_id();
                self.folders.push(Folder::new(id, kind));
            }
        }
    }

    /// A fresh id in Archi's style (`id-` followed by 32 hex digits).
    pub fn new_id(&mut self) -> Id {
        loop {
            self.next_id += 1;
            let id = format!("id-{}", random_hex(self.next_id));
            if !self.contains_id(&id) {
                return id;
            }
        }
    }

    fn contains_id(&self, id: &str) -> bool {
        self.elements.contains_key(id)
            || self.relationships.contains_key(id)
            || self.diagrams.contains_key(id)
    }

    pub fn top_folder_mut(&mut self, kind: FolderKind) -> &mut Folder {
        if !self.folders.iter().any(|f| f.kind == kind) {
            let id = self.new_id();
            self.folders.push(Folder::new(id, kind));
        }
        self.folders.iter_mut().find(|f| f.kind == kind).unwrap()
    }

    pub fn add_element(&mut self, kind: ElementType, name: impl Into<String>) -> Id {
        let id = self.new_id();
        self.elements.insert(
            id.clone(),
            Element {
                id: id.clone(),
                kind,
                name: name.into(),
                documentation: String::new(),
                properties: Vec::new(),
                extra_attrs: Vec::new(),
                extra_children: Vec::new(),
            },
        );
        self.top_folder_mut(kind.layer().folder())
            .items
            .push(FolderItem::Element(id.clone()));
        id
    }

    pub fn add_relationship(&mut self, kind: RelationshipType, source: &str, target: &str) -> Id {
        let id = self.new_id();
        self.relationships.insert(
            id.clone(),
            Relationship {
                id: id.clone(),
                kind,
                name: String::new(),
                source: source.to_string(),
                target: target.to_string(),
                documentation: String::new(),
                properties: Vec::new(),
                extra_attrs: Vec::new(),
                extra_children: Vec::new(),
            },
        );
        self.top_folder_mut(FolderKind::Relations)
            .items
            .push(FolderItem::Relationship(id.clone()));
        id
    }

    pub fn add_diagram(&mut self, name: impl Into<String>) -> Id {
        let id = self.new_id();
        self.diagrams
            .insert(id.clone(), Diagram::new(id.clone(), name));
        self.top_folder_mut(FolderKind::Diagrams)
            .items
            .push(FolderItem::Diagram(id.clone()));
        id
    }

    /// Place an element on a view; returns the new diagram node id.
    pub fn add_to_diagram(&mut self, diagram: &str, element: &str, x: i32, y: i32) -> Option<Id> {
        let id = self.new_id();
        let d = self.diagrams.get_mut(diagram)?;
        d.nodes.push(DiagramNode {
            id: id.clone(),
            kind: DiagramNodeKind::Element(element.to_string()),
            bounds: Bounds {
                x,
                y,
                width: Bounds::DEFAULT_WIDTH,
                height: Bounds::DEFAULT_HEIGHT,
            },
            fill_color: None,
            children: Vec::new(),
            extra_attrs: Vec::new(),
            extra_children: Vec::new(),
        });
        Some(id)
    }

    /// Draw a relationship on a view between two existing diagram nodes.
    pub fn connect_in_diagram(
        &mut self,
        diagram: &str,
        relationship: &str,
        source_node: &str,
        target_node: &str,
    ) -> Option<Id> {
        let id = self.new_id();
        let d = self.diagrams.get_mut(diagram)?;
        d.connections.push(Connection {
            id: id.clone(),
            source: source_node.to_string(),
            target: target_node.to_string(),
            relationship: Some(relationship.to_string()),
            bendpoints: Vec::new(),
            extra_attrs: Vec::new(),
            extra_children: Vec::new(),
        });
        Some(id)
    }

    /// Delete an element everywhere: its relationships, and its occurrences on every view.
    pub fn delete_element(&mut self, id: &str) {
        let rels: Vec<Id> = self
            .relationships
            .values()
            .filter(|r| r.source == id || r.target == id)
            .map(|r| r.id.clone())
            .collect();
        for r in rels {
            self.delete_relationship(&r);
        }
        self.elements.remove(id);
        for d in self.diagrams.values_mut() {
            let doomed: Vec<Id> = d
                .flatten()
                .into_iter()
                .filter(|(n, _, _)| n.kind == DiagramNodeKind::Element(id.to_string()))
                .map(|(n, _, _)| n.id.clone())
                .collect();
            for n in doomed {
                d.remove_node(&n);
            }
        }
        self.remove_from_folders(id);
    }

    pub fn delete_relationship(&mut self, id: &str) {
        // Relationships can connect to relationships; cascade.
        let dependents: Vec<Id> = self
            .relationships
            .values()
            .filter(|r| r.source == id || r.target == id)
            .map(|r| r.id.clone())
            .collect();
        self.relationships.remove(id);
        for r in dependents {
            self.delete_relationship(&r);
        }
        for d in self.diagrams.values_mut() {
            d.connections
                .retain(|c| c.relationship.as_deref() != Some(id));
        }
        self.remove_from_folders(id);
    }

    pub fn delete_diagram(&mut self, id: &str) {
        self.diagrams.remove(id);
        self.remove_from_folders(id);
    }

    fn remove_from_folders(&mut self, id: &str) {
        fn walk(f: &mut Folder, id: &str) {
            f.items.retain(|i| match i {
                FolderItem::Element(x) | FolderItem::Relationship(x) | FolderItem::Diagram(x) => {
                    x != id
                }
                FolderItem::Raw(_) => true,
            });
            f.folders.iter_mut().for_each(|s| walk(s, id));
        }
        self.folders.iter_mut().for_each(|f| walk(f, id));
    }

    /// Display name for any concept id.
    pub fn concept_name(&self, id: &str) -> Option<&str> {
        if let Some(e) = self.elements.get(id) {
            return Some(&e.name);
        }
        if let Some(r) = self.relationships.get(id) {
            return Some(&r.name);
        }
        self.diagrams.get(id).map(|d| d.name.as_str())
    }

    /// How many view references each element has.
    pub fn element_usage(&self) -> HashMap<&str, usize> {
        let mut usage: HashMap<&str, usize> = HashMap::new();
        for d in self.diagrams.values() {
            for (n, _, _) in d.flatten() {
                if let DiagramNodeKind::Element(e) = &n.kind {
                    *usage.entry(e.as_str()).or_default() += 1;
                }
            }
        }
        usage
    }
}

/// 32 hex digits, unique per process and call.
fn random_hex(counter: u64) -> String {
    use std::collections::hash_map::RandomState;
    use std::hash::{BuildHasher, Hasher};
    use std::time::{SystemTime, UNIX_EPOCH};
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    let mut out = String::with_capacity(32);
    for salt in 0..2u64 {
        let mut h = RandomState::new().build_hasher();
        h.write_u128(nanos);
        h.write_u64(counter);
        h.write_u64(salt);
        out.push_str(&format!("{:016x}", h.finish()));
    }
    out
}
