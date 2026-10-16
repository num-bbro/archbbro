//! Application state, menus, the model tree, the palette and the properties panel.

use crate::canvas::{self, ViewState};
use crate::notation;
use archimate_core::model::{DiagramNodeKind, Folder, FolderItem, FolderKind, Id};
use archimate_core::validate::{self, Issue, Severity};
use archimate_core::{archi_format, csv_export, ElementType, Layer, Model, RelationshipType};
use eframe::egui::{self, Color32, Key, KeyboardShortcut, Modifiers, RichText, Sense};
use std::path::PathBuf;

const UNDO_LIMIT: usize = 100;

#[derive(Debug, Clone, PartialEq)]
pub enum Selection {
    None,
    Element(Id),
    Relationship(Id),
    Diagram(Id),
    Node { diagram: Id, node: Id },
    Connection { diagram: Id, connection: Id },
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Tool {
    Select,
    Create(ElementType),
    Group,
    Note,
    /// `None` is the "magic connector": pick the relationship type after drawing the line.
    Connect(Option<RelationshipType>),
}

pub struct Studio {
    pub model: Model,
    pub path: Option<PathBuf>,
    pub dirty: bool,
    pub selection: Selection,
    pub open_diagram: Option<Id>,
    pub tool: Tool,
    pub view: ViewState,
    pub status: String,
    undo: Vec<Model>,
    redo: Vec<Model>,
    /// Model as it was when a text field gained focus; pushed to undo on the first edit.
    edit_snapshot: Option<Model>,
    issues: Vec<Issue>,
    show_issues: bool,
    new_property_key: String,
}

impl Studio {
    pub fn new(file: Option<PathBuf>) -> Self {
        let mut s = Studio {
            model: archimate_core::example::online_shop(),
            path: None,
            dirty: false,
            selection: Selection::None,
            open_diagram: None,
            tool: Tool::Select,
            view: ViewState::default(),
            status: "Example model loaded. Open an Archi model with File › Open.".into(),
            undo: Vec::new(),
            redo: Vec::new(),
            edit_snapshot: None,
            issues: Vec::new(),
            show_issues: false,
            new_property_key: String::new(),
        };
        if let Some(f) = file {
            s.open_path(f);
        }
        s.open_first_diagram();
        s
    }

    fn open_first_diagram(&mut self) {
        self.open_diagram = first_diagram(&self.model.folders);
        self.view = ViewState::default();
    }

    /// Call before every change to the model.
    pub fn checkpoint(&mut self) {
        self.undo.push(self.model.clone());
        if self.undo.len() > UNDO_LIMIT {
            self.undo.remove(0);
        }
        self.redo.clear();
        self.dirty = true;
    }

    fn undo(&mut self) {
        if let Some(prev) = self.undo.pop() {
            self.redo.push(std::mem::replace(&mut self.model, prev));
            self.after_history_jump();
        }
    }

    fn redo(&mut self) {
        if let Some(next) = self.redo.pop() {
            self.undo.push(std::mem::replace(&mut self.model, next));
            self.after_history_jump();
        }
    }

    fn after_history_jump(&mut self) {
        self.dirty = true;
        self.edit_snapshot = None;
        if self
            .open_diagram
            .as_ref()
            .is_some_and(|d| !self.model.diagrams.contains_key(d))
        {
            self.open_first_diagram();
        }
        if !self.selection_exists() {
            self.selection = Selection::None;
        }
    }

    fn selection_exists(&self) -> bool {
        let m = &self.model;
        match &self.selection {
            Selection::None => true,
            Selection::Element(id) => m.elements.contains_key(id),
            Selection::Relationship(id) => m.relationships.contains_key(id),
            Selection::Diagram(id) => m.diagrams.contains_key(id),
            Selection::Node { diagram, node } => m
                .diagrams
                .get(diagram)
                .is_some_and(|d| d.node(node).is_some()),
            Selection::Connection {
                diagram,
                connection,
            } => m
                .diagrams
                .get(diagram)
                .is_some_and(|d| d.connections.iter().any(|c| &c.id == connection)),
        }
    }

    fn replace_model(&mut self, model: Model, path: Option<PathBuf>) {
        self.model = model;
        self.path = path;
        self.dirty = false;
        self.undo.clear();
        self.redo.clear();
        self.selection = Selection::None;
        self.issues.clear();
        self.open_first_diagram();
    }

    fn open_path(&mut self, path: PathBuf) {
        match archi_format::load(&path) {
            Ok(m) => {
                self.status = format!("Opened {}", path.display());
                self.replace_model(m, Some(path));
            }
            Err(e) => self.status = format!("Could not open {}: {e}", path.display()),
        }
    }

    fn save(&mut self, save_as: bool) {
        let path = match (&self.path, save_as) {
            (Some(p), false) => p.clone(),
            _ => match rfd::FileDialog::new()
                .add_filter("Archi model", &["archimate"])
                .set_file_name(format!("{}.archimate", self.model.name))
                .save_file()
            {
                Some(p) => p,
                None => return,
            },
        };
        match archi_format::save(&self.model, &path) {
            Ok(()) => {
                self.status = format!("Saved {}", path.display());
                self.path = Some(path);
                self.dirty = false;
            }
            Err(e) => self.status = format!("Save failed: {e}"),
        }
    }

    fn run_validation(&mut self) {
        self.issues = validate::validate(&self.model);
        self.show_issues = true;
        self.status = format!("Validation found {} issue(s)", self.issues.len());
    }

    fn delete_selection(&mut self) {
        match self.selection.clone() {
            Selection::Node { diagram, node } => {
                self.checkpoint();
                if let Some(d) = self.model.diagrams.get_mut(&diagram) {
                    d.remove_node(&node);
                }
                self.status = "Removed from view (the element stays in the model)".into();
            }
            Selection::Connection {
                diagram,
                connection,
            } => {
                self.checkpoint();
                if let Some(d) = self.model.diagrams.get_mut(&diagram) {
                    d.connections.retain(|c| c.id != connection);
                }
            }
            Selection::Element(id) => {
                self.checkpoint();
                self.model.delete_element(&id);
            }
            Selection::Relationship(id) => {
                self.checkpoint();
                self.model.delete_relationship(&id);
            }
            Selection::Diagram(id) => {
                self.checkpoint();
                self.model.delete_diagram(&id);
                if self.open_diagram.as_ref() == Some(&id) {
                    self.open_first_diagram();
                }
            }
            Selection::None => return,
        }
        self.selection = Selection::None;
    }

    fn title(&self) -> String {
        let file = self
            .path
            .as_ref()
            .and_then(|p| p.file_name())
            .map(|n| n.to_string_lossy().into_owned());
        format!(
            "{}{} — ArchiMate Studio",
            if self.dirty { "• " } else { "" },
            file.unwrap_or_else(|| self.model.name.clone())
        )
    }

    fn shortcuts(&mut self, ctx: &egui::Context) {
        let typing = ctx.egui_wants_keyboard_input();
        let (save, undo, redo, redo2, delete, escape) = ctx.input_mut(|i| {
            (
                i.consume_shortcut(&KeyboardShortcut::new(Modifiers::COMMAND, Key::S)),
                !typing && i.consume_shortcut(&KeyboardShortcut::new(Modifiers::COMMAND, Key::Z)),
                !typing
                    && i.consume_shortcut(&KeyboardShortcut::new(
                        Modifiers::COMMAND | Modifiers::SHIFT,
                        Key::Z,
                    )),
                !typing && i.consume_shortcut(&KeyboardShortcut::new(Modifiers::COMMAND, Key::Y)),
                !typing && (i.key_pressed(Key::Delete) || i.key_pressed(Key::Backspace)),
                i.key_pressed(Key::Escape),
            )
        });
        if save {
            self.save(false);
        }
        if undo {
            self.undo();
        }
        if redo || redo2 {
            self.redo();
        }
        if delete {
            self.delete_selection();
        }
        if escape {
            self.tool = Tool::Select;
            self.view.cancel();
        }
    }

    // -----------------------------------------------------------------------------------------
    // Panels

    fn menu_bar(&mut self, ui: &mut egui::Ui) {
        egui::MenuBar::new().ui(ui, |ui| {
            ui.menu_button("File", |ui| {
                if ui.button("New model").clicked() {
                    self.replace_model(Model::new("New Model"), None);
                    let v = self.model.add_diagram("Default View");
                    self.open_diagram = Some(v);
                }
                if ui.button("New example model").clicked() {
                    self.replace_model(archimate_core::example::online_shop(), None);
                }
                if ui.button("Open…").clicked() {
                    if let Some(p) = rfd::FileDialog::new()
                        .add_filter("Archi model", &["archimate", "xml"])
                        .pick_file()
                    {
                        self.open_path(p);
                    }
                }
                ui.separator();
                if ui.button("Save   (Ctrl+S)").clicked() {
                    self.save(false);
                }
                if ui.button("Save As…").clicked() {
                    self.save(true);
                }
                if ui.button("Export CSV…").clicked() {
                    if let Some(dir) = rfd::FileDialog::new().pick_folder() {
                        match csv_export::export(&self.model, &dir, "") {
                            Ok(files) => {
                                self.status = format!(
                                    "Exported {} CSV files to {}",
                                    files.len(),
                                    dir.display()
                                )
                            }
                            Err(e) => self.status = format!("Export failed: {e}"),
                        }
                    }
                }
                ui.separator();
                if ui.button("Quit").clicked() {
                    ui.ctx().send_viewport_cmd(egui::ViewportCommand::Close);
                }
            });
            ui.menu_button("Edit", |ui| {
                if ui
                    .add_enabled(!self.undo.is_empty(), egui::Button::new("Undo   (Ctrl+Z)"))
                    .clicked()
                {
                    self.undo();
                }
                if ui
                    .add_enabled(!self.redo.is_empty(), egui::Button::new("Redo   (Ctrl+Y)"))
                    .clicked()
                {
                    self.redo();
                }
                ui.separator();
                if ui
                    .add_enabled(
                        self.selection != Selection::None,
                        egui::Button::new("Delete   (Del)"),
                    )
                    .clicked()
                {
                    self.delete_selection();
                }
            });
            ui.menu_button("View", |ui| {
                if ui.button("New view").clicked() {
                    self.checkpoint();
                    let id = self.model.add_diagram("New View");
                    self.open_diagram = Some(id.clone());
                    self.selection = Selection::Diagram(id);
                }
                if ui.button("Zoom to 100%").clicked() {
                    self.view.zoom = 1.0;
                }
            });
            ui.menu_button("Tools", |ui| {
                if ui.button("Validate model").clicked() {
                    self.run_validation();
                }
            });
        });
    }

    fn model_tree(&mut self, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            ui.heading("Models");
        });
        ui.label(RichText::new(&self.model.name).strong());
        egui::ScrollArea::vertical()
            .auto_shrink([false, false])
            .show(ui, |ui| {
                let folders = self.model.folders.clone();
                for f in &folders {
                    self.folder_ui(ui, f);
                }
            });
    }

    fn folder_ui(&mut self, ui: &mut egui::Ui, folder: &Folder) {
        let count = count_items(folder);
        let header = egui::CollapsingHeader::new(format!("{}  ({count})", folder.name))
            .id_salt(&folder.id)
            .default_open(matches!(folder.kind, FolderKind::Diagrams))
            .show(ui, |ui| {
                for sub in &folder.folders {
                    self.folder_ui(ui, sub);
                }
                for item in &folder.items {
                    self.item_ui(ui, item);
                }
            });
        if folder.kind == FolderKind::Diagrams {
            header.header_response.context_menu(|ui| {
                if ui.button("New view").clicked() {
                    self.checkpoint();
                    let id = self.model.add_diagram("New View");
                    self.open_diagram = Some(id.clone());
                    self.selection = Selection::Diagram(id);
                    ui.close();
                }
            });
        }
    }

    fn item_ui(&mut self, ui: &mut egui::Ui, item: &FolderItem) {
        match item {
            FolderItem::Element(id) => {
                let Some(e) = self.model.elements.get(id) else {
                    return;
                };
                let [r, g, b] = e.kind.default_color();
                let label = if e.name.is_empty() {
                    format!("({})", e.kind.label())
                } else {
                    e.name.clone()
                };
                let selected = self.selection == Selection::Element(id.clone());
                let resp = ui
                    .horizontal(|ui| {
                        let (rect, _) =
                            ui.allocate_exact_size(egui::vec2(10.0, 10.0), Sense::hover());
                        ui.painter().rect(
                            rect,
                            2.0,
                            Color32::from_rgb(r, g, b),
                            egui::Stroke::new(1.0, Color32::GRAY),
                            egui::StrokeKind::Inside,
                        );
                        ui.selectable_label(selected, label)
                            .on_hover_text(e.kind.label())
                    })
                    .inner;
                let resp = resp.interact(Sense::click_and_drag());
                resp.dnd_set_drag_payload(id.clone());
                if resp.clicked() {
                    self.selection = Selection::Element(id.clone());
                }
                let id = id.clone();
                resp.context_menu(|ui| {
                    if ui.button("Delete from model").clicked() {
                        self.selection = Selection::Element(id.clone());
                        self.delete_selection();
                        ui.close();
                    }
                });
            }
            FolderItem::Relationship(id) => {
                let Some(r) = self.model.relationships.get(id) else {
                    return;
                };
                let src = self.model.concept_name(&r.source).unwrap_or("?");
                let tgt = self.model.concept_name(&r.target).unwrap_or("?");
                let label = format!(
                    "{} ({src} → {tgt})",
                    if r.name.is_empty() {
                        r.kind.label()
                    } else {
                        &r.name
                    }
                );
                let selected = self.selection == Selection::Relationship(id.clone());
                if ui.selectable_label(selected, label).clicked() {
                    self.selection = Selection::Relationship(id.clone());
                }
            }
            FolderItem::Diagram(id) => {
                let Some(d) = self.model.diagrams.get(id) else {
                    return;
                };
                let open = self.open_diagram.as_ref() == Some(id);
                let label = if open {
                    RichText::new(format!("▣ {}", d.name)).strong()
                } else {
                    RichText::new(format!("▢ {}", d.name))
                };
                let resp =
                    ui.selectable_label(self.selection == Selection::Diagram(id.clone()), label);
                if resp.clicked() {
                    self.selection = Selection::Diagram(id.clone());
                    if self.open_diagram.as_ref() != Some(id) {
                        self.open_diagram = Some(id.clone());
                        self.view = ViewState::default();
                    }
                }
                let id = id.clone();
                resp.context_menu(|ui| {
                    if ui.button("Delete view").clicked() {
                        self.selection = Selection::Diagram(id.clone());
                        self.delete_selection();
                        ui.close();
                    }
                });
            }
            FolderItem::Raw(n) => {
                let name = n.attr("name").unwrap_or("(unsupported view)");
                ui.label(RichText::new(format!("◌ {name}")).weak())
                    .on_hover_text("Sketch or canvas view: kept in the file, not editable here");
            }
        }
    }

    fn palette(&mut self, ui: &mut egui::Ui) {
        egui::ScrollArea::vertical()
            .auto_shrink([false, false])
            .show(ui, |ui| {
                ui.selectable_value(&mut self.tool, Tool::Select, "↖ Select");
                ui.selectable_value(&mut self.tool, Tool::Connect(None), "✨ Magic connector");
                ui.selectable_value(&mut self.tool, Tool::Group, "▭ Group");
                ui.selectable_value(&mut self.tool, Tool::Note, "🗒 Note");
                egui::CollapsingHeader::new("Relations")
                    .default_open(true)
                    .show(ui, |ui| {
                        for r in RelationshipType::ALL {
                            ui.selectable_value(&mut self.tool, Tool::Connect(Some(r)), r.label());
                        }
                    });
                for layer in Layer::ALL {
                    let [r, g, b] = layer.default_color();
                    egui::CollapsingHeader::new(layer.label())
                        .default_open(matches!(layer, Layer::Business | Layer::Application))
                        .show(ui, |ui| {
                            for t in ElementType::in_layer(layer) {
                                ui.horizontal(|ui| {
                                    let (rect, _) = ui.allocate_exact_size(
                                        egui::vec2(10.0, 10.0),
                                        Sense::hover(),
                                    );
                                    let fill = if t == ElementType::Junction {
                                        Color32::BLACK
                                    } else {
                                        Color32::from_rgb(r, g, b)
                                    };
                                    ui.painter().rect(
                                        rect,
                                        2.0,
                                        fill,
                                        egui::Stroke::new(1.0, Color32::GRAY),
                                        egui::StrokeKind::Inside,
                                    );
                                    ui.selectable_value(&mut self.tool, Tool::Create(t), t.label());
                                });
                            }
                        });
                }
            });
    }

    fn properties(&mut self, ui: &mut egui::Ui) {
        ui.heading("Properties");
        ui.separator();
        let sel = self.selection.clone();
        // A view object stands for its element.
        let target = match &sel {
            Selection::Node { diagram, node } => match self
                .model
                .diagrams
                .get(diagram)
                .and_then(|d| d.node(node))
                .map(|n| n.kind.clone())
            {
                Some(DiagramNodeKind::Element(e)) => Selection::Element(e),
                _ => sel.clone(),
            },
            Selection::Connection {
                diagram,
                connection,
            } => {
                match self
                    .model
                    .diagrams
                    .get(diagram)
                    .and_then(|d| d.connections.iter().find(|c| &c.id == connection))
                    .and_then(|c| c.relationship.clone())
                {
                    Some(r) => Selection::Relationship(r),
                    None => sel.clone(),
                }
            }
            _ => sel.clone(),
        };
        egui::ScrollArea::vertical()
            .auto_shrink([false, false])
            .show(ui, |ui| match target {
                Selection::Element(id) => self.element_props(ui, &id),
                Selection::Relationship(id) => self.relationship_props(ui, &id),
                Selection::Diagram(id) => self.diagram_props(ui, &id),
                Selection::Node { diagram, node } => self.node_props(ui, &diagram, &node),
                Selection::None => self.model_props(ui),
                Selection::Connection { .. } => {
                    ui.label("Plain connection");
                }
            });
        if let Selection::Node { diagram, node } = &sel {
            if let Some(DiagramNodeKind::Element(_)) = self
                .model
                .diagrams
                .get(diagram)
                .and_then(|d| d.node(node))
                .map(|n| n.kind.clone())
            {
                ui.separator();
                self.fill_color_ui(ui, diagram, node);
            }
        }
    }

    /// Wraps an edit widget so the first change after focusing records one undo step.
    fn track_edit(&mut self, resp: &egui::Response) {
        if resp.gained_focus() {
            self.edit_snapshot = Some(self.model.clone());
        }
        if resp.changed() {
            if let Some(snap) = self.edit_snapshot.take() {
                self.undo.push(snap);
                self.redo.clear();
            }
            self.dirty = true;
        }
    }

    fn element_props(&mut self, ui: &mut egui::Ui, id: &str) {
        let Some(e) = self.model.elements.get(id) else {
            return;
        };
        let kind = e.kind;
        ui.label(RichText::new(kind.label()).color(Color32::GRAY));
        let mut name = e.name.clone();
        let mut doc = e.documentation.clone();
        let mut props = e.properties.clone();
        ui.label("Name");
        let r1 = ui.text_edit_singleline(&mut name);
        ui.label("Documentation");
        let r2 = ui.add(
            egui::TextEdit::multiline(&mut doc)
                .desired_rows(5)
                .desired_width(f32::INFINITY),
        );
        self.track_edit(&r1);
        self.track_edit(&r2);
        let props_changed = self.properties_table(ui, &mut props);
        if let Some(e) = self.model.elements.get_mut(id) {
            e.name = name;
            e.documentation = doc;
            e.properties = props;
        }
        if props_changed {
            self.dirty = true;
        }
        ui.separator();
        ui.label(RichText::new("Relations").strong());
        let rels: Vec<_> = self
            .model
            .relationships
            .values()
            .filter(|r| r.source == id || r.target == id)
            .cloned()
            .collect();
        for r in rels {
            let (dir, other) = if r.source == id {
                ("→", &r.target)
            } else {
                ("←", &r.source)
            };
            let other_name = self.model.concept_name(other).unwrap_or("?").to_string();
            if ui.link(format!("{} {dir} {other_name}", r.kind)).clicked() {
                self.selection = Selection::Relationship(r.id.clone());
            }
        }
        ui.separator();
        let usage: Vec<(Id, String)> = self
            .model
            .diagrams
            .values()
            .filter(|d| {
                d.flatten()
                    .iter()
                    .any(|(n, _, _)| n.kind == DiagramNodeKind::Element(id.to_string()))
            })
            .map(|d| (d.id.clone(), d.name.clone()))
            .collect();
        ui.label(RichText::new("Used in views").strong());
        if usage.is_empty() {
            ui.label(RichText::new("Not in any view").italics());
        }
        for (vid, vname) in usage {
            if ui.link(vname).clicked() {
                self.open_diagram = Some(vid);
            }
        }
    }

    fn relationship_props(&mut self, ui: &mut egui::Ui, id: &str) {
        let Some(r) = self.model.relationships.get(id).cloned() else {
            return;
        };
        ui.label(RichText::new(format!("{} relationship", r.kind)).color(Color32::GRAY));
        let src = self
            .model
            .concept_name(&r.source)
            .unwrap_or("?")
            .to_string();
        let tgt = self
            .model
            .concept_name(&r.target)
            .unwrap_or("?")
            .to_string();
        ui.label(format!("{src}  →  {tgt}"));
        let (mut name, mut doc, mut props) = (
            r.name.clone(),
            r.documentation.clone(),
            r.properties.clone(),
        );
        ui.label("Name");
        let r1 = ui.text_edit_singleline(&mut name);
        ui.label("Documentation");
        let r2 = ui.add(
            egui::TextEdit::multiline(&mut doc)
                .desired_rows(4)
                .desired_width(f32::INFINITY),
        );
        self.track_edit(&r1);
        self.track_edit(&r2);
        if r.kind == RelationshipType::Access {
            let current = r
                .extra_attrs
                .iter()
                .find(|(k, _)| k == "accessType")
                .map(|(_, v)| v.clone())
                .unwrap_or_else(|| "0".into());
            let mut access = current.clone();
            egui::ComboBox::from_label("Access")
                .selected_text(access_label(&access))
                .show_ui(ui, |ui| {
                    for v in ["0", "1", "2", "3"] {
                        ui.selectable_value(&mut access, v.to_string(), access_label(v));
                    }
                });
            if access != current {
                self.checkpoint();
                if let Some(r) = self.model.relationships.get_mut(id) {
                    r.extra_attrs.retain(|(k, _)| k != "accessType");
                    if access != "0" {
                        r.extra_attrs.push(("accessType".into(), access));
                    }
                }
            }
        }
        let props_changed = self.properties_table(ui, &mut props);
        if let Some(r) = self.model.relationships.get_mut(id) {
            r.name = name;
            r.documentation = doc;
            r.properties = props;
        }
        if props_changed {
            self.dirty = true;
        }
    }

    fn diagram_props(&mut self, ui: &mut egui::Ui, id: &str) {
        let Some(d) = self.model.diagrams.get(id) else {
            return;
        };
        ui.label(RichText::new("View").color(Color32::GRAY));
        let (mut name, mut doc) = (d.name.clone(), d.documentation.clone());
        ui.label("Name");
        let r1 = ui.text_edit_singleline(&mut name);
        ui.label("Documentation");
        let r2 = ui.add(
            egui::TextEdit::multiline(&mut doc)
                .desired_rows(4)
                .desired_width(f32::INFINITY),
        );
        self.track_edit(&r1);
        self.track_edit(&r2);
        if let Some(d) = self.model.diagrams.get_mut(id) {
            d.name = name;
            d.documentation = doc;
        }
    }

    fn node_props(&mut self, ui: &mut egui::Ui, diagram: &str, node: &str) {
        let Some(n) = self
            .model
            .diagrams
            .get(diagram)
            .and_then(|d| d.node(node))
            .cloned()
        else {
            return;
        };
        match n.kind {
            DiagramNodeKind::Group { name } => {
                ui.label(RichText::new("Group").color(Color32::GRAY));
                let mut name = name;
                let r = ui.text_edit_singleline(&mut name);
                self.track_edit(&r);
                if let Some(DiagramNodeKind::Group { name: n }) = self
                    .model
                    .diagrams
                    .get_mut(diagram)
                    .and_then(|d| d.node_mut(node))
                    .map(|n| &mut n.kind)
                {
                    *n = name;
                }
            }
            DiagramNodeKind::Note { content } => {
                ui.label(RichText::new("Note").color(Color32::GRAY));
                let mut content = content;
                let r = ui.add(
                    egui::TextEdit::multiline(&mut content)
                        .desired_rows(8)
                        .desired_width(f32::INFINITY),
                );
                self.track_edit(&r);
                if let Some(DiagramNodeKind::Note { content: c }) = self
                    .model
                    .diagrams
                    .get_mut(diagram)
                    .and_then(|d| d.node_mut(node))
                    .map(|n| &mut n.kind)
                {
                    *c = content;
                }
            }
            DiagramNodeKind::Reference(_) => {
                ui.label("View reference");
            }
            DiagramNodeKind::Other(t) => {
                ui.label(format!("{t} (kept as-is)"));
            }
            DiagramNodeKind::Element(_) => {}
        }
    }

    fn fill_color_ui(&mut self, ui: &mut egui::Ui, diagram: &str, node: &str) {
        let Some(n) = self
            .model
            .diagrams
            .get(diagram)
            .and_then(|d| d.node(node))
            .cloned()
        else {
            return;
        };
        let DiagramNodeKind::Element(eid) = &n.kind else {
            return;
        };
        let Some(kind) = self.model.elements.get(eid).map(|e| e.kind) else {
            return;
        };
        let mut color = notation::element_fill(kind, n.fill_color.as_deref());
        ui.horizontal(|ui| {
            ui.label("Fill colour");
            if ui.color_edit_button_srgba(&mut color).changed() {
                self.checkpoint();
                if let Some(n) = self
                    .model
                    .diagrams
                    .get_mut(diagram)
                    .and_then(|d| d.node_mut(node))
                {
                    n.fill_color = Some(notation::to_hex(color));
                }
            }
            if n.fill_color.is_some() && ui.button("Default").clicked() {
                self.checkpoint();
                if let Some(n) = self
                    .model
                    .diagrams
                    .get_mut(diagram)
                    .and_then(|d| d.node_mut(node))
                {
                    n.fill_color = None;
                }
            }
        });
    }

    fn model_props(&mut self, ui: &mut egui::Ui) {
        ui.label(RichText::new("Model").color(Color32::GRAY));
        let (mut name, mut purpose) = (self.model.name.clone(), self.model.purpose.clone());
        ui.label("Name");
        let r1 = ui.text_edit_singleline(&mut name);
        ui.label("Purpose");
        let r2 = ui.add(
            egui::TextEdit::multiline(&mut purpose)
                .desired_rows(5)
                .desired_width(f32::INFINITY),
        );
        self.track_edit(&r1);
        self.track_edit(&r2);
        self.model.name = name;
        self.model.purpose = purpose;
        ui.separator();
        ui.label(format!(
            "{} elements · {} relationships · {} views",
            self.model.elements.len(),
            self.model.relationships.len(),
            self.model.diagrams.len()
        ));
    }

    /// Returns true when the list changed (rows added, edited or removed).
    fn properties_table(&mut self, ui: &mut egui::Ui, props: &mut Vec<(String, String)>) -> bool {
        ui.separator();
        ui.label(RichText::new("User properties").strong());
        let mut changed = false;
        let mut remove = None;
        egui::Grid::new("props")
            .num_columns(3)
            .striped(true)
            .show(ui, |ui| {
                for (i, (k, v)) in props.iter_mut().enumerate() {
                    let a = ui.add(egui::TextEdit::singleline(k).desired_width(90.0));
                    let b = ui.add(egui::TextEdit::singleline(v).desired_width(110.0));
                    changed |= a.changed() || b.changed();
                    if ui.small_button("✕").clicked() {
                        remove = Some(i);
                    }
                    ui.end_row();
                }
            });
        if let Some(i) = remove {
            self.checkpoint();
            props.remove(i);
            changed = true;
        }
        ui.horizontal(|ui| {
            ui.add(
                egui::TextEdit::singleline(&mut self.new_property_key)
                    .hint_text("New property")
                    .desired_width(140.0),
            );
            if ui.button("Add").clicked() && !self.new_property_key.trim().is_empty() {
                self.checkpoint();
                props.push((std::mem::take(&mut self.new_property_key), String::new()));
                changed = true;
            }
        });
        changed
    }

    fn issues_window(&mut self, ctx: &egui::Context) {
        let mut open = self.show_issues;
        let mut jump = None;
        egui::Window::new("Validation")
            .open(&mut open)
            .default_size([520.0, 320.0])
            .show(ctx, |ui| {
                if self.issues.is_empty() {
                    ui.label("No issues found.");
                }
                egui::ScrollArea::vertical().show(ui, |ui| {
                    for issue in &self.issues {
                        let color = match issue.severity {
                            Severity::Error => Color32::from_rgb(200, 40, 40),
                            Severity::Warning => Color32::from_rgb(200, 130, 0),
                            Severity::Advice => Color32::GRAY,
                        };
                        ui.horizontal(|ui| {
                            ui.label(
                                RichText::new(format!("{:?}", issue.severity))
                                    .color(color)
                                    .strong(),
                            );
                            if ui.link(&issue.message).clicked() {
                                jump = Some(issue.object.clone());
                            }
                        });
                    }
                });
                if ui.button("Re-run").clicked() {
                    self.issues = validate::validate(&self.model);
                }
            });
        self.show_issues = open;
        if let Some(id) = jump {
            self.selection = if self.model.elements.contains_key(&id) {
                Selection::Element(id)
            } else if self.model.relationships.contains_key(&id) {
                Selection::Relationship(id)
            } else if self.model.diagrams.contains_key(&id) {
                self.open_diagram = Some(id.clone());
                Selection::Diagram(id)
            } else {
                Selection::None
            };
        }
    }
}

impl eframe::App for Studio {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        let ctx = ui.ctx().clone();
        ctx.send_viewport_cmd(egui::ViewportCommand::Title(self.title()));
        self.shortcuts(&ctx);

        egui::Panel::top("menu").show(ui, |ui| self.menu_bar(ui));
        egui::Panel::bottom("status").show(ui, |ui| {
            ui.horizontal(|ui| {
                ui.label(&self.status);
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    ui.label(format!("{:.0}%", self.view.zoom * 100.0));
                    ui.label(tool_hint(self.tool));
                });
            });
        });
        egui::Panel::left("tree")
            .resizable(true)
            .default_size(260.0)
            .show(ui, |ui| self.model_tree(ui));
        egui::Panel::right("props")
            .resizable(true)
            .default_size(300.0)
            .show(ui, |ui| self.properties(ui));
        egui::Panel::left("palette")
            .resizable(true)
            .default_size(190.0)
            .show(ui, |ui| self.palette(ui));
        egui::CentralPanel::default().show(ui, |ui| canvas::show(self, ui));
        self.issues_window(&ctx);
    }
}

fn tool_hint(tool: Tool) -> String {
    match tool {
        Tool::Select => "Drag to move · drag empty space to pan · Ctrl+scroll to zoom".into(),
        Tool::Create(t) => format!("Click in the view to add a {}", t.label()),
        Tool::Group => "Click in the view to add a group".into(),
        Tool::Note => "Click in the view to add a note".into(),
        Tool::Connect(Some(r)) => format!("Drag from one element to another to add {r}"),
        Tool::Connect(None) => "Drag between two elements, then pick a valid relationship".into(),
    }
}

fn access_label(v: &str) -> &'static str {
    match v {
        "1" => "Read",
        "2" => "Access",
        "3" => "Read/Write",
        _ => "Write",
    }
}

fn count_items(f: &Folder) -> usize {
    f.items.len() + f.folders.iter().map(count_items).sum::<usize>()
}

fn first_diagram(folders: &[Folder]) -> Option<Id> {
    for f in folders {
        for item in &f.items {
            if let FolderItem::Diagram(id) = item {
                return Some(id.clone());
            }
        }
        if let Some(d) = first_diagram(&f.folders) {
            return Some(d);
        }
    }
    None
}
