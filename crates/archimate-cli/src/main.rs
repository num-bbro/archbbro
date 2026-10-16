use anyhow::{bail, Context, Result};
use archimate_core::{
    archi_format, csv_export, metamodel::Layer, model::FolderItem, validate, ElementType, Model,
    RelationshipType,
};
use clap::{Parser, Subcommand};
use std::path::PathBuf;

#[derive(Parser)]
#[command(
    name = "archimate",
    version,
    about = "Work with ArchiMate models stored in Archi's .archimate format"
)]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Summarise a model: counts per layer, relationships and views
    Info { file: PathBuf },
    /// Check a model for invalid relationships, unused elements, empty views and duplicates
    Validate {
        file: PathBuf,
        /// Exit with status 1 if any error is found (for CI)
        #[arg(long)]
        strict: bool,
    },
    /// Export elements, relations and properties as Archi-compatible CSV files
    ExportCsv {
        file: PathBuf,
        out_dir: PathBuf,
        #[arg(long, default_value = "")]
        prefix: String,
    },
    /// Re-save a model in canonical form (useful for clean diffs in git)
    Format {
        file: PathBuf,
        #[arg(short, long)]
        output: Option<PathBuf>,
    },
    /// Create a new empty model
    New {
        file: PathBuf,
        #[arg(long, default_value = "New Model")]
        name: String,
    },
    /// Create a small example model with a view
    Example { file: PathBuf },
    /// List the relationships ArchiMate allows between two element types (e.g. BusinessActor BusinessRole)
    Rules { source: String, target: String },
}

fn main() -> Result<()> {
    match Cli::parse().command {
        Command::Info { file } => info(&load(&file)?),
        Command::Validate { file, strict } => {
            let model = load(&file)?;
            let issues = validate::validate(&model);
            for i in &issues {
                println!("{i}");
            }
            let errors = issues
                .iter()
                .filter(|i| i.severity == validate::Severity::Error)
                .count();
            println!("{} issue(s), {errors} error(s)", issues.len());
            if strict && errors > 0 {
                std::process::exit(1);
            }
            Ok(())
        }
        Command::ExportCsv {
            file,
            out_dir,
            prefix,
        } => {
            for p in csv_export::export(&load(&file)?, &out_dir, &prefix)? {
                println!("wrote {}", p.display());
            }
            Ok(())
        }
        Command::Format { file, output } => {
            let model = load(&file)?;
            save(&model, output.as_ref().unwrap_or(&file))
        }
        Command::New { file, name } => save(&Model::new(name), &file),
        Command::Example { file } => save(&archimate_core::example::online_shop(), &file),
        Command::Rules { source, target } => {
            let parse = |s: &str| {
                ElementType::from_name(s).with_context(|| format!("unknown element type '{s}'"))
            };
            let (s, t) = (parse(&source)?, parse(&target)?);
            let allowed = archimate_core::rules::Rules::get().allowed_between(s, t);
            if allowed.is_empty() {
                bail!("no relationships allowed from {s} to {t}");
            }
            for r in allowed {
                println!("{r}");
            }
            Ok(())
        }
    }
}

fn load(file: &PathBuf) -> Result<Model> {
    archi_format::load(file).with_context(|| format!("reading {}", file.display()))
}

fn save(model: &Model, file: &PathBuf) -> Result<()> {
    archi_format::save(model, file).with_context(|| format!("writing {}", file.display()))?;
    println!("wrote {}", file.display());
    Ok(())
}

fn info(m: &Model) -> Result<()> {
    println!("Model: {} ({})", m.name, m.id);
    println!("Format version: {}", m.version);
    println!("Elements: {}", m.elements.len());
    for layer in Layer::ALL {
        let n = m
            .elements
            .values()
            .filter(|e| e.kind.layer() == layer)
            .count();
        if n > 0 {
            println!("  {:<28}{n}", layer.label());
        }
    }
    println!("Relationships: {}", m.relationships.len());
    for kind in RelationshipType::ALL {
        let n = m.relationships.values().filter(|r| r.kind == kind).count();
        if n > 0 {
            println!("  {:<28}{n}", kind.label());
        }
    }
    println!("Views: {}", m.diagrams.len());
    for d in m.diagrams.values() {
        println!(
            "  {} ({} objects, {} connections)",
            d.name,
            d.flatten().len(),
            d.connections.len()
        );
    }
    let raw = count_raw(&m.folders);
    if raw > 0 {
        println!("Other views kept as-is (sketch/canvas): {raw}");
    }
    Ok(())
}

fn count_raw(folders: &[archimate_core::model::Folder]) -> usize {
    folders
        .iter()
        .map(|f| {
            f.items
                .iter()
                .filter(|i| matches!(i, FolderItem::Raw(_)))
                .count()
                + count_raw(&f.folders)
        })
        .sum()
}
