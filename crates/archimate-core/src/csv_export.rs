//! CSV export in Archi's layout (`elements.csv`, `relations.csv`, `properties.csv`),
//! so the files can be re-imported into Archi with File › Import › CSV.

use crate::model::Model;
use std::fmt::Write as _;
use std::path::Path;

fn field(out: &mut String, s: &str) {
    out.push('"');
    out.push_str(&s.replace('"', "\"\""));
    out.push('"');
}

fn row(out: &mut String, fields: &[&str]) {
    for (i, f) in fields.iter().enumerate() {
        if i > 0 {
            out.push(',');
        }
        field(out, f);
    }
    out.push_str("\r\n");
}

pub struct CsvFiles {
    pub elements: String,
    pub relations: String,
    pub properties: String,
}

pub fn to_csv(m: &Model) -> CsvFiles {
    let mut elements = String::new();
    row(
        &mut elements,
        &["ID", "Type", "Name", "Documentation", "Specialization"],
    );
    row(
        &mut elements,
        &[&m.id, "ArchimateModel", &m.name, &m.purpose, ""],
    );
    for e in m.elements.values() {
        row(
            &mut elements,
            &[&e.id, e.kind.name(), &e.name, &e.documentation, ""],
        );
    }

    let mut relations = String::new();
    row(
        &mut relations,
        &[
            "ID",
            "Type",
            "Name",
            "Documentation",
            "Source",
            "Target",
            "Specialization",
        ],
    );
    for r in m.relationships.values() {
        row(
            &mut relations,
            &[
                &r.id,
                r.kind.name(),
                &r.name,
                &r.documentation,
                &r.source,
                &r.target,
                "",
            ],
        );
    }

    let mut properties = String::new();
    row(&mut properties, &["ID", "Key", "Value"]);
    let mut props = |id: &str, list: &[(String, String)]| {
        for (k, v) in list {
            row(&mut properties, &[id, k, v]);
        }
    };
    props(&m.id, &m.properties);
    for e in m.elements.values() {
        props(&e.id, &e.properties);
    }
    for r in m.relationships.values() {
        props(&r.id, &r.properties);
    }
    CsvFiles {
        elements,
        relations,
        properties,
    }
}

pub fn export(
    m: &Model,
    dir: impl AsRef<Path>,
    prefix: &str,
) -> std::io::Result<Vec<std::path::PathBuf>> {
    let dir = dir.as_ref();
    std::fs::create_dir_all(dir)?;
    let files = to_csv(m);
    let mut written = Vec::new();
    for (name, body) in [
        ("elements", files.elements),
        ("relations", files.relations),
        ("properties", files.properties),
    ] {
        let mut path_name = String::new();
        let _ = write!(path_name, "{prefix}{name}.csv");
        let p = dir.join(path_name);
        std::fs::write(&p, body)?;
        written.push(p);
    }
    Ok(written)
}
