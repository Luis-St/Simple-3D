//! A tag scanner, which is all the XML a 3MF model part needs reading as.
//!
//! What has to be understood is a flat sequence of elements and their
//! attributes: `<object>`, `<vertex x= y= z=>`, `<triangle v1= v2= v3=>`,
//! `<item objectid= transform=>`. Nothing is asked of the document's shape --
//! no validation, no namespace resolution, no text content -- so a scanner
//! that yields one tag at a time is the whole job, and a parser crate would be
//! a dependency for less than this file does.
//!
//! Element names are reported without their prefix. 3MF's materials extension
//! is written `<m:colorgroup>` by this workspace's exporter and `<ns2:color>`
//! or plain `<colorgroup>` by other programs, all meaning the same element:
//! matching on the local name is what makes those the same file to read.

/// One tag, as it was written.
#[derive(Clone, Copy, Debug)]
pub struct Tag<'a> {
    /// The element name with any namespace prefix removed, lowercased at the
    /// point of comparison rather than here so the borrow stays cheap.
    pub name: &'a str,
    /// `</name>`.
    pub closing: bool,
    /// `<name/>`, which closes as it opens.
    pub empty: bool,
    attributes: &'a str,
}

impl<'a> Tag<'a> {
    /// Whether this is the opening of `name`, compared without case or prefix.
    pub fn opens(&self, name: &str) -> bool {
        !self.closing && self.name.eq_ignore_ascii_case(name)
    }

    /// Whether this is the close of `name` -- either `</name>` or the `/>` of
    /// an empty element, since the two say the same thing to a reader tracking
    /// depth.
    pub fn closes(&self, name: &str) -> bool {
        (self.closing || self.empty) && self.name.eq_ignore_ascii_case(name)
    }

    /// An attribute's value with its entities resolved, or `None` when the tag
    /// does not carry one by that name.
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

    /// An attribute read as a number, `None` when it is absent or not one.
    pub fn number(&self, name: &str) -> Option<f64> {
        let value = self.attr(name)?;
        let parsed = value.trim().parse::<f64>().ok()?;
        parsed.is_finite().then_some(parsed)
    }

    /// An attribute read as a whole number, for the indices and ids a 3MF is
    /// held together by.
    pub fn index(&self, name: &str) -> Option<usize> {
        self.attr(name)?.trim().parse::<usize>().ok()
    }
}

/// Every tag in `text`, in order. Comments, processing instructions, doctypes
/// and CDATA are skipped; text between tags is not reported, because nothing in
/// a 3MF model part is carried as element text.
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
            // The three things that start with `<` and are not an element.
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
                // A tag the file ends in the middle of: there is nothing left
                // to read, and the caller finds out from what is missing.
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

/// Resolve the five named entities XML defines, and numeric character
/// references. Anything else is left exactly as written: an unknown entity in
/// a node's name is not a reason to refuse a model.
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
