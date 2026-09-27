//! Numbers as text, shared by the interface and the file writers.

/// `value` at `decimals` places without float noise (`1.8`, not `1.7999999999`): trailing zeros and
/// a bare point are dropped, and negative zero reads `0`.
pub fn trimmed(value: f64, decimals: usize) -> String {
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
