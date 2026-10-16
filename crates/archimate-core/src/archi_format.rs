//! Reading and writing Archi's native `.archimate` XML format.

use crate::metamodel::{ElementType, RelationshipType};
use crate::model::*;
use crate::xml::{self, Node};
use std::collections::HashMap;
use std::path::Path;

const NS_XSI: &str = "http://www.w3.org/2001/XMLSchema-instance";
const NS_ARCHIMATE: &str = "http://www.archimatetool.com/archimate";

#[derive(Debug, thiserror::Error)]
pub enum FormatError {
    #[error(transparent)]
    Xml(#[from] xml::XmlError),
    #[error(transparent)]
    Io(#[from] std::io::Error),
    #[error("not an Archi model: root element is <{0}>")]
    NotAModel(String),
    #[error("this file is a zipped Archi archive (model with images); unzip it and open model.archimate")]
    Archive,
}

pub fn load(path: impl AsRef<Path>) -> Result<Model, FormatError> {
    let bytes = std::fs::read(path)?;
    if bytes.starts_with(b"PK\x03\x04") {
        return Err(FormatError::Archive);
    }
    let text = String::from_utf8_lossy(&bytes);
    from_str(&text)
}

pub fn save(model: &Model, path: impl AsRef<Path>) -> Result<(), FormatError> {
    let path = path.as_ref();
    // Write next to the target and rename, so a failed save never truncates the model.
    let tmp = path.with_extension("archimate.tmp");
    std::fs::write(&tmp, to_string(model))?;
    std::fs::rename(&tmp, path)?;
    Ok(())
}

pub fn from_str(text: &str) -> Result<Model, FormatError> {
    let root = xml::parse(text)?;
    if root.name != "archimate:model" && root.name != "model" {
        return Err(FormatError::NotAModel(root.name));
    }
    let mut m = Model::empty();
    for (k, v) in &root.attrs {
        match k.as_str() {
            "name" => m.name = v.clone(),
            "id" => m.id = v.clone(),
            "version" => m.version = v.clone(),
            _ if k.starts_with("xmlns") => {}
            _ => m.extra_attrs.push((k.clone(), v.clone())),
        }
    }
    for child in &root.children {
        match child.name.as_str() {
            "folder" => {
                let f = read_folder(child, &mut m);
                m.folders.push(f);
            }
            "purpose" => m.purpose = child.text.clone(),
            "property" => m.properties.push(read_property(child)),
            _ => m.extra_children.push(child.clone()),
        }
    }
    if m.id.is_empty() {
        m.id = m.new_id();
    }
    Ok(m)
}

fn read_property(n: &Node) -> (String, String) {
    (
        n.attr("key").unwrap_or_default().to_string(),
        n.attr("value").unwrap_or_default().to_string(),
    )
}

fn xsi_type(n: &Node) -> &str {
    n.attr("xsi:type")
        .map(|t| t.strip_prefix("archimate:").unwrap_or(t))
        .unwrap_or("")
}

/// Split a concept's attributes into the ones we model and the rest.
fn common_attrs(n: &Node, known: &[&str]) -> Vec<(String, String)> {
    n.attrs
        .iter()
        .filter(|(k, _)| {
            k != "xsi:type" && k != "id" && k != "name" && !known.contains(&k.as_str())
        })
        .cloned()
        .collect()
}

/// Documentation and properties, with everything else returned as extras.
fn doc_props(n: &Node) -> (String, Properties, Vec<Node>) {
    let mut doc = n.attr("documentation").unwrap_or_default().to_string();
    let mut props = Vec::new();
    let mut extra = Vec::new();
    for c in &n.children {
        match c.name.as_str() {
            "documentation" => doc = c.text.clone(),
            "property" => props.push(read_property(c)),
            _ => extra.push(c.clone()),
        }
    }
    (doc, props, extra)
}

fn read_folder(n: &Node, m: &mut Model) -> Folder {
    let kind = FolderKind::from_attr(n.attr("type"));
    let mut f = Folder::new(
        n.attr("id")
            .map(str::to_string)
            .unwrap_or_else(|| m.new_id()),
        kind,
    );
    f.name = n.attr("name").unwrap_or(kind.default_name()).to_string();
    for c in &n.children {
        match c.name.as_str() {
            "documentation" => f.documentation = c.text.clone(),
            "property" => f.properties.push(read_property(c)),
            "folder" => {
                let sub = read_folder(c, m);
                f.folders.push(sub);
            }
            "element" => f.items.push(read_concept(c, m)),
            _ => f.items.push(FolderItem::Raw(c.clone())),
        }
    }
    f
}

fn read_concept(n: &Node, m: &mut Model) -> FolderItem {
    let ty = xsi_type(n);
    let id = n
        .attr("id")
        .map(str::to_string)
        .unwrap_or_else(|| m.new_id());
    let name = n.attr("name").unwrap_or_default().to_string();
    if let Some(kind) = ElementType::from_name(ty) {
        let (documentation, properties, extra_children) = doc_props(n);
        m.elements.insert(
            id.clone(),
            Element {
                id: id.clone(),
                kind,
                name,
                documentation,
                properties,
                extra_attrs: common_attrs(n, &["documentation"]),
                extra_children,
            },
        );
        FolderItem::Element(id)
    } else if let Some(kind) = RelationshipType::from_name(ty) {
        let (documentation, properties, extra_children) = doc_props(n);
        m.relationships.insert(
            id.clone(),
            Relationship {
                id: id.clone(),
                kind,
                name,
                source: n.attr("source").unwrap_or_default().to_string(),
                target: n.attr("target").unwrap_or_default().to_string(),
                documentation,
                properties,
                extra_attrs: common_attrs(n, &["source", "target", "documentation"]),
                extra_children,
            },
        );
        FolderItem::Relationship(id)
    } else if ty == "ArchimateDiagramModel" {
        let mut d = Diagram::new(id.clone(), name);
        d.viewpoint = n.attr("viewpoint").map(str::to_string);
        d.extra_attrs = common_attrs(n, &["viewpoint", "documentation"]);
        d.documentation = n.attr("documentation").unwrap_or_default().to_string();
        let mut conns = Vec::new();
        for c in &n.children {
            match c.name.as_str() {
                "documentation" => d.documentation = c.text.clone(),
                "property" => d.properties.push(read_property(c)),
                "child" => d.nodes.push(read_node(c, m, &mut conns)),
                _ => d.extra_children.push(c.clone()),
            }
        }
        d.connections = conns;
        m.diagrams.insert(id.clone(), d);
        FolderItem::Diagram(id)
    } else {
        // Sketch, canvas or unknown: keep exactly as read.
        FolderItem::Raw(n.clone())
    }
}

fn read_node(n: &Node, m: &mut Model, conns: &mut Vec<Connection>) -> DiagramNode {
    let ty = xsi_type(n);
    let kind = match ty {
        "DiagramObject" => {
            DiagramNodeKind::Element(n.attr("archimateElement").unwrap_or_default().to_string())
        }
        "Group" => DiagramNodeKind::Group {
            name: n.attr("name").unwrap_or_default().to_string(),
        },
        "Note" => DiagramNodeKind::Note {
            content: n
                .child("content")
                .map(|c| c.text.clone())
                .unwrap_or_default(),
        },
        "DiagramModelReference" => {
            DiagramNodeKind::Reference(n.attr("model").unwrap_or_default().to_string())
        }
        other => DiagramNodeKind::Other(other.to_string()),
    };
    let mut node = DiagramNode {
        id: n
            .attr("id")
            .map(str::to_string)
            .unwrap_or_else(|| m.new_id()),
        kind,
        bounds: Bounds::default(),
        fill_color: n.attr("fillColor").map(str::to_string),
        children: Vec::new(),
        extra_attrs: n
            .attrs
            .iter()
            .filter(|(k, _)| {
                !matches!(
                    k.as_str(),
                    "xsi:type"
                        | "id"
                        | "fillColor"
                        | "archimateElement"
                        | "model"
                        | "targetConnections"
                ) && !(k == "name" && ty == "Group")
            })
            .cloned()
            .collect(),
        extra_children: Vec::new(),
    };
    for c in &n.children {
        match c.name.as_str() {
            "bounds" => {
                let num = |k: &str, d: i32| c.attr(k).and_then(|v| v.parse().ok()).unwrap_or(d);
                node.bounds = Bounds {
                    x: num("x", 0),
                    y: num("y", 0),
                    width: num("width", -1),
                    height: num("height", -1),
                };
            }
            "child" => {
                let child = read_node(c, m, conns);
                node.children.push(child);
            }
            "sourceConnection" => conns.push(read_connection(c, &node.id, m)),
            "content" if ty == "Note" => {}
            _ => node.extra_children.push(c.clone()),
        }
    }
    node
}

fn read_connection(n: &Node, source_node: &str, m: &mut Model) -> Connection {
    let mut bendpoints = Vec::new();
    let mut extra_children = Vec::new();
    for c in &n.children {
        if c.name == "bendpoint" {
            bendpoints.push(c.clone());
        } else {
            extra_children.push(c.clone());
        }
    }
    Connection {
        id: n
            .attr("id")
            .map(str::to_string)
            .unwrap_or_else(|| m.new_id()),
        source: n.attr("source").unwrap_or(source_node).to_string(),
        target: n.attr("target").unwrap_or_default().to_string(),
        relationship: n.attr("archimateRelationship").map(str::to_string),
        bendpoints,
        extra_attrs: n
            .attrs
            .iter()
            .filter(|(k, v)| {
                !matches!(
                    k.as_str(),
                    "id" | "source" | "target" | "archimateRelationship"
                ) && !(k == "xsi:type" && v == "archimate:Connection")
            })
            .cloned()
            .collect(),
        extra_children,
    }
}

// ---------------------------------------------------------------------------------------------
// Writing

pub fn to_string(m: &Model) -> String {
    let mut root = with_name(
        Node::new("archimate:model")
            .with_attr("xmlns:xsi", NS_XSI)
            .with_attr("xmlns:archimate", NS_ARCHIMATE),
        &m.name,
    )
    .with_attr("id", &m.id)
    .with_attr("version", &m.version);
    root.attrs.extend(m.extra_attrs.iter().cloned());
    for f in &m.folders {
        root.children.push(write_folder(f, m));
    }
    if !m.purpose.is_empty() {
        let mut p = Node::new("purpose");
        p.text = m.purpose.clone();
        root.children.push(p);
    }
    push_props(&mut root, &m.properties);
    root.children.extend(m.extra_children.iter().cloned());
    xml::write_document(&root)
}

fn push_doc(n: &mut Node, doc: &str) {
    if !doc.is_empty() {
        let mut d = Node::new("documentation");
        d.text = doc.to_string();
        n.children.push(d);
    }
}

fn push_props(n: &mut Node, props: &Properties) {
    for (k, v) in props {
        n.children.push(
            Node::new("property")
                .with_attr("key", k)
                .with_attr("value", v),
        );
    }
}

fn with_name(n: Node, name: &str) -> Node {
    if name.is_empty() {
        n
    } else {
        n.with_attr("name", name)
    }
}

fn write_folder(f: &Folder, m: &Model) -> Node {
    let mut n = Node::new("folder")
        .with_attr("name", &f.name)
        .with_attr("id", &f.id);
    if let Some(t) = f.kind.attr() {
        n.set_attr("type", t);
    }
    push_doc(&mut n, &f.documentation);
    push_props(&mut n, &f.properties);
    for sub in &f.folders {
        n.children.push(write_folder(sub, m));
    }
    for item in &f.items {
        let child = match item {
            FolderItem::Element(id) => m.elements.get(id).map(write_element),
            FolderItem::Relationship(id) => m.relationships.get(id).map(write_relationship),
            FolderItem::Diagram(id) => m.diagrams.get(id).map(write_diagram),
            FolderItem::Raw(raw) => Some(raw.clone()),
        };
        n.children.extend(child);
    }
    n
}

fn write_element(e: &Element) -> Node {
    let mut n = with_name(
        Node::new("element").with_attr("xsi:type", format!("archimate:{}", e.kind.name())),
        &e.name,
    )
    .with_attr("id", &e.id);
    n.attrs.extend(e.extra_attrs.iter().cloned());
    push_doc(&mut n, &e.documentation);
    push_props(&mut n, &e.properties);
    n.children.extend(e.extra_children.iter().cloned());
    n
}

fn write_relationship(r: &Relationship) -> Node {
    let mut n = with_name(
        Node::new("element").with_attr("xsi:type", format!("archimate:{}", r.kind.name())),
        &r.name,
    )
    .with_attr("id", &r.id)
    .with_attr("source", &r.source)
    .with_attr("target", &r.target);
    n.attrs.extend(r.extra_attrs.iter().cloned());
    push_doc(&mut n, &r.documentation);
    push_props(&mut n, &r.properties);
    n.children.extend(r.extra_children.iter().cloned());
    n
}

fn write_diagram(d: &Diagram) -> Node {
    let mut n = with_name(
        Node::new("element").with_attr("xsi:type", "archimate:ArchimateDiagramModel"),
        &d.name,
    )
    .with_attr("id", &d.id);
    if let Some(vp) = &d.viewpoint {
        n.set_attr("viewpoint", vp);
    }
    n.attrs.extend(d.extra_attrs.iter().cloned());
    push_doc(&mut n, &d.documentation);

    let mut incoming: HashMap<&str, Vec<&str>> = HashMap::new();
    let mut outgoing: HashMap<&str, Vec<&Connection>> = HashMap::new();
    for c in &d.connections {
        incoming
            .entry(c.target.as_str())
            .or_default()
            .push(c.id.as_str());
        outgoing.entry(c.source.as_str()).or_default().push(c);
    }
    for node in &d.nodes {
        n.children.push(write_node(node, &incoming, &outgoing));
    }
    // Archi writes a view's properties after its contents.
    push_props(&mut n, &d.properties);
    n.children.extend(d.extra_children.iter().cloned());
    n
}

fn write_node(
    dn: &DiagramNode,
    incoming: &HashMap<&str, Vec<&str>>,
    outgoing: &HashMap<&str, Vec<&Connection>>,
) -> Node {
    let ty = match &dn.kind {
        DiagramNodeKind::Element(_) => "DiagramObject",
        DiagramNodeKind::Group { .. } => "Group",
        DiagramNodeKind::Note { .. } => "Note",
        DiagramNodeKind::Reference(_) => "DiagramModelReference",
        DiagramNodeKind::Other(t) => t.as_str(),
    };
    let mut n = Node::new("child").with_attr("xsi:type", format!("archimate:{ty}"));
    if let DiagramNodeKind::Group { name } = &dn.kind {
        n.set_attr("name", name);
    }
    n.set_attr("id", &dn.id);
    if let Some(ins) = incoming.get(dn.id.as_str()) {
        n.set_attr("targetConnections", ins.join(" "));
    }
    // Same attribute order as Archi: styling first, the model reference last.
    n.attrs.extend(dn.extra_attrs.iter().cloned());
    if let Some(c) = &dn.fill_color {
        n.set_attr("fillColor", c);
    }
    match &dn.kind {
        DiagramNodeKind::Element(e) => n.set_attr("archimateElement", e),
        DiagramNodeKind::Reference(r) => n.set_attr("model", r),
        _ => {}
    }

    let b = dn.bounds;
    n.children.push(
        Node::new("bounds")
            .with_attr("x", b.x.to_string())
            .with_attr("y", b.y.to_string())
            .with_attr("width", b.width.to_string())
            .with_attr("height", b.height.to_string()),
    );
    if let DiagramNodeKind::Note { content } = &dn.kind {
        if !content.is_empty() {
            let mut c = Node::new("content");
            c.text = content.clone();
            n.children.push(c);
        }
    }
    for c in outgoing.get(dn.id.as_str()).into_iter().flatten() {
        let mut cn = Node::new("sourceConnection");
        if !c.extra_attrs.iter().any(|(k, _)| k == "xsi:type") {
            cn.set_attr("xsi:type", "archimate:Connection");
        }
        for (k, v) in c.extra_attrs.iter().filter(|(k, _)| k == "xsi:type") {
            cn.set_attr(k, v);
        }
        cn.set_attr("id", &c.id);
        cn.set_attr("source", &c.source);
        cn.set_attr("target", &c.target);
        if let Some(r) = &c.relationship {
            cn.set_attr("archimateRelationship", r);
        }
        cn.attrs.extend(
            c.extra_attrs
                .iter()
                .filter(|(k, _)| k != "xsi:type")
                .cloned(),
        );
        cn.children.extend(c.bendpoints.iter().cloned());
        cn.children.extend(c.extra_children.iter().cloned());
        n.children.push(cn);
    }
    n.children.extend(dn.extra_children.iter().cloned());
    for child in &dn.children {
        n.children.push(write_node(child, incoming, outgoing));
    }
    n
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample() -> Model {
        let mut m = Model::new("Shop");
        let actor = m.add_element(ElementType::BusinessActor, "Customer");
        let svc = m.add_element(ElementType::BusinessService, "Order & Pay");
        let rel = m.add_relationship(RelationshipType::Serving, &svc, &actor);
        let view = m.add_diagram("Overview");
        let a = m.add_to_diagram(&view, &actor, 40, 40).unwrap();
        let s = m.add_to_diagram(&view, &svc, 240, 40).unwrap();
        m.connect_in_diagram(&view, &rel, &s, &a).unwrap();
        m.elements.get_mut(&actor).unwrap().documentation = "Line 1\nLine 2 <b>".into();
        m.elements
            .get_mut(&actor)
            .unwrap()
            .properties
            .push(("Owner".into(), "Sales \"EU\"".into()));
        m
    }

    #[test]
    fn roundtrip_is_lossless() {
        let m = sample();
        let text = to_string(&m);
        let back = from_str(&text).unwrap();
        assert_eq!(back.elements, m.elements);
        assert_eq!(back.relationships, m.relationships);
        assert_eq!(back.diagrams, m.diagrams);
        assert_eq!(to_string(&back), text, "second write must be identical");
    }

    #[test]
    fn unknown_views_are_preserved() {
        let src = r#"<?xml version="1.0" encoding="UTF-8"?>
<archimate:model xmlns:xsi="http://www.w3.org/2001/XMLSchema-instance" xmlns:archimate="http://www.archimatetool.com/archimate" name="m" id="m1" version="5.0.0">
  <folder name="Views" id="f1" type="diagrams">
    <element xsi:type="archimate:SketchModel" name="Sketch" id="s1">
      <child xsi:type="archimate:SketchModelActor" id="a1" name="Bob"><bounds x="1" y="2" width="3" height="4"/></child>
    </element>
  </folder>
</archimate:model>
"#;
        let m = from_str(src).unwrap();
        assert!(to_string(&m)
            .contains(r#"<child xsi:type="archimate:SketchModelActor" id="a1" name="Bob">"#));
    }
}
