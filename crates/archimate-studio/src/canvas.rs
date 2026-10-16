//! The diagram canvas: drawing a view and editing it with the mouse.

use crate::app::{Selection, Studio, Tool};
use crate::notation;
use archimate_core::model::{Bounds, Diagram, DiagramNode, DiagramNodeKind, Id};
use archimate_core::rules::Rules;
use archimate_core::{ElementType, Model, RelationshipType};
use eframe::egui::{
    self, pos2, vec2, Color32, CornerRadius, Pos2, Rect, Sense, Stroke, StrokeKind, Vec2,
};
use std::collections::HashMap;

pub struct ViewState {
    pub zoom: f32,
    pub pan: Vec2,
    drag: Option<Drag>,
    /// A line drawn with the magic connector, waiting for the user to pick a type.
    pending: Option<PendingConnection>,
}

impl Default for ViewState {
    fn default() -> Self {
        ViewState {
            zoom: 1.0,
            pan: vec2(20.0, 20.0),
            drag: None,
            pending: None,
        }
    }
}

impl ViewState {
    pub fn cancel(&mut self) {
        self.drag = None;
        self.pending = None;
    }
}

enum Drag {
    Move {
        node: Id,
        start: Bounds,
        accumulated: Vec2,
        moved: bool,
    },
    Resize {
        node: Id,
        start: Bounds,
        accumulated: Vec2,
    },
    Connect {
        from: Id,
    },
    Pan,
}

struct PendingConnection {
    from: Id,
    to: Id,
    at: Pos2,
    allowed: Vec<RelationshipType>,
}

struct Placed {
    id: Id,
    rect: Rect,
    kind: DiagramNodeKind,
}

fn to_screen(origin: Pos2, view: &ViewState, x: f32, y: f32) -> Pos2 {
    origin + view.pan + vec2(x, y) * view.zoom
}

fn to_model(origin: Pos2, view: &ViewState, p: Pos2) -> Vec2 {
    (p - origin - view.pan) / view.zoom
}

fn layout(d: &Diagram, origin: Pos2, view: &ViewState) -> Vec<Placed> {
    d.flatten()
        .into_iter()
        .map(|(n, x, y)| {
            let (w, h) = n.bounds.effective_size();
            let min = to_screen(origin, view, x as f32, y as f32);
            Placed {
                id: n.id.clone(),
                rect: Rect::from_min_size(min, vec2(w as f32, h as f32) * view.zoom),
                kind: n.kind.clone(),
            }
        })
        .collect()
}

/// Topmost node under the pointer (children are drawn after their parents).
fn hit<'a>(placed: &'a [Placed], p: Pos2, exclude: Option<&str>) -> Option<&'a Placed> {
    placed
        .iter()
        .rev()
        .find(|n| n.rect.contains(p) && Some(n.id.as_str()) != exclude)
}

fn element_kind(m: &Model, node: &DiagramNodeKind) -> Option<ElementType> {
    match node {
        DiagramNodeKind::Element(e) => m.elements.get(e).map(|e| e.kind),
        _ => None,
    }
}

pub fn show(app: &mut Studio, ui: &mut egui::Ui) {
    // Toolbar
    ui.horizontal(|ui| {
        let names: Vec<(Id, String)> = app
            .model
            .diagrams
            .values()
            .map(|d| (d.id.clone(), d.name.clone()))
            .collect();
        let current = app
            .open_diagram
            .as_ref()
            .and_then(|id| app.model.diagrams.get(id))
            .map(|d| d.name.clone())
            .unwrap_or_else(|| "No view".into());
        egui::ComboBox::from_id_salt("open-view")
            .selected_text(current)
            .width(220.0)
            .show_ui(ui, |ui| {
                for (id, name) in names {
                    if ui
                        .selectable_label(app.open_diagram.as_ref() == Some(&id), name)
                        .clicked()
                    {
                        app.open_diagram = Some(id);
                        app.view = ViewState::default();
                    }
                }
            });
        if ui.button("−").clicked() {
            app.view.zoom = (app.view.zoom / 1.2).max(0.2);
        }
        if ui.button("100%").clicked() {
            app.view.zoom = 1.0;
        }
        if ui.button("+").clicked() {
            app.view.zoom = (app.view.zoom * 1.2).min(4.0);
        }
        if ui.button("Fit").clicked() {
            app.view.pending = None;
            fit(app, ui.available_rect_before_wrap());
        }
    });

    let Some(diagram_id) = app
        .open_diagram
        .clone()
        .filter(|id| app.model.diagrams.contains_key(id))
    else {
        ui.centered_and_justified(|ui| ui.label("No view open. Create one with View › New view."));
        return;
    };

    let (response, painter) = ui.allocate_painter(ui.available_size(), Sense::click_and_drag());
    let canvas = response.rect;
    let origin = canvas.min;
    painter.rect_filled(canvas, CornerRadius::ZERO, Color32::WHITE);
    draw_grid(&painter, canvas, origin, &app.view);

    // Zoom around the pointer, scroll to pan.
    if response.hovered() {
        let (zoom_delta, scroll, pointer) =
            ui.input(|i| (i.zoom_delta(), i.smooth_scroll_delta, i.pointer.hover_pos()));
        if (zoom_delta - 1.0).abs() > 1e-3 {
            if let Some(p) = pointer {
                let before = to_model(origin, &app.view, p);
                app.view.zoom = (app.view.zoom * zoom_delta).clamp(0.2, 4.0);
                let after = to_model(origin, &app.view, p);
                app.view.pan += (after - before) * app.view.zoom;
            }
        } else {
            app.view.pan += scroll;
        }
    }

    let placed = layout(&app.model.diagrams[&diagram_id], origin, &app.view);
    let pointer = response
        .interact_pointer_pos()
        .or_else(|| ui.input(|i| i.pointer.hover_pos()));

    handle_input(app, ui, &response, &diagram_id, &placed, origin, pointer);

    // Draw after input so this frame shows the result.
    let placed = layout(&app.model.diagrams[&diagram_id], origin, &app.view);
    let painter = painter.with_clip_rect(canvas);
    draw_diagram(app, &painter, &diagram_id, &placed);

    if let (Some(Drag::Connect { from }), Some(p)) = (&app.view.drag, pointer) {
        if let Some(src) = placed.iter().find(|n| &n.id == from) {
            let valid = hit(&placed, p, Some(from))
                .map(|t| connection_allowed(app, &src.kind, &t.kind))
                .unwrap_or(true);
            let color = if valid {
                Color32::from_rgb(30, 120, 255)
            } else {
                Color32::from_rgb(220, 50, 50)
            };
            painter.line_segment([src.rect.center(), p], Stroke::new(1.5, color));
        }
    }

    // Accept elements dragged in from the model tree.
    if let Some(payload) = response.dnd_release_payload::<Id>() {
        if let Some(p) = pointer {
            let at = to_model(origin, &app.view, p);
            let element = (*payload).clone();
            if app.model.elements.contains_key(&element) {
                app.checkpoint();
                let parent = hit(&placed, p, None).map(|n| n.id.clone());
                let node = add_node(
                    &mut app.model,
                    &diagram_id,
                    parent.as_deref(),
                    DiagramNodeKind::Element(element.clone()),
                    at,
                );
                add_existing_connections(&mut app.model, &diagram_id, &element, &node);
                app.selection = Selection::Node {
                    diagram: diagram_id.clone(),
                    node,
                };
            }
        }
    }

    magic_menu(app, ui, &diagram_id, &placed);
}

fn connection_allowed(app: &Studio, from: &DiagramNodeKind, to: &DiagramNodeKind) -> bool {
    let (Some(s), Some(t)) = (element_kind(&app.model, from), element_kind(&app.model, to)) else {
        return false;
    };
    let allowed = Rules::get().allowed_between(s, t);
    match app.tool {
        Tool::Connect(Some(r)) => allowed.contains(&r),
        _ => !allowed.is_empty(),
    }
}

fn handle_input(
    app: &mut Studio,
    ui: &egui::Ui,
    response: &egui::Response,
    diagram: &Id,
    placed: &[Placed],
    origin: Pos2,
    pointer: Option<Pos2>,
) {
    let Some(p) = pointer else { return };
    let zoom = app.view.zoom;

    if response.drag_started() || (response.clicked() && app.view.drag.is_none()) {
        // A drag is recognised only after the pointer has moved; hit-test where it was pressed.
        let p = ui.input(|i| i.pointer.press_origin()).unwrap_or(p);
        let target = hit(placed, p, None);
        match app.tool {
            Tool::Create(_) | Tool::Group | Tool::Note
                if response.clicked() || response.drag_started() =>
            {
                let kind = match app.tool {
                    Tool::Create(t) => {
                        app.checkpoint();
                        let id = app.model.add_element(t, t.label());
                        DiagramNodeKind::Element(id)
                    }
                    Tool::Group => {
                        app.checkpoint();
                        DiagramNodeKind::Group {
                            name: "Group".into(),
                        }
                    }
                    _ => {
                        app.checkpoint();
                        DiagramNodeKind::Note {
                            content: String::new(),
                        }
                    }
                };
                let at = to_model(origin, &app.view, p);
                let parent = target.map(|n| n.id.clone());
                let node = add_node(&mut app.model, diagram, parent.as_deref(), kind, at);
                if matches!(app.tool, Tool::Group) {
                    if let Some(n) = app
                        .model
                        .diagrams
                        .get_mut(diagram)
                        .and_then(|d| d.node_mut(&node))
                    {
                        n.bounds.width = 300;
                        n.bounds.height = 200;
                    }
                }
                app.selection = Selection::Node {
                    diagram: diagram.clone(),
                    node,
                };
                app.tool = Tool::Select;
                app.view.drag = None;
                return;
            }
            Tool::Connect(_) => {
                if let Some(t) = target {
                    if response.drag_started() {
                        app.view.drag = Some(Drag::Connect { from: t.id.clone() });
                    }
                }
            }
            _ => {
                // Resize handle at the bottom-right corner of the selected node.
                let selected_node = match &app.selection {
                    Selection::Node { node, .. } => Some(node.clone()),
                    _ => None,
                };
                let on_handle = selected_node
                    .as_ref()
                    .and_then(|sel| placed.iter().find(|n| &n.id == sel))
                    .filter(|n| handle_rect(n.rect).contains(p));
                if let Some(n) = on_handle {
                    if response.drag_started() {
                        let start = app.model.diagrams[diagram]
                            .node(&n.id)
                            .map(|n| n.bounds)
                            .unwrap_or_default();
                        app.view.drag = Some(Drag::Resize {
                            node: n.id.clone(),
                            start,
                            accumulated: Vec2::ZERO,
                        });
                    }
                } else if let Some(t) = target {
                    app.selection = Selection::Node {
                        diagram: diagram.clone(),
                        node: t.id.clone(),
                    };
                    if response.drag_started() {
                        let start = app.model.diagrams[diagram]
                            .node(&t.id)
                            .map(|n| n.bounds)
                            .unwrap_or_default();
                        app.view.drag = Some(Drag::Move {
                            node: t.id.clone(),
                            start,
                            accumulated: Vec2::ZERO,
                            moved: false,
                        });
                    }
                } else if let Some(c) = hit_connection(app, diagram, placed, p) {
                    app.selection = Selection::Connection {
                        diagram: diagram.clone(),
                        connection: c,
                    };
                } else {
                    app.selection = Selection::Diagram(diagram.clone());
                    if response.drag_started() {
                        app.view.drag = Some(Drag::Pan);
                    }
                }
            }
        }
    }

    if response.dragged() {
        let delta = response.drag_delta();
        // The first movement of a drag records one undo step for the whole move/resize.
        let first_move = match &mut app.view.drag {
            Some(Drag::Move { moved, .. }) if !*moved && delta != Vec2::ZERO => {
                *moved = true;
                true
            }
            Some(Drag::Resize { accumulated, .. }) => {
                *accumulated == Vec2::ZERO && delta != Vec2::ZERO
            }
            _ => false,
        };
        if first_move {
            app.checkpoint();
        }
        match &mut app.view.drag {
            Some(Drag::Pan) => app.view.pan += delta,
            Some(Drag::Move {
                node,
                start,
                accumulated,
                ..
            }) => {
                *accumulated += delta / zoom;
                let (node, start, acc) = (node.clone(), *start, *accumulated);
                if let Some(n) = app
                    .model
                    .diagrams
                    .get_mut(diagram)
                    .and_then(|d| d.node_mut(&node))
                {
                    n.bounds.x = start.x + acc.x.round() as i32;
                    n.bounds.y = start.y + acc.y.round() as i32;
                }
            }
            Some(Drag::Resize {
                node,
                start,
                accumulated,
            }) => {
                *accumulated += delta / zoom;
                let (node, start, acc) = (node.clone(), *start, *accumulated);
                let (w, h) = start.effective_size();
                if let Some(n) = app
                    .model
                    .diagrams
                    .get_mut(diagram)
                    .and_then(|d| d.node_mut(&node))
                {
                    n.bounds.width = (w + acc.x.round() as i32).max(20);
                    n.bounds.height = (h + acc.y.round() as i32).max(15);
                }
            }
            _ => {}
        }
    }

    if response.drag_stopped() {
        match app.view.drag.take() {
            Some(Drag::Connect { from }) => {
                if let Some(t) = hit(placed, p, Some(&from)) {
                    let src = placed.iter().find(|n| n.id == from).map(|n| n.kind.clone());
                    if let Some(src) = src {
                        finish_connection(app, diagram, &from, &src, t, p);
                    }
                }
            }
            Some(Drag::Move {
                node, moved: true, ..
            }) => {
                // Drop into (or out of) a container, like Archi's nesting.
                let new_parent = hit(placed, p, Some(&node))
                    .filter(|t| !is_descendant(&app.model.diagrams[diagram], &node, &t.id))
                    .map(|t| t.id.clone());
                reparent(&mut app.model, diagram, &node, new_parent.as_deref());
            }
            _ => {}
        }
    }

    // Double-click an element to rename it in the properties panel; on empty space, fit.
    if response.double_clicked() && hit(placed, p, None).is_none() {
        fit(app, response.rect);
    }
}

fn finish_connection(
    app: &mut Studio,
    diagram: &Id,
    from: &Id,
    src: &DiagramNodeKind,
    to: &Placed,
    at: Pos2,
) {
    let (Some(s), Some(t)) = (
        element_kind(&app.model, src),
        element_kind(&app.model, &to.kind),
    ) else {
        app.status = "Relationships connect ArchiMate elements".into();
        return;
    };
    let allowed = Rules::get().allowed_between(s, t).to_vec();
    match app.tool {
        Tool::Connect(Some(r)) if allowed.contains(&r) => {
            create_connection(app, diagram, from, &to.id, r);
        }
        Tool::Connect(Some(r)) => {
            let options: Vec<_> = allowed.iter().map(|r| r.label()).collect();
            app.status = format!(
                "{r} is not allowed from {s} to {t}. Allowed: {}",
                options.join(", ")
            );
        }
        _ if allowed.is_empty() => {
            app.status = format!("No relationship is allowed from {s} to {t}")
        }
        _ => {
            app.view.pending = Some(PendingConnection {
                from: from.clone(),
                to: to.id.clone(),
                at,
                allowed,
            })
        }
    }
}

fn create_connection(app: &mut Studio, diagram: &Id, from: &Id, to: &Id, kind: RelationshipType) {
    let d = &app.model.diagrams[diagram];
    let (Some(DiagramNodeKind::Element(se)), Some(DiagramNodeKind::Element(te))) = (
        d.node(from).map(|n| n.kind.clone()),
        d.node(to).map(|n| n.kind.clone()),
    ) else {
        return;
    };
    app.checkpoint();
    let rel = app.model.add_relationship(kind, &se, &te);
    if let Some(conn) = app.model.connect_in_diagram(diagram, &rel, from, to) {
        app.selection = Selection::Connection {
            diagram: diagram.clone(),
            connection: conn,
        };
    }
    app.status = format!("Added {kind} relationship");
}

fn magic_menu(app: &mut Studio, ui: &egui::Ui, diagram: &Id, _placed: &[Placed]) {
    let Some(pending) = &app.view.pending else {
        return;
    };
    let (at, allowed) = (pending.at, pending.allowed.clone());
    let mut chosen = None;
    let mut close = false;
    egui::Area::new(egui::Id::unique("magic-connector"))
        .fixed_pos(at)
        .order(egui::Order::Foreground)
        .show(ui.ctx(), |ui| {
            egui::Frame::popup(ui.style()).show(ui, |ui| {
                ui.label(egui::RichText::new("Relationship").strong());
                for r in allowed {
                    if ui.button(r.label()).clicked() {
                        chosen = Some(r);
                    }
                }
                if ui.button("Cancel").clicked() {
                    close = true;
                }
            });
        });
    if let Some(r) = chosen {
        if let Some(p) = app.view.pending.take() {
            create_connection(app, diagram, &p.from, &p.to, r);
        }
    } else if close {
        app.view.pending = None;
    }
}

fn handle_rect(r: Rect) -> Rect {
    Rect::from_center_size(r.right_bottom(), vec2(8.0, 8.0))
}

fn hit_connection(app: &Studio, diagram: &Id, placed: &[Placed], p: Pos2) -> Option<Id> {
    let rects: HashMap<&str, Rect> = placed.iter().map(|n| (n.id.as_str(), n.rect)).collect();
    app.model.diagrams[diagram]
        .connections
        .iter()
        .rev()
        .find_map(|c| {
            let (a, b) = (rects.get(c.source.as_str())?, rects.get(c.target.as_str())?);
            let (from, to) = (
                notation::clip_to_rect(*a, b.center()),
                notation::clip_to_rect(*b, a.center()),
            );
            (notation::distance_to_segment(p, from, to) < 5.0).then(|| c.id.clone())
        })
}

/// Add a node at a model position, nested in `parent` when given.
fn add_node(
    m: &mut Model,
    diagram: &str,
    parent: Option<&str>,
    kind: DiagramNodeKind,
    at: Vec2,
) -> Id {
    let id = m.new_id();
    let d = m.diagrams.get_mut(diagram).expect("open view exists");
    let parent_origin = parent.and_then(|p| {
        d.flatten()
            .into_iter()
            .find(|(n, _, _)| n.id == p)
            .map(|(_, x, y)| (x, y))
    });
    let (ox, oy) = parent_origin.unwrap_or((0, 0));
    let node = DiagramNode {
        id: id.clone(),
        kind,
        bounds: Bounds {
            x: at.x.round() as i32 - ox - 60,
            y: at.y.round() as i32 - oy - 27,
            width: Bounds::DEFAULT_WIDTH,
            height: Bounds::DEFAULT_HEIGHT,
        },
        fill_color: None,
        children: Vec::new(),
        extra_attrs: Vec::new(),
        extra_children: Vec::new(),
    };
    match parent.and_then(|p| d.node_mut(p)) {
        Some(p) => p.children.push(node),
        None => d.nodes.push(node),
    }
    id
}

/// When an element is added to a view, also draw its relationships to elements already there.
fn add_existing_connections(m: &mut Model, diagram: &str, element: &str, node: &str) {
    let on_view: Vec<(Id, Id)> = m.diagrams[diagram]
        .flatten()
        .iter()
        .filter_map(|(n, _, _)| match &n.kind {
            DiagramNodeKind::Element(e) if n.id != node => Some((e.clone(), n.id.clone())),
            _ => None,
        })
        .collect();
    let rels: Vec<(Id, Id, Id)> = m
        .relationships
        .values()
        .map(|r| (r.id.clone(), r.source.clone(), r.target.clone()))
        .collect();
    for (rid, s, t) in rels {
        for (e, n) in &on_view {
            if s == element && &t == e {
                m.connect_in_diagram(diagram, &rid, node, n);
            } else if t == element && &s == e {
                m.connect_in_diagram(diagram, &rid, n, node);
            }
        }
    }
}

fn is_descendant(d: &Diagram, ancestor: &str, candidate: &str) -> bool {
    fn contains(n: &DiagramNode, id: &str) -> bool {
        n.children.iter().any(|c| c.id == id || contains(c, id))
    }
    d.node(ancestor).is_some_and(|a| contains(a, candidate))
}

fn parent_of(d: &Diagram, id: &str) -> Option<Id> {
    d.flatten()
        .into_iter()
        .find(|(n, _, _)| n.children.iter().any(|c| c.id == id))
        .map(|(n, _, _)| n.id.clone())
}

/// Move a node under a new parent (or to the top level), keeping its position on screen.
fn reparent(m: &mut Model, diagram: &str, node: &str, new_parent: Option<&str>) {
    let d = m.diagrams.get_mut(diagram).expect("open view exists");
    if parent_of(d, node).as_deref() == new_parent {
        return;
    }
    let abs = |d: &Diagram, id: &str| {
        d.flatten()
            .into_iter()
            .find(|(n, _, _)| n.id == id)
            .map(|(_, x, y)| (x, y))
    };
    let Some((ax, ay)) = abs(d, node) else { return };
    let (px, py) = new_parent.and_then(|p| abs(d, p)).unwrap_or((0, 0));
    // Detach without dropping connections.
    fn take(nodes: &mut Vec<DiagramNode>, id: &str) -> Option<DiagramNode> {
        if let Some(i) = nodes.iter().position(|n| n.id == id) {
            return Some(nodes.remove(i));
        }
        nodes.iter_mut().find_map(|n| take(&mut n.children, id))
    }
    let Some(mut n) = take(&mut d.nodes, node) else {
        return;
    };
    n.bounds.x = ax - px;
    n.bounds.y = ay - py;
    match new_parent.and_then(|p| d.node_mut(p)) {
        Some(p) => p.children.push(n),
        None => d.nodes.push(n),
    }
}

fn fit(app: &mut Studio, area: Rect) {
    let Some(d) = app
        .open_diagram
        .as_ref()
        .and_then(|id| app.model.diagrams.get(id))
    else {
        return;
    };
    let mut bounds: Option<Rect> = None;
    for (n, x, y) in d.flatten() {
        let (w, h) = n.bounds.effective_size();
        let r = Rect::from_min_size(pos2(x as f32, y as f32), vec2(w as f32, h as f32));
        bounds = Some(bounds.map_or(r, |b| b.union(r)));
    }
    let Some(b) = bounds else { return };
    let margin = 30.0;
    let zoom = ((area.width() - 2.0 * margin) / b.width())
        .min((area.height() - 2.0 * margin) / b.height())
        .clamp(0.2, 2.0);
    app.view.zoom = zoom;
    app.view.pan = vec2(margin, margin) - b.min.to_vec2() * zoom;
}

fn draw_grid(p: &egui::Painter, canvas: Rect, origin: Pos2, view: &ViewState) {
    let step = 24.0 * view.zoom;
    if step < 8.0 {
        return;
    }
    let start = origin + view.pan;
    let color = Color32::from_gray(235);
    let mut x = start.x + ((canvas.left() - start.x) / step).ceil() * step;
    while x < canvas.right() {
        let mut y = start.y + ((canvas.top() - start.y) / step).ceil() * step;
        while y < canvas.bottom() {
            p.circle_filled(pos2(x, y), 1.0, color);
            y += step;
        }
        x += step;
    }
}

fn draw_diagram(app: &Studio, p: &egui::Painter, diagram: &Id, placed: &[Placed]) {
    let d = &app.model.diagrams[diagram];
    let zoom = app.view.zoom;
    let selected_node = match &app.selection {
        Selection::Node { node, .. } => Some(node.as_str()),
        _ => None,
    };
    let selected_element = match &app.selection {
        Selection::Element(e) => Some(e.as_str()),
        _ => None,
    };
    let fills: HashMap<&str, Option<&str>> = d
        .flatten()
        .into_iter()
        .map(|(n, _, _)| (n.id.as_str(), n.fill_color.as_deref()))
        .collect();

    for n in placed {
        let fill = fills.get(n.id.as_str()).copied().flatten();
        let selected = selected_node == Some(n.id.as_str());
        match &n.kind {
            DiagramNodeKind::Element(e) => {
                let Some(el) = app.model.elements.get(e) else {
                    p.rect_stroke(
                        n.rect,
                        CornerRadius::ZERO,
                        Stroke::new(1.0, Color32::RED),
                        StrokeKind::Inside,
                    );
                    continue;
                };
                let highlighted = selected || selected_element == Some(e.as_str());
                notation::draw_element(
                    p,
                    n.rect,
                    el.kind,
                    notation::element_fill(el.kind, fill),
                    &el.name,
                    zoom,
                    highlighted,
                );
            }
            DiagramNodeKind::Group { name } => {
                let color = fill
                    .and_then(notation::parse_hex)
                    .unwrap_or(Color32::from_rgb(210, 215, 215));
                notation::draw_group_frame(p, n.rect, color, name, zoom, false);
            }
            DiagramNodeKind::Note { content } => {
                let color = fill.and_then(notation::parse_hex).unwrap_or(Color32::WHITE);
                notation::draw_note(p, n.rect, color, content, zoom);
            }
            DiagramNodeKind::Reference(r) => {
                let name = app
                    .model
                    .diagrams
                    .get(r)
                    .map(|d| d.name.as_str())
                    .unwrap_or("View");
                p.rect(
                    n.rect,
                    CornerRadius::ZERO,
                    Color32::from_rgb(220, 235, 245),
                    Stroke::new(1.0, Color32::GRAY),
                    StrokeKind::Inside,
                );
                p.text(
                    n.rect.center(),
                    egui::Align2::CENTER_CENTER,
                    format!("↗ {name}"),
                    egui::FontId::proportional(12.0 * zoom),
                    Color32::BLACK,
                );
            }
            DiagramNodeKind::Other(t) => {
                p.rect_stroke(
                    n.rect,
                    CornerRadius::ZERO,
                    Stroke::new(1.0, Color32::GRAY),
                    StrokeKind::Inside,
                );
                p.text(
                    n.rect.center(),
                    egui::Align2::CENTER_CENTER,
                    t,
                    egui::FontId::proportional(10.0 * zoom),
                    Color32::GRAY,
                );
            }
        }
        if selected {
            if !matches!(n.kind, DiagramNodeKind::Element(_)) {
                p.rect_stroke(
                    n.rect.expand(2.0),
                    CornerRadius::same(2),
                    Stroke::new(2.0, Color32::from_rgb(30, 120, 255)),
                    StrokeKind::Outside,
                );
            }
            p.rect_filled(
                handle_rect(n.rect),
                CornerRadius::same(1),
                Color32::from_rgb(30, 120, 255),
            );
        }
    }

    let rects: HashMap<&str, Rect> = placed.iter().map(|n| (n.id.as_str(), n.rect)).collect();
    let selected_conn = match &app.selection {
        Selection::Connection { connection, .. } => Some(connection.as_str()),
        _ => None,
    };
    let selected_rel = match &app.selection {
        Selection::Relationship(r) => Some(r.as_str()),
        _ => None,
    };
    for c in &d.connections {
        let (Some(a), Some(b)) = (rects.get(c.source.as_str()), rects.get(c.target.as_str()))
        else {
            continue;
        };
        let (from, to) = (
            notation::clip_to_rect(*a, b.center()),
            notation::clip_to_rect(*b, a.center()),
        );
        let kind = c
            .relationship
            .as_ref()
            .and_then(|r| app.model.relationships.get(r))
            .map(|r| r.kind)
            .unwrap_or(RelationshipType::Association);
        let selected = selected_conn == Some(c.id.as_str())
            || (selected_rel.is_some() && c.relationship.as_deref() == selected_rel);
        notation::draw_relationship(p, kind, from, to, zoom, selected);
        if let Some(name) = c
            .relationship
            .as_ref()
            .and_then(|r| app.model.relationships.get(r))
            .map(|r| r.name.as_str())
            .filter(|n| !n.is_empty())
        {
            let mid = from + (to - from) / 2.0;
            p.text(
                mid + vec2(4.0, -4.0),
                egui::Align2::LEFT_BOTTOM,
                name,
                egui::FontId::proportional(11.0 * zoom),
                Color32::DARK_GRAY,
            );
        }
    }
}
