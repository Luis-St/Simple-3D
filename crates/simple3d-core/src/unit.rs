//! Display units (spec section 4).
//!
//! Lengths are stored in millimetres everywhere. A `Unit` only changes how a
//! number is shown and read back, never the model: with metres selected,
//! typing `1.8` stores 1800mm and the field afterwards reads `1.8`.

use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Unit {
    #[serde(rename = "mm")]
    Millimetre,
    #[serde(rename = "cm")]
    Centimetre,
    #[serde(rename = "m")]
    Metre,
}

impl Default for Unit {
    fn default() -> Self {
        Unit::Millimetre
    }
}

impl Unit {
    pub const ALL: [Unit; 3] = [Unit::Millimetre, Unit::Centimetre, Unit::Metre];

    /// Millimetres per one of this unit.
    pub fn mm_per(self) -> f64 {
        match self {
            Unit::Millimetre => 1.0,
            Unit::Centimetre => 10.0,
            Unit::Metre => 1000.0,
        }
    }

    pub fn suffix(self) -> &'static str {
        match self {
            Unit::Millimetre => "mm",
            Unit::Centimetre => "cm",
            Unit::Metre => "m",
        }
    }

    /// Decimal places worth showing so that a value entered in this unit round
    /// -trips: millimetres are stored exactly, so three places is ample; metres
    /// need six to express a single millimetre.
    pub fn decimals(self) -> usize {
        match self {
            Unit::Millimetre => 4,
            Unit::Centimetre => 5,
            Unit::Metre => 7,
        }
    }

    pub fn from_mm(self, mm: f64) -> f64 {
        mm / self.mm_per()
    }

    pub fn to_mm(self, value: f64) -> f64 {
        value * self.mm_per()
    }
}

/// Format a number for display without floating-point noise: `1.8`, never
/// `1.7999999999`. Trailing zeros and a trailing point are trimmed.
pub fn format_number(value: f64, decimals: usize) -> String {
    if !value.is_finite() {
        return "0".to_string();
    }
    let mut s = format!("{value:.decimals$}");
    if s.contains('.') {
        while s.ends_with('0') {
            s.pop();
        }
        if s.ends_with('.') {
            s.pop();
        }
    }
    if s == "-0" {
        s = "0".to_string();
    }
    s
}

/// Format a stored millimetre length in the given display unit.
pub fn format_length(mm: f64, unit: Unit) -> String {
    format_number(unit.from_mm(mm), unit.decimals())
}

/// Format an angle in degrees. Angles are always degrees regardless of the
/// length unit (spec section 4).
pub fn format_angle(deg: f64) -> String {
    format_number(deg, 4)
}

/// What the user typed into a numeric field, once it has been read.
///
/// A field accepts more than a number: an expression (`40/3`), a value in a unit
/// other than the document's (`4 cm` in a millimetre drawing) and a *delta*
/// (`+2`, `- 5`) that adjusts whatever is already there. The delta is the reason
/// this is a struct rather than an `f64`: only the caller knows what "already
/// there" is, and with several nodes selected each of them has its own.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Entry {
    /// The number, expressed in the unit the field is displayed in.
    pub value: f64,
    /// True when `value` is an adjustment to add to the current value rather
    /// than a replacement for it.
    pub relative: bool,
}

impl Entry {
    /// Resolve against what the field currently holds, in the display unit.
    pub fn resolve(self, current: f64) -> f64 {
        if self.relative {
            current + self.value
        } else {
            self.value
        }
    }
}

/// Read a field's text in the context of a display unit.
///
/// Absolute (`40`), expression (`40/3`, `12+8`, `(2+3)*4`), unit-suffixed
/// (`4 cm`, which is `40` in a millimetre document and `0.04` in a metre one)
/// and relative (`+2`, `+= 2`, `- 5`, `-= 5`).
///
/// **A leading `-` is only a delta when a space or an `=` follows it.** `-5` has
/// to keep meaning minus five, because a position field must be able to hold a
/// negative number, and no field can read the same six keystrokes two ways. `+`
/// has no such conflict, so a bare `+5` is a delta.
pub fn parse_entry(text: &str, unit: Unit) -> Option<Entry> {
    parse_entry_in(text, Some(unit))
}

/// `parse_entry` for a field that is not a length -- an angle, a count -- where
/// a unit suffix converts nothing because there is nothing to convert into.
pub fn parse_entry_plain(text: &str) -> Option<Entry> {
    parse_entry_in(text, None)
}

fn parse_entry_in(text: &str, unit: Option<Unit>) -> Option<Entry> {
    let trimmed = text.trim();
    let (relative, negate, rest) = if let Some(rest) = trimmed.strip_prefix("+=") {
        (true, false, rest)
    } else if let Some(rest) = trimmed.strip_prefix("-=") {
        (true, true, rest)
    } else if let Some(rest) = trimmed.strip_prefix('+') {
        (true, false, rest)
    } else if let Some(rest) = trimmed.strip_prefix('-').filter(|r| r.starts_with(char::is_whitespace)) {
        (true, true, rest)
    } else {
        (false, false, trimmed)
    };
    let value = evaluate(rest, unit)?;
    Some(Entry { value: if negate { -value } else { value }, relative })
}

/// Parse a number with no unit context: any suffix the user typed is accepted
/// and ignored, since there is nothing to convert it into. Expressions work
/// here too. `None` on anything unparseable -- callers restore the previous
/// value and mark the field rather than raising a dialog (spec section 4).
pub fn parse_number(text: &str) -> Option<f64> {
    evaluate(text.trim(), None)
}

/// The expression reader. Deliberately small: four operators, parentheses, one
/// leading sign per factor, and a unit suffix on any number.
///
/// `unit` is the document's display unit, and `None` means suffixes convert to
/// nothing -- an angle or a count has no length to be expressed in.
fn evaluate(text: &str, unit: Option<Unit>) -> Option<f64> {
    let tokens = tokenize(text)?;
    let mut parser = Parser { tokens: &tokens, at: 0, unit };
    let value = parser.expr()?;
    if parser.at != parser.tokens.len() {
        return None;
    }
    value.is_finite().then_some(value)
}

#[derive(Clone, Debug, PartialEq)]
enum Token {
    Number(f64),
    /// A unit suffix that followed a number.
    Suffix(&'static str),
    Op(char),
}

/// Suffixes a number may carry. `deg` and the degree sign are lengths of
/// nothing: they are accepted so that copying a value back out of an angle
/// field parses, and they convert nothing.
fn suffix_mm_per(name: &str) -> Option<Option<f64>> {
    match name {
        "mm" => Some(Some(1.0)),
        "cm" => Some(Some(10.0)),
        "m" => Some(Some(1000.0)),
        "deg" | "\u{00B0}" => Some(None),
        _ => None,
    }
}

fn tokenize(text: &str) -> Option<Vec<Token>> {
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

struct Parser<'a> {
    tokens: &'a [Token],
    at: usize,
    unit: Option<Unit>,
}

impl Parser<'_> {
    fn peek_op(&self) -> Option<char> {
        match self.tokens.get(self.at) {
            Some(Token::Op(c)) => Some(*c),
            _ => None,
        }
    }

    fn expr(&mut self) -> Option<f64> {
        let mut value = self.term()?;
        while let Some(op @ ('+' | '-')) = self.peek_op() {
            self.at += 1;
            let rhs = self.term()?;
            value = if op == '+' { value + rhs } else { value - rhs };
        }
        Some(value)
    }

    fn term(&mut self) -> Option<f64> {
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
    fn factor(&mut self) -> Option<f64> {
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

    fn primary(&mut self) -> Option<f64> {
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

/// Parse an absolute length typed in `unit` into stored millimetres. A relative
/// entry is refused here rather than silently read as an absolute one: the
/// caller that has no current value to add it to must not guess.
pub fn parse_length(text: &str, unit: Unit) -> Option<f64> {
    let entry = parse_entry(text, unit)?;
    (!entry.relative).then(|| unit.to_mm(entry.value))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn display_unit_round_trips_without_rescaling() {
        // Spec acceptance criterion 6.
        let plate = [40.0, 20.0, 4.0];
        let shown: Vec<String> = plate.iter().map(|&mm| format_length(mm, Unit::Metre)).collect();
        assert_eq!(shown, vec!["0.04", "0.02", "0.004"]);
        for (&mm, s) in plate.iter().zip(shown.iter()) {
            assert_eq!(parse_length(s, Unit::Metre), Some(mm));
        }
        for &mm in &plate {
            assert_eq!(parse_length(&format_length(mm, Unit::Millimetre), Unit::Millimetre), Some(mm));
        }
    }

    #[test]
    fn typing_in_metres_stores_millimetres() {
        assert_eq!(parse_length("1.8", Unit::Metre), Some(1800.0));
        assert_eq!(format_length(1800.0, Unit::Metre), "1.8");
    }

    #[test]
    fn both_decimal_separators_are_accepted() {
        assert_eq!(parse_number("1,8"), Some(1.8));
        assert_eq!(parse_number("1.8"), Some(1.8));
        assert_eq!(parse_number(" 12 mm "), Some(12.0));
        assert_eq!(parse_number("-0,5"), Some(-0.5));
    }

    #[test]
    fn garbage_is_rejected_rather_than_guessed() {
        // Acceptance criterion 14: the caller restores the previous value.
        for bad in ["", "  ", "abc", "1.2.3", "1,2,3", "--4", "NaN", "inf", "12mmm"] {
            assert_eq!(parse_number(bad), None, "{bad:?} should not parse");
        }
    }

    #[test]
    fn a_field_reads_an_expression() {
        // The design's own examples.
        assert_eq!(parse_number("40/3"), Some(40.0 / 3.0));
        assert_eq!(parse_number("12+8"), Some(20.0));
        assert_eq!(parse_number("(2+3)*4"), Some(20.0));
        assert_eq!(parse_number("100 - 2*15"), Some(70.0));
        assert_eq!(parse_number("-3*4"), Some(-12.0));
    }

    #[test]
    fn an_expression_that_does_not_resolve_to_a_number_is_refused() {
        // Division by zero must not leave a field holding infinity: there is no
        // shape that could be drawn from it.
        for bad in ["1/0", "5*", "(1+2", "1+2)", "()", "+", "*3", "2 3"] {
            assert_eq!(parse_number(bad), None, "{bad:?} should not parse");
        }
    }

    #[test]
    fn a_value_may_be_typed_in_another_unit_than_the_document_shows() {
        // "4 cm" in a millimetre document is 40 of what the field shows.
        assert_eq!(parse_entry("4 cm", Unit::Millimetre).unwrap().value, 40.0);
        assert_eq!(parse_entry("4cm", Unit::Millimetre).unwrap().value, 40.0);
        assert_eq!(parse_entry("4 cm", Unit::Metre).unwrap().value, 0.04);
        assert_eq!(parse_entry("1 m", Unit::Centimetre).unwrap().value, 100.0);
        // And it stores the same millimetres either way round.
        assert_eq!(parse_length("4cm", Unit::Millimetre), Some(40.0));
        assert_eq!(parse_length("4cm", Unit::Metre), Some(40.0));
        // A suffix inside an expression converts that term alone.
        assert_eq!(parse_entry("4cm + 5", Unit::Millimetre).unwrap().value, 45.0);
    }

    #[test]
    fn a_leading_plus_is_a_delta_and_a_bare_minus_is_not() {
        // `+2` adjusts; `-5` still has to mean minus five, because a position
        // field must be able to hold one.
        assert_eq!(parse_entry("+2", Unit::Millimetre), Some(Entry { value: 2.0, relative: true }));
        assert_eq!(parse_entry("+= 2", Unit::Millimetre), Some(Entry { value: 2.0, relative: true }));
        assert_eq!(parse_entry("-5", Unit::Millimetre), Some(Entry { value: -5.0, relative: false }));
        assert_eq!(parse_entry("- 5", Unit::Millimetre), Some(Entry { value: -5.0, relative: true }));
        assert_eq!(parse_entry("-= 5", Unit::Millimetre), Some(Entry { value: -5.0, relative: true }));
        // A delta is an expression too, and carries its own unit.
        assert_eq!(parse_entry("+2*3", Unit::Millimetre).unwrap().value, 6.0);
        assert_eq!(parse_entry("+1cm", Unit::Millimetre).unwrap().value, 10.0);
        // An absolute entry refuses to be read as a relative one.
        assert_eq!(parse_length("+2", Unit::Millimetre), None);
    }

    #[test]
    fn a_delta_resolves_against_whatever_the_field_holds() {
        let entry = parse_entry("+2", Unit::Millimetre).unwrap();
        // The same typed text gives a different answer per node, which is what
        // makes it work across a multi-selection.
        assert_eq!(entry.resolve(10.0), 12.0);
        assert_eq!(entry.resolve(40.0), 42.0);
        let absolute = parse_entry("2", Unit::Millimetre).unwrap();
        assert_eq!(absolute.resolve(10.0), 2.0);
        assert_eq!(absolute.resolve(40.0), 2.0);
    }

    #[test]
    fn no_floating_point_noise_in_output() {
        assert_eq!(format_number(0.1 + 0.2, 4), "0.3");
        assert_eq!(format_number(1.7999999999, 4), "1.8");
        assert_eq!(format_number(-0.00001, 4), "0");
        assert_eq!(format_number(5.0, 4), "5");
    }
}
