//! The arithmetic a field accepts, so a dimension can be typed as a sum.

use super::*;

/// The expression reader. Deliberately small: four operators, parentheses, one
/// leading sign per factor, and a unit suffix on any number.
///
/// `unit` is the document's display unit, and `None` means suffixes convert to
/// nothing -- an angle or a count has no length to be expressed in.
pub(crate) fn evaluate(text: &str, unit: Option<Unit>) -> Option<f64> {
    let tokens = tokenize(text)?;
    let mut parser = Parser { tokens: &tokens, at: 0, unit };
    let value = parser.expr()?;
    if parser.at != parser.tokens.len() {
        return None;
    }
    value.is_finite().then_some(value)
}

#[derive(Clone, Debug, PartialEq)]
pub(crate) enum Token {
    Number(f64),
    /// A unit suffix that followed a number.
    Suffix(&'static str),
    Op(char),
}

/// Suffixes a number may carry. `deg` and the degree sign are lengths of
/// nothing: they are accepted so that copying a value back out of an angle
/// field parses, and they convert nothing.
pub(crate) fn suffix_mm_per(name: &str) -> Option<Option<f64>> {
    match name {
        "mm" => Some(Some(1.0)),
        "cm" => Some(Some(10.0)),
        "m" => Some(Some(1000.0)),
        "deg" | "\u{00B0}" => Some(None),
        _ => None,
    }
}

pub(crate) fn tokenize(text: &str) -> Option<Vec<Token>> {
    let chars: Vec<char> = text.chars().collect();
    let mut tokens = Vec::new();
    let mut i = 0;
    while i < chars.len() {
        let c = chars[i];
        if c.is_whitespace() {
            i += 1;
            continue;
        }
        if c.is_ascii_digit() || c == '.' || c == ',' {
            let start = i;
            let mut separators = 0;
            while i < chars.len() && (chars[i].is_ascii_digit() || chars[i] == '.' || chars[i] == ',') {
                // Both `1.8` and `1,8` mean the same thing; a second separator
                // in one number is a mistake, and a thousands separator is not
                // supported, which makes it one too.
                if chars[i] == '.' || chars[i] == ',' {
                    separators += 1;
                    if separators > 1 {
                        return None;
                    }
                }
                i += 1;
            }
            let raw: String = chars[start..i].iter().collect::<String>().replace(',', ".");
            tokens.push(Token::Number(raw.parse::<f64>().ok()?));
            continue;
        }
        if c.is_alphabetic() || c == '\u{00B0}' {
            let start = i;
            while i < chars.len() && (chars[i].is_alphabetic() || chars[i] == '\u{00B0}') {
                i += 1;
            }
            let name: String = chars[start..i].iter().collect();
            // Reject an unknown word here rather than at the parser, so `12mmm`
            // fails as one mistake instead of as a trailing token.
            suffix_mm_per(&name)?;
            tokens.push(Token::Suffix(match name.as_str() {
                "mm" => "mm",
                "cm" => "cm",
                "m" => "m",
                "deg" => "deg",
                _ => "\u{00B0}",
            }));
            continue;
        }
        if matches!(c, '+' | '-' | '*' | '/' | '(' | ')') {
            tokens.push(Token::Op(c));
            i += 1;
            continue;
        }
        return None;
    }
    (!tokens.is_empty()).then_some(tokens)
}

pub(crate) struct Parser<'a> {
    pub(super) tokens: &'a [Token],
    pub(super) at: usize,
    pub(super) unit: Option<Unit>,
}

impl Parser<'_> {
    pub(super) fn peek_op(&self) -> Option<char> {
        match self.tokens.get(self.at) {
            Some(Token::Op(c)) => Some(*c),
            _ => None,
        }
    }

    pub(super) fn expr(&mut self) -> Option<f64> {
        let mut value = self.term()?;
        while let Some(op @ ('+' | '-')) = self.peek_op() {
            self.at += 1;
            let rhs = self.term()?;
            value = if op == '+' { value + rhs } else { value - rhs };
        }
        Some(value)
    }

    pub(super) fn term(&mut self) -> Option<f64> {
        let mut value = self.factor()?;
        while let Some(op @ ('*' | '/')) = self.peek_op() {
            self.at += 1;
            let rhs = self.factor()?;
            if op == '/' {
                // Division by zero gives infinity, which `evaluate` rejects: a
                // field must never end up holding a value that cannot be drawn.
                value /= rhs;
            } else {
                value *= rhs;
            }
        }
        Some(value)
    }

    /// One optional sign, then a primary. Stacking signs (`--4`) is a typing
    /// mistake far more often than it is arithmetic, so it is refused.
    pub(super) fn factor(&mut self) -> Option<f64> {
        match self.peek_op() {
            Some('-') => {
                self.at += 1;
                Some(-self.primary()?)
            }
            Some('+') => {
                self.at += 1;
                self.primary()
            }
            _ => self.primary(),
        }
    }

    pub(super) fn primary(&mut self) -> Option<f64> {
        match self.tokens.get(self.at)? {
            Token::Number(n) => {
                let mut value = *n;
                self.at += 1;
                if let Some(Token::Suffix(name)) = self.tokens.get(self.at) {
                    let mm_per = suffix_mm_per(name)?;
                    self.at += 1;
                    // `4 cm` in a millimetre document is 40 of what the field
                    // shows; in a metre document it is 0.04 of it.
                    if let (Some(mm_per), Some(unit)) = (mm_per, self.unit) {
                        value = value * mm_per / unit.mm_per();
                    }
                }
                Some(value)
            }
            Token::Op('(') => {
                self.at += 1;
                let value = self.expr()?;
                if self.peek_op() != Some(')') {
                    return None;
                }
                self.at += 1;
                Some(value)
            }
            _ => None,
        }
    }
}
