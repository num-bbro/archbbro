//! A small XML DOM: enough for Archi's `.archimate` files (no DTDs, standard entities only).
//!
//! Keeping a generic tree lets us round-trip parts of a file this crate does not model
//! (sketch views, canvas views, profiles, images) without losing them.

use std::fmt::Write as _;

#[derive(Debug, Clone, PartialEq, Default)]
pub struct Node {
    pub name: String,
    pub attrs: Vec<(String, String)>,
    pub children: Vec<Node>,
    /// Concatenated character data directly inside this element.
    pub text: String,
}

impl Node {
    pub fn new(name: impl Into<String>) -> Self {
        Node {
            name: name.into(),
            ..Default::default()
        }
    }

    pub fn attr(&self, key: &str) -> Option<&str> {
        self.attrs
            .iter()
            .find(|(k, _)| k == key)
            .map(|(_, v)| v.as_str())
    }

    pub fn set_attr(&mut self, key: &str, value: impl Into<String>) {
        let value = value.into();
        if let Some(slot) = self.attrs.iter_mut().find(|(k, _)| k == key) {
            slot.1 = value;
        } else {
            self.attrs.push((key.to_string(), value));
        }
    }

    pub fn with_attr(mut self, key: &str, value: impl Into<String>) -> Self {
        self.set_attr(key, value);
        self
    }

    pub fn child(&self, name: &str) -> Option<&Node> {
        self.children.iter().find(|c| c.name == name)
    }
}

#[derive(Debug, thiserror::Error)]
#[error("XML parse error at byte {pos}: {msg}")]
pub struct XmlError {
    pub pos: usize,
    pub msg: String,
}

pub fn parse(input: &str) -> Result<Node, XmlError> {
    // XML 1.0 §2.11: line breaks are normalised to LF before parsing.
    let normalised;
    let input = if input.contains('\r') {
        normalised = input.replace("\r\n", "\n").replace('\r', "\n");
        normalised.as_str()
    } else {
        input
    };
    let mut p = Parser {
        s: input.as_bytes(),
        src: input,
        i: 0,
    };
    p.skip_prolog()?;
    let root = p.element()?;
    Ok(root)
}

struct Parser<'a> {
    s: &'a [u8],
    src: &'a str,
    i: usize,
}

impl<'a> Parser<'a> {
    fn err<T>(&self, msg: impl Into<String>) -> Result<T, XmlError> {
        Err(XmlError {
            pos: self.i,
            msg: msg.into(),
        })
    }

    fn starts(&self, pat: &str) -> bool {
        self.s[self.i..].starts_with(pat.as_bytes())
    }

    fn skip_ws(&mut self) {
        while self.i < self.s.len() && self.s[self.i].is_ascii_whitespace() {
            self.i += 1;
        }
    }

    fn skip_until(&mut self, pat: &str) -> Result<(), XmlError> {
        match self.src[self.i..].find(pat) {
            Some(off) => {
                self.i += off + pat.len();
                Ok(())
            }
            None => self.err(format!("unterminated construct, expected {pat}")),
        }
    }

    fn skip_misc(&mut self) -> Result<(), XmlError> {
        loop {
            self.skip_ws();
            if self.starts("<?") {
                self.skip_until("?>")?;
            } else if self.starts("<!--") {
                self.skip_until("-->")?;
            } else if self.starts("<!DOCTYPE") {
                return self.err("DOCTYPE declarations are not supported");
            } else {
                return Ok(());
            }
        }
    }

    fn skip_prolog(&mut self) -> Result<(), XmlError> {
        if self.starts("\u{feff}") {
            self.i += 3;
        }
        self.skip_misc()
    }

    fn name(&mut self) -> Result<String, XmlError> {
        let start = self.i;
        while self.i < self.s.len() {
            let c = self.s[self.i];
            if c.is_ascii_whitespace() || matches!(c, b'=' | b'>' | b'/' | b'<') {
                break;
            }
            self.i += 1;
        }
        if start == self.i {
            return self.err("expected a name");
        }
        Ok(self.src[start..self.i].to_string())
    }

    fn element(&mut self) -> Result<Node, XmlError> {
        if !self.starts("<") {
            return self.err("expected '<'");
        }
        self.i += 1;
        let mut node = Node::new(self.name()?);
        loop {
            self.skip_ws();
            if self.starts("/>") {
                self.i += 2;
                return Ok(node);
            }
            if self.starts(">") {
                self.i += 1;
                break;
            }
            let key = self.name()?;
            self.skip_ws();
            if !self.starts("=") {
                return self.err("expected '=' after attribute name");
            }
            self.i += 1;
            self.skip_ws();
            let quote = match self.s.get(self.i) {
                Some(b'"') => "\"",
                Some(b'\'') => "'",
                _ => return self.err("expected quoted attribute value"),
            };
            self.i += 1;
            let end = match self.src[self.i..].find(quote) {
                Some(off) => self.i + off,
                None => return self.err("unterminated attribute value"),
            };
            let raw = &self.src[self.i..end];
            self.i = end + 1;
            node.attrs.push((
                key,
                unescape(raw, true).map_err(|m| XmlError {
                    pos: self.i,
                    msg: m,
                })?,
            ));
        }
        // Content
        loop {
            if self.i >= self.s.len() {
                return self.err(format!("unclosed element <{}>", node.name));
            }
            if self.starts("</") {
                self.i += 2;
                let close = self.name()?;
                if close != node.name {
                    return self.err(format!("mismatched </{close}>, expected </{}>", node.name));
                }
                self.skip_ws();
                if !self.starts(">") {
                    return self.err("expected '>'");
                }
                self.i += 1;
                // Archi never uses mixed content: whitespace between child elements is layout.
                if !node.children.is_empty() && node.text.trim().is_empty() {
                    node.text.clear();
                }
                return Ok(node);
            } else if self.starts("<!--") {
                self.skip_until("-->")?;
            } else if self.starts("<![CDATA[") {
                self.i += 9;
                let start = self.i;
                self.skip_until("]]>")?;
                node.text.push_str(&self.src[start..self.i - 3]);
            } else if self.starts("<?") {
                self.skip_until("?>")?;
            } else if self.starts("<") {
                let child = self.element()?;
                node.children.push(child);
            } else {
                let start = self.i;
                while self.i < self.s.len() && self.s[self.i] != b'<' {
                    self.i += 1;
                }
                let raw = &self.src[start..self.i];
                node.text
                    .push_str(&unescape(raw, false).map_err(|m| XmlError { pos: start, msg: m })?);
            }
        }
    }
}

fn unescape(raw: &str, attr: bool) -> Result<String, String> {
    if !raw.contains('&') && !(attr && raw.contains(['\n', '\t', '\r'])) {
        return Ok(raw.to_string());
    }
    let mut out = String::with_capacity(raw.len());
    let mut rest = raw;
    while let Some(pos) = rest.find('&') {
        push_text(&mut out, &rest[..pos], attr);
        rest = &rest[pos + 1..];
        let end = rest.find(';').ok_or("unterminated entity")?;
        let ent = &rest[..end];
        rest = &rest[end + 1..];
        let ch = match ent {
            "lt" => '<',
            "gt" => '>',
            "amp" => '&',
            "quot" => '"',
            "apos" => '\'',
            _ if ent.starts_with("#x") || ent.starts_with("#X") => {
                char::from_u32(u32::from_str_radix(&ent[2..], 16).map_err(|e| e.to_string())?)
                    .ok_or("invalid character reference")?
            }
            _ if ent.starts_with('#') => {
                char::from_u32(ent[1..].parse::<u32>().map_err(|e| e.to_string())?)
                    .ok_or("invalid character reference")?
            }
            _ => return Err(format!("unknown entity &{ent};")),
        };
        out.push(ch);
    }
    push_text(&mut out, rest, attr);
    Ok(out)
}

/// Attribute values normalise literal whitespace characters to spaces (XML 1.0 §3.3.3).
fn push_text(out: &mut String, s: &str, attr: bool) {
    if attr {
        out.extend(s.chars().map(|c| {
            if matches!(c, '\n' | '\t' | '\r') {
                ' '
            } else {
                c
            }
        }));
    } else {
        out.push_str(s);
    }
}

/// Escapes like Archi (EMF): quotes are escaped in text too.
fn escape_into(out: &mut String, s: &str, attr: bool) {
    for c in s.chars() {
        match c {
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '&' => out.push_str("&amp;"),
            '"' => out.push_str("&quot;"),
            '\n' if attr => out.push_str("&#xA;"),
            '\r' => out.push_str("&#xD;"),
            '\t' if attr => out.push_str("&#x9;"),
            _ => out.push(c),
        }
    }
}

/// Serialise with Archi's layout: two-space indent, text-only elements kept on one line.
pub fn write_document(root: &Node) -> String {
    let mut out = String::from("<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n");
    write_node(&mut out, root, 0);
    out
}

fn write_node(out: &mut String, n: &Node, depth: usize) {
    for _ in 0..depth {
        out.push_str("  ");
    }
    let _ = write!(out, "<{}", n.name);
    for (k, v) in &n.attrs {
        let _ = write!(out, " {k}=\"");
        escape_into(out, v, true);
        out.push('"');
    }
    if n.children.is_empty() && n.text.is_empty() {
        out.push_str("/>\n");
        return;
    }
    out.push('>');
    escape_into(out, &n.text, false);
    if !n.children.is_empty() {
        out.push('\n');
        for c in &n.children {
            write_node(out, c, depth + 1);
        }
        for _ in 0..depth {
            out.push_str("  ");
        }
    }
    let _ = writeln!(out, "</{}>", n.name);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_and_roundtrips() {
        let src = r#"<?xml version="1.0"?><a x="1 &amp; 2"><!-- c --><b y='q'/><t>hi &lt;there&gt;</t></a>"#;
        let n = parse(src).unwrap();
        assert_eq!(n.attr("x"), Some("1 & 2"));
        assert_eq!(n.children[0].attr("y"), Some("q"));
        assert_eq!(n.children[1].text, "hi <there>");
        let again = parse(&write_document(&n)).unwrap();
        assert_eq!(n, again);
    }

    #[test]
    fn newline_in_attribute_survives() {
        let mut n = Node::new("a");
        n.set_attr("v", "line1\nline2");
        let back = parse(&write_document(&n)).unwrap();
        assert_eq!(back.attr("v"), Some("line1\nline2"));
    }

    #[test]
    fn rejects_doctype() {
        assert!(parse("<!DOCTYPE x [<!ENTITY e 'x'>]><a/>").is_err());
    }
}
