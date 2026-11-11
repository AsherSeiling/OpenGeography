//! SVG text helpers. Output must be byte-identical on every platform
//! (docs/LAYOUTS_PLAN.md P3), so numbers are formatted by integer arithmetic
//! at a fixed precision, never through locale- or libm-dependent printing.

use std::fmt::Write;

/// Millimetres → string with at most 3 decimals (0.001 mm), trailing zeros
/// trimmed, no "-0".
pub fn mm(v: f64) -> String {
    let n = (v * 1000.0).round();
    if !n.is_finite() {
        return "0".into();
    }
    let n = n as i64;
    let (sign, a) = if n < 0 { ("-", -n) } else { ("", n) };
    let (whole, frac) = (a / 1000, a % 1000);
    if frac == 0 {
        return if a == 0 { "0".into() } else { format!("{sign}{whole}") };
    }
    let mut s = format!("{sign}{whole}.{frac:03}");
    while s.ends_with('0') {
        s.pop();
    }
    s
}

/// Escape text for element content and attribute values.
pub fn esc(s: &str) -> String {
    let mut o = String::with_capacity(s.len());
    for c in s.chars() {
        match c {
            '&' => o.push_str("&amp;"),
            '<' => o.push_str("&lt;"),
            '>' => o.push_str("&gt;"),
            '"' => o.push_str("&quot;"),
            '\'' => o.push_str("&apos;"),
            c if (c as u32) < 0x20 && c != '\n' && c != '\t' => {}
            c => o.push(c),
        }
    }
    o
}

/// A colour that is safe to put in an attribute: `#rgb`, `#rrggbb`,
/// `#rrggbbaa`, or a plain colour name. Anything else becomes `fallback`.
pub fn color(c: &str, fallback: &str) -> String {
    let t = c.trim();
    let hex = t.strip_prefix('#').map(|h| matches!(h.len(), 3 | 4 | 6 | 8) && h.chars().all(|c| c.is_ascii_hexdigit()));
    match hex {
        Some(true) => t.to_ascii_lowercase(),
        Some(false) => fallback.into(),
        None if !t.is_empty() && t.len() < 24 && t.chars().all(|c| c.is_ascii_alphabetic()) => t.to_ascii_lowercase(),
        None => fallback.into(),
    }
}

/// A colour split into its opaque part and its alpha 0..=1, so renderers write `fill-opacity` / `stroke-opacity`
/// (PNG through resvg and PDF through svg2pdf both honour those; neither is left to read 8-digit hex).
/// `#rrggbb`, `#rgb` and names have alpha 1; `#rrggbbaa` and `#rgba` carry theirs. Anything unsafe becomes `fallback` (alpha 1).
pub fn color_alpha(c: &str, fallback: &str) -> (String, f64) {
    let v = color(c, fallback);
    let Some(h) = v.strip_prefix('#') else { return (v, 1.0) };
    let hex = |s: &str| u8::from_str_radix(s, 16).unwrap_or(255);
    match h.len() {
        8 => (format!("#{}", &h[..6]), f64::from(hex(&h[6..])) / 255.0),
        4 => (format!("#{}", &h[..3]), f64::from(hex(&h[3..4]) * 17) / 255.0),
        _ => (v, 1.0),
    }
}

/// ` name="a"` for an alpha below 1, else nothing, so a fully opaque colour keeps byte-identical output.
pub fn alpha_attr(name: &str, a: f64) -> String {
    if a < 1.0 {
        format!(" {name}=\"{}\"", opacity(a))
    } else {
        String::new()
    }
}

/// Opacity 0..=1 as a short decimal.
pub fn opacity(v: f64) -> String {
    let n = (v.clamp(0.0, 1.0) * 1000.0).round() as i64;
    let mut s = format!("{}.{:03}", n / 1000, n % 1000);
    while s.ends_with('0') {
        s.pop();
    }
    if s.ends_with('.') {
        s.pop();
    }
    s
}

/// Append `M x y` / `L x y` style path commands.
pub fn cmd(out: &mut String, c: char, x: f64, y: f64) {
    let _ = write!(out, "{c}{} {}", mm(x), mm(y));
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn numbers_are_fixed_and_trimmed() {
        assert_eq!(mm(0.0), "0");
        assert_eq!(mm(-0.0004), "0");
        assert_eq!(mm(12.5), "12.5");
        assert_eq!(mm(12.0004), "12");
        assert_eq!(mm(-3.0126), "-3.013");
        assert_eq!(mm(f64::NAN), "0");
        assert_eq!(opacity(0.75), "0.75");
        assert_eq!(opacity(1.0), "1");
        assert_eq!(opacity(0.0), "0");
    }

    #[test]
    fn color_alpha_splits_hex_with_alpha() {
        assert_eq!(color_alpha("#FF0000", "#000"), ("#ff0000".to_string(), 1.0));
        let (c, a) = color_alpha("#ff000080", "#000");
        assert_eq!(c, "#ff0000");
        assert!((a - 128.0 / 255.0).abs() < 1e-9);
        let (c, a) = color_alpha("#f008", "#000");
        assert_eq!(c, "#f00");
        assert!((a - 136.0 / 255.0).abs() < 1e-9);
        assert_eq!(color_alpha("none", "#000"), ("none".to_string(), 1.0));
        assert_eq!(color_alpha("bad\"", "#123456"), ("#123456".to_string(), 1.0));
        assert_eq!(alpha_attr("stroke-opacity", 1.0), "");
        assert_eq!(alpha_attr("stroke-opacity", 0.5), " stroke-opacity=\"0.5\"");
    }

    #[test]
    fn colors_cannot_break_out_of_attributes() {
        assert_eq!(color("#7AA8D8", "#000"), "#7aa8d8");
        assert_eq!(color("red\" onload=\"x", "#000"), "#000");
        assert_eq!(color("", "#123456"), "#123456");
        assert_eq!(esc("a<b&\"c\""), "a&lt;b&amp;&quot;c&quot;");
    }
}
