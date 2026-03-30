/// Format f32 like Bevy: trim trailing zeros but keep at least one decimal.
/// e.g. 1.0, 0.5, 0.15, 0.333
pub(crate) fn fmt_f32(v: f32) -> String {
    let s = format!("{v:.4}");
    let s = s.trim_end_matches('0');
    if s.ends_with('.') {
        format!("{s}0")
    } else {
        s.to_string()
    }
}
