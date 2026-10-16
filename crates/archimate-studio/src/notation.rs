//! How ArchiMate concepts look on a view: element shapes and relationship line styles.

use archimate_core::{ElementType, RelationshipType};
use eframe::egui::{
    pos2, vec2, Color32, CornerRadius, Painter, Pos2, Rect, Shape, Stroke, StrokeKind, Vec2,
};

pub fn element_fill(kind: ElementType, custom: Option<&str>) -> Color32 {
    if let Some(c) = custom.and_then(parse_hex) {
        return c;
    }
    let [r, g, b] = kind.default_color();
    Color32::from_rgb(r, g, b)
}

pub fn parse_hex(s: &str) -> Option<Color32> {
    let s = s.strip_prefix('#')?;
    if s.len() != 6 {
        return None;
    }
    let v = u32::from_str_radix(s, 16).ok()?;
    Some(Color32::from_rgb((v >> 16) as u8, (v >> 8) as u8, v as u8))
}

pub fn to_hex(c: Color32) -> String {
    format!("#{:02x}{:02x}{:02x}", c.r(), c.g(), c.b())
}

/// Corner style used by Archi for each family of elements.
fn corner(kind: ElementType, rect: Rect) -> CornerRadius {
    use ElementType::*;
    let name = kind.name();
    let behaviour = name.ends_with("Process")
        || name.ends_with("Function")
        || name.ends_with("Interaction")
        || name.ends_with("Event")
        || matches!(kind, ValueStream | CourseOfAction | Capability);
    if name.ends_with("Service") {
        CornerRadius::same((rect.height() / 2.0).min(255.0) as u8)
    } else if behaviour
        || matches!(
            kind,
            Goal | Outcome
                | Principle
                | Requirement
                | Constraint
                | Driver
                | Assessment
                | Value
                | Meaning
                | Stakeholder
        )
    {
        CornerRadius::same(8)
    } else {
        CornerRadius::ZERO
    }
}

pub fn draw_element(
    p: &Painter,
    rect: Rect,
    kind: ElementType,
    fill: Color32,
    name: &str,
    zoom: f32,
    selected: bool,
) {
    let outline = Stroke::new(1.0, Color32::from_gray(90));
    if kind == ElementType::Junction {
        let r = rect.width().min(rect.height()) / 2.0;
        p.circle_filled(rect.center(), r, Color32::BLACK);
    } else if kind == ElementType::Grouping {
        draw_group_frame(p, rect, fill, name, zoom, true);
    } else {
        p.rect(rect, corner(kind, rect), fill, outline, StrokeKind::Inside);
        draw_icon(
            p,
            kind,
            pos2(rect.right() - 14.0 * zoom, rect.top() + 10.0 * zoom),
            zoom,
        );
        let font = eframe::egui::FontId::proportional(12.0 * zoom);
        let galley = p.layout(
            name.to_string(),
            font,
            Color32::BLACK,
            (rect.width() - 24.0 * zoom).max(10.0),
        );
        let pos = pos2(
            rect.center().x - galley.size().x / 2.0,
            rect.top() + 18.0 * zoom,
        );
        p.with_clip_rect(rect).galley(pos, galley, Color32::BLACK);
    }
    if selected {
        p.rect_stroke(
            rect.expand(2.0),
            CornerRadius::same(2),
            Stroke::new(2.0, Color32::from_rgb(30, 120, 255)),
            StrokeKind::Outside,
        );
    }
}

pub fn draw_group_frame(
    p: &Painter,
    rect: Rect,
    fill: Color32,
    name: &str,
    zoom: f32,
    dashed: bool,
) {
    let tab = Rect::from_min_size(
        rect.min,
        vec2((rect.width() * 0.5).max(60.0 * zoom), 18.0 * zoom),
    );
    let body = Rect::from_min_max(pos2(rect.left(), tab.bottom()), rect.max);
    let stroke = Stroke::new(1.0, Color32::from_gray(90));
    p.rect_filled(tab, CornerRadius::ZERO, fill);
    p.rect_filled(body, CornerRadius::ZERO, fill.gamma_multiply(0.6));
    if dashed {
        let pts = [
            tab.left_top(),
            tab.right_top(),
            pos2(tab.right(), body.top()),
            body.right_top(),
            body.right_bottom(),
            body.left_bottom(),
            tab.left_top(),
        ];
        p.extend(Shape::dashed_line(&pts, stroke, 6.0, 4.0));
    } else {
        p.rect_stroke(tab, CornerRadius::ZERO, stroke, StrokeKind::Inside);
        p.rect_stroke(body, CornerRadius::ZERO, stroke, StrokeKind::Inside);
    }
    p.text(
        tab.left_center() + vec2(6.0, 0.0),
        eframe::egui::Align2::LEFT_CENTER,
        name,
        eframe::egui::FontId::proportional(12.0 * zoom),
        Color32::BLACK,
    );
}

pub fn draw_note(p: &Painter, rect: Rect, fill: Color32, text: &str, zoom: f32) {
    let fold = 10.0 * zoom;
    let pts = vec![
        rect.left_top(),
        pos2(rect.right() - fold, rect.top()),
        pos2(rect.right(), rect.top() + fold),
        rect.right_bottom(),
        rect.left_bottom(),
    ];
    p.add(Shape::convex_polygon(
        pts,
        fill,
        Stroke::new(1.0, Color32::from_gray(90)),
    ));
    let font = eframe::egui::FontId::proportional(11.0 * zoom);
    let galley = p.layout(text.to_string(), font, Color32::BLACK, rect.width() - 10.0);
    p.with_clip_rect(rect)
        .galley(rect.min + vec2(5.0, 5.0), galley, Color32::BLACK);
}

/// A small glyph in the top-right corner, close to Archi's figure icons.
fn draw_icon(p: &Painter, kind: ElementType, c: Pos2, z: f32) {
    use ElementType::*;
    let s = Stroke::new(1.0, Color32::BLACK);
    let name = kind.name();
    let r = |w: f32, h: f32| Rect::from_center_size(c, vec2(w * z, h * z));
    match kind {
        BusinessActor | Stakeholder => {
            p.circle_stroke(c + vec2(0.0, -4.0) * z, 2.5 * z, s);
            p.line_segment([c + vec2(0.0, -1.5) * z, c + vec2(0.0, 3.0) * z], s);
            p.line_segment([c + vec2(-4.0, 0.0) * z, c + vec2(4.0, 0.0) * z], s);
            p.line_segment([c + vec2(0.0, 3.0) * z, c + vec2(-3.0, 7.0) * z], s);
            p.line_segment([c + vec2(0.0, 3.0) * z, c + vec2(3.0, 7.0) * z], s);
        }
        BusinessRole => {
            p.rect_stroke(r(12.0, 7.0), CornerRadius::same(3), s, StrokeKind::Middle);
        }
        _ if name.ends_with("Collaboration") => {
            p.circle_stroke(c + vec2(-2.5, 0.0) * z, 4.0 * z, s);
            p.circle_stroke(c + vec2(2.5, 0.0) * z, 4.0 * z, s);
        }
        _ if name.ends_with("Interface") => {
            p.circle_stroke(c + vec2(2.5, 0.0) * z, 3.5 * z, s);
            p.line_segment([c + vec2(-6.0, 0.0) * z, c + vec2(-1.0, 0.0) * z], s);
        }
        _ if name.ends_with("Process") || kind == ValueStream => {
            p.arrow(c + vec2(-6.0, 0.0) * z, vec2(12.0, 0.0) * z, s);
        }
        _ if name.ends_with("Function") => {
            let pts = vec![
                c + vec2(-5.0, 6.0) * z,
                c + vec2(-5.0, -3.0) * z,
                c + vec2(0.0, -6.0) * z,
                c + vec2(5.0, -3.0) * z,
                c + vec2(5.0, 6.0) * z,
                c + vec2(0.0, 3.0) * z,
            ];
            p.add(Shape::closed_line(pts, s));
        }
        _ if name.ends_with("Interaction") => {
            p.circle_stroke(c, 5.0 * z, s);
            p.line_segment([c + vec2(0.0, -5.0) * z, c + vec2(0.0, 5.0) * z], s);
        }
        _ if name.ends_with("Event") => {
            p.rect_stroke(r(12.0, 7.0), CornerRadius::same(3), s, StrokeKind::Middle);
        }
        _ if name.ends_with("Service") => {
            p.rect_stroke(r(13.0, 7.0), CornerRadius::same(3), s, StrokeKind::Middle);
        }
        BusinessObject | DataObject | Contract | Representation | Artifact | Deliverable => {
            let b = r(12.0, 9.0);
            p.rect_stroke(b, CornerRadius::ZERO, s, StrokeKind::Middle);
            p.line_segment(
                [
                    pos2(b.left(), b.top() + 3.0 * z),
                    pos2(b.right(), b.top() + 3.0 * z),
                ],
                s,
            );
        }
        ApplicationComponent => {
            let b = r(10.0, 11.0);
            p.rect_stroke(b, CornerRadius::ZERO, s, StrokeKind::Middle);
            for dy in [-2.5, 2.5] {
                p.rect_filled(
                    Rect::from_center_size(pos2(b.left(), c.y + dy * z), vec2(5.0, 3.0) * z),
                    CornerRadius::ZERO,
                    Color32::WHITE,
                );
                p.rect_stroke(
                    Rect::from_center_size(pos2(b.left(), c.y + dy * z), vec2(5.0, 3.0) * z),
                    CornerRadius::ZERO,
                    s,
                    StrokeKind::Middle,
                );
            }
        }
        Node | Device | Equipment | Facility => {
            let b = r(10.0, 9.0).translate(vec2(-1.0, 1.0) * z);
            p.rect_stroke(b, CornerRadius::ZERO, s, StrokeKind::Middle);
            p.line_segment([b.left_top(), b.left_top() + vec2(3.0, -3.0) * z], s);
            p.line_segment([b.right_top(), b.right_top() + vec2(3.0, -3.0) * z], s);
            p.line_segment(
                [b.right_bottom(), b.right_bottom() + vec2(3.0, -3.0) * z],
                s,
            );
            p.line_segment(
                [
                    b.left_top() + vec2(3.0, -3.0) * z,
                    b.right_top() + vec2(3.0, -3.0) * z,
                ],
                s,
            );
            p.line_segment(
                [
                    b.right_top() + vec2(3.0, -3.0) * z,
                    b.right_bottom() + vec2(3.0, -3.0) * z,
                ],
                s,
            );
        }
        SystemSoftware => {
            p.circle_stroke(c, 5.0 * z, s);
            p.circle_stroke(c + vec2(-1.5, 1.5) * z, 3.5 * z, s);
        }
        Goal | Outcome => {
            p.circle_stroke(c, 5.5 * z, s);
            p.circle_stroke(c, 3.0 * z, s);
            p.circle_filled(c, 1.0 * z, Color32::BLACK);
        }
        Requirement | Constraint | Principle => {
            let pts = vec![
                c + vec2(-4.0, -4.0) * z,
                c + vec2(6.0, -4.0) * z,
                c + vec2(4.0, 4.0) * z,
                c + vec2(-6.0, 4.0) * z,
            ];
            p.add(Shape::closed_line(pts, s));
        }
        Capability => {
            for (dx, dy) in [
                (2.0, -4.0),
                (2.0, 0.0),
                (-2.0, 0.0),
                (2.0, 4.0),
                (-2.0, 4.0),
                (-6.0, 4.0),
            ] {
                p.rect_stroke(
                    Rect::from_center_size(c + vec2(dx + 2.0, dy) * z, vec2(4.0, 4.0) * z),
                    CornerRadius::ZERO,
                    s,
                    StrokeKind::Middle,
                );
            }
        }
        Location => {
            p.circle_stroke(c + vec2(0.0, -2.0) * z, 3.5 * z, s);
            p.line_segment([c + vec2(0.0, 1.5) * z, c + vec2(0.0, 6.0) * z], s);
        }
        _ => {}
    }
}

/// Where a line from the centre of `rect` towards `toward` leaves the rectangle.
pub fn clip_to_rect(rect: Rect, toward: Pos2) -> Pos2 {
    let c = rect.center();
    let d = toward - c;
    if d.length_sq() < 1e-6 {
        return c;
    }
    let sx = if d.x.abs() > 1e-6 {
        (rect.width() / 2.0) / d.x.abs()
    } else {
        f32::INFINITY
    };
    let sy = if d.y.abs() > 1e-6 {
        (rect.height() / 2.0) / d.y.abs()
    } else {
        f32::INFINITY
    };
    c + d * sx.min(sy).min(1.0)
}

fn head_points(tip: Pos2, dir: Vec2, len: f32, half: f32) -> [Pos2; 3] {
    let n = vec2(-dir.y, dir.x);
    [tip, tip - dir * len + n * half, tip - dir * len - n * half]
}

/// Draw a relationship between two points (already clipped to the figures).
pub fn draw_relationship(
    p: &Painter,
    kind: RelationshipType,
    from: Pos2,
    to: Pos2,
    zoom: f32,
    selected: bool,
) {
    use RelationshipType::*;
    let color = if selected {
        Color32::from_rgb(30, 120, 255)
    } else {
        Color32::BLACK
    };
    let s = Stroke::new(if selected { 2.0 } else { 1.0 }, color);
    let d = to - from;
    if d.length() < 1.0 {
        return;
    }
    let dir = d.normalized();
    let z = zoom.max(0.4);
    match kind {
        Realization | Influence | Flow => {
            p.extend(Shape::dashed_line(&[from, to], s, 5.0 * z, 3.0 * z));
        }
        Access => {
            p.extend(Shape::dashed_line(&[from, to], s, 1.5 * z, 2.5 * z));
        }
        _ => {
            p.line_segment([from, to], s);
        }
    }
    let white = Color32::WHITE;
    match kind {
        Composition | Aggregation => {
            let n = vec2(-dir.y, dir.x);
            let pts = vec![
                from,
                from + dir * 7.0 * z + n * 4.0 * z,
                from + dir * 14.0 * z,
                from + dir * 7.0 * z - n * 4.0 * z,
            ];
            p.add(Shape::convex_polygon(
                pts,
                if kind == Composition { color } else { white },
                s,
            ));
        }
        Assignment => {
            p.circle_filled(from + dir * 3.0 * z, 3.0 * z, color);
            p.add(Shape::convex_polygon(
                head_points(to, dir, 9.0 * z, 4.0 * z).to_vec(),
                color,
                s,
            ));
        }
        Realization | Specialization => {
            p.add(Shape::convex_polygon(
                head_points(to, dir, 11.0 * z, 6.0 * z).to_vec(),
                white,
                s,
            ));
        }
        Serving | Influence | Access => {
            let [a, b, c] = head_points(to, dir, 9.0 * z, 5.0 * z);
            p.line_segment([b, a], s);
            p.line_segment([c, a], s);
        }
        Triggering | Flow => {
            p.add(Shape::convex_polygon(
                head_points(to, dir, 10.0 * z, 4.5 * z).to_vec(),
                color,
                s,
            ));
        }
        Association => {}
    }
}

pub fn distance_to_segment(p: Pos2, a: Pos2, b: Pos2) -> f32 {
    let ab = b - a;
    let t = ((p - a).dot(ab) / ab.length_sq().max(1e-6)).clamp(0.0, 1.0);
    (a + ab * t).distance(p)
}
