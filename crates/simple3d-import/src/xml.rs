//! A tag scanner, which is all the XML a 3MF model part needs.
//!
//! Only a flat sequence of elements and attributes is read (no validation, namespaces or text), so
//! a parser crate is unnecessary. Names are matched without their prefix, since the materials
//! extension appears as `<m:colorgroup>`, `<ns2:color>` or unprefixed depending on the writer.

/// One tag, as written.
#[derive(Clone, Copy, Debug)]
pub struct Tag<'a> {
    /// The element name without prefix; lowercased at comparison to keep the borrow cheap.
    pub name: &'a str,
    /// `</name>`.
    pub closing: bool,
    /// `<name/>`, which closes as it opens.
    pub empty: bool,
    attributes: &'a str,
}

impl<'a> Tag<'a> {
    /// Whether this opens `name`, ignoring case and prefix.
    pub fn opens(&self, name: &str) -> bool {
        !self.closing && self.name.eq_ignore_ascii_case(name)
    }

    /// Whether this closes `name`: `</name>` or an empty element's `/>`.
    pub fn closes(&self, name: &str) -> bool {
        (self.closing || self.empty) && self.name.eq_ignore_ascii_case(name)
    }

    /// An attribute's value with entities resolved, or `None` if absent.
    pub fn attr(&self, name: &str) -> Option<String> {
        let mut rest = self.attributes;
        while let Some(equals) = rest.find('=') {
            let key = rest[..equals].trim();
            let key = key.rsplit(':').next().unwrap_or(key);
            let after = &rest[equals + 1..];
            let after = after.trim_start();
            let quote = after.chars().next()?;
            if quote != '"' && quote != '\'' {
                return None;
            }
            let value_end = after[1..].find(quote)? + 1;
            let value = &after[1..value_end];
            if key.eq_ignore_ascii_case(name) {
                return Some(unescape(value));
            }
            rest = &after[value_end + 1..];
        }
        None
    }

    /// An attribute as a number, or `None` if absent or not a number.
    pub fn number(&self, name: &str) -> Option<f64> {
        let value = self.attr(name)?;
        let parsed = value.trim().parse::<f64>().ok()?;
        parsed.is_finite().then_some(parsed)
    }

    /// An attribute as a whole number, for 3MF indices and ids.
    pub fn index(&self, name: &str) -> Option<usize> {
        self.attr(name)?.trim().parse::<usize>().ok()
    }
}

/// Every tag in `text`, in order. Comments, processing instructions, doctypes and CDATA are skipped;
/// element text is not reported, since a 3MF model part carries none.
pub fn tags(text: &str) -> Tags<'_> {
    Tags { rest: text }
}

pub struct Tags<'a> {
    rest: &'a str,
}

impl<'a> Iterator for Tags<'a> {
    type Item = Tag<'a>;

    fn next(&mut self) -> Option<Tag<'a>> {
        loop {
            let open = self.rest.find('<')?;
            let after = &self.rest[open + 1..];
            // The three things starting with `<` that are not elements.
            if let Some(body) = after.strip_prefix("!--") {
                let end = body.find("-->").map(|at| at + 3).unwrap_or(body.len());
                self.rest = &body[end..];
                continue;
            }
            if let Some(body) = after.strip_prefix("![CDATA[") {
                let end = body.find("]]>").map(|at| at + 3).unwrap_or(body.len());
                self.rest = &body[end..];
                continue;
            }
            if after.starts_with('?') || after.starts_with('!') {
                let end = after.find('>').map(|at| at + 1).unwrap_or(after.len());
                self.rest = &after[end..];
                continue;
            }
            let end = match after.find('>') {
                Some(end) => end,
                // The file ends mid-tag: nothing left to read; the caller notices what is missing.
                None => {
                    self.rest = "";
                    return None;
                }
            };
            let inner = &after[..end];
            self.rest = &after[end + 1..];
            let (closing, inner) = match inner.strip_prefix('/') {
                Some(inner) => (true, inner),
                None => (false, inner),
            };
            let (empty, inner) = match inner.strip_suffix('/') {
                Some(inner) => (true, inner),
                None => (false, inner),
            };
            let inner = inner.trim();
            if inner.is_empty() {
                continue;
            }
            let split = inner.find(|c: char| c.is_whitespace()).unwrap_or(inner.len());
            let name = &inner[..split];
            let name = name.rsplit(':').next().unwrap_or(name);
            return Some(Tag { name, closing, empty, attributes: &inner[split..] });
        }
    }
}

/// Resolve XML's five named entities and numeric references; anything else is left as written.
fn unescape(value: &str) -> String {
    if !value.contains('&') {
        return value.to_string();
    }
    let mut out = String::with_capacity(value.len());
    let mut rest = value;
    while let Some(at) = rest.find('&') {
        out.push_str(&rest[..at]);
        let after = &rest[at..];
        let Some(end) = after.find(';') else {
            out.push_str(after);
            return out;
        };
        let entity = &after[1..end];
        match entity {
            "amp" => out.push('&'),
            "lt" => out.push('<'),
            "gt" => out.push('>'),
            "quot" => out.push('"'),
            "apos" => out.push('\''),
            _ => {
                let code = entity
                    .strip_prefix("#x")
                    .or_else(|| entity.strip_prefix("#X"))
                    .and_then(|hex| u32::from_str_radix(hex, 16).ok())
                    .or_else(|| entity.strip_prefix('#').and_then(|dec| dec.parse::<u32>().ok()));
                match code.and_then(char::from_u32) {
                    Some(c) => out.push(c),
                    None => out.push_str(&after[..=end]),
                }
            }
        }
        rest = &after[end + 1..];
    }
    out.push_str(rest);
    out
}
