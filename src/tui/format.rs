//! Format helpers ported from `web/src/components/content.ts` and friends.
//!
//! - [`fmt_ns`] — adaptive duration formatter.
//! - [`fmt_clock`] — wall-clock `HH:MM:SS` from a unix-ns timestamp.
//! - [`fmt_relative`] — "5s ago" / "3m ago" / "just now".
//! - [`pretty_json`] — `serde_json::to_string_pretty` with a safe fallback.
//! - [`hash_color`] — FNV-1a → HSL → terminal [`ratatui::style::Color::Rgb`].

use chrono::{DateTime, Utc};
use ratatui::style::Color;
use serde_json::Value;

/// Adaptive nanosecond formatter. Mirrors `fmtNs` in the web client.
pub fn fmt_ns(ns: Option<i128>) -> String {
    let Some(ns) = ns else {
        return "—".to_string();
    };
    if ns < 0 {
        return "—".to_string();
    }
    let ns = ns as f64;
    if ns < 1_000.0 {
        format!("{ns:.0}ns")
    } else if ns < 1_000_000.0 {
        format!("{:.1}µs", ns / 1_000.0)
    } else if ns < 1_000_000_000.0 {
        format!("{:.1}ms", ns / 1_000_000.0)
    } else if ns < 60.0 * 1_000_000_000.0 {
        format!("{:.2}s", ns / 1_000_000_000.0)
    } else {
        let secs = ns / 1_000_000_000.0;
        let m = (secs / 60.0).floor();
        let s = secs - m * 60.0;
        format!("{m:.0}m{s:.0}s")
    }
}

/// `HH:MM:SS` in local timezone from a unix nanosecond timestamp.
pub fn fmt_clock(unix_ns: Option<i128>) -> String {
    let Some(ns) = unix_ns else {
        return "--:--:--".to_string();
    };
    let secs = (ns / 1_000_000_000) as i64;
    let nsec = (ns % 1_000_000_000) as u32;
    let Some(dt) = DateTime::<Utc>::from_timestamp(secs, nsec) else {
        return "--:--:--".to_string();
    };
    dt.with_timezone(&chrono::Local)
        .format("%H:%M:%S")
        .to_string()
}

/// "5s ago" / "3m ago" / "2h ago" / "just now". `now_ns` is a clock injection
/// hook for tests; pass `None` to use the wall clock.
pub fn fmt_relative(unix_ns: Option<i128>, now_ns: Option<i128>) -> String {
    let Some(ns) = unix_ns else {
        return "—".to_string();
    };
    let now = now_ns.unwrap_or_else(|| Utc::now().timestamp_nanos_opt().unwrap_or(0) as i128);
    let delta = (now - ns).max(0);
    let secs = (delta / 1_000_000_000) as i64;
    if secs < 5 {
        "just now".to_string()
    } else if secs < 60 {
        format!("{secs}s ago")
    } else if secs < 3600 {
        format!("{}m ago", secs / 60)
    } else if secs < 86400 {
        format!("{}h ago", secs / 3600)
    } else {
        format!("{}d ago", secs / 86400)
    }
}

/// Pretty-print a `serde_json::Value`. Falls back to `{:?}` if formatting fails
/// for any reason (it should not for `Value`, but parity with `prettyJson`).
pub fn pretty_json(value: &Value) -> String {
    serde_json::to_string_pretty(value).unwrap_or_else(|_| format!("{value:?}"))
}

/// FNV-1a 32-bit hash. Offset basis `0x811c9dc5`, prime `0x01000193`.
pub fn fnv1a_32(s: &str) -> u32 {
    let mut h: u32 = 0x811c_9dc5;
    for b in s.as_bytes() {
        h ^= *b as u32;
        h = h.wrapping_mul(0x0100_0193);
    }
    h
}

/// Stable per-string colour. Mirrors webui `hashColor()` (FNV-1a → hue % 360,
/// rendered at S=65%, L=68%). Returned as a 24-bit RGB terminal colour so
/// modern terminal emulators preserve hue parity with the web side.
pub fn hash_color(s: &str) -> Color {
    let hue = (fnv1a_32(s) % 360) as f64;
    let (r, g, b) = hsl_to_rgb(hue, 0.65, 0.68);
    Color::Rgb(r, g, b)
}

/// HSL (S/L in [0,1], H in degrees) → 8-bit RGB. Standard formula.
fn hsl_to_rgb(h: f64, s: f64, l: f64) -> (u8, u8, u8) {
    let c = (1.0 - (2.0 * l - 1.0).abs()) * s;
    let h_ = h / 60.0;
    let x = c * (1.0 - (h_ % 2.0 - 1.0).abs());
    let (r1, g1, b1) = match h_ as i32 {
        0 => (c, x, 0.0),
        1 => (x, c, 0.0),
        2 => (0.0, c, x),
        3 => (0.0, x, c),
        4 => (x, 0.0, c),
        _ => (c, 0.0, x),
    };
    let m = l - c / 2.0;
    let to_u8 = |v: f64| ((v + m).clamp(0.0, 1.0) * 255.0).round() as u8;
    (to_u8(r1), to_u8(g1), to_u8(b1))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fmt_ns_units() {
        assert_eq!(fmt_ns(None), "—");
        assert_eq!(fmt_ns(Some(-1)), "—");
        assert_eq!(fmt_ns(Some(500)), "500ns");
        assert_eq!(fmt_ns(Some(1_500)), "1.5µs");
        assert_eq!(fmt_ns(Some(2_500_000)), "2.5ms");
        assert_eq!(fmt_ns(Some(1_500_000_000)), "1.50s");
        assert!(fmt_ns(Some(125_000_000_000)).starts_with("2m"));
    }

    #[test]
    fn fmt_relative_buckets() {
        let now: i128 = 1_700_000_000_000_000_000;
        assert_eq!(fmt_relative(Some(now), Some(now)), "just now");
        assert_eq!(
            fmt_relative(Some(now - 10 * 1_000_000_000), Some(now)),
            "10s ago"
        );
        assert_eq!(
            fmt_relative(Some(now - 90 * 1_000_000_000), Some(now)),
            "1m ago"
        );
        assert_eq!(
            fmt_relative(Some(now - 7200 * 1_000_000_000), Some(now)),
            "2h ago"
        );
        assert_eq!(fmt_relative(None, Some(now)), "—");
    }

    /// FNV-1a is *the* hash; we lock the offset+prime exactly, so a regression
    /// in either constant would change the hue across reloads.
    #[test]
    fn fnv1a_known_vectors() {
        assert_eq!(fnv1a_32(""), 0x811c_9dc5);
        // "foobar" → 0xbf9cf968 per FNV-1a reference vectors.
        assert_eq!(fnv1a_32("foobar"), 0xbf9c_f968);
    }

    /// Two calls with the same input must produce the same RGB.
    #[test]
    fn hash_color_is_deterministic() {
        let a = hash_color("apply_patch");
        let b = hash_color("apply_patch");
        assert_eq!(a, b);
        // And different inputs *generally* differ — pick two we know hash apart.
        assert_ne!(hash_color("apply_patch"), hash_color("bash"));
    }

    #[test]
    fn pretty_json_round_trip() {
        let v = serde_json::json!({"a": 1, "b": [2, 3]});
        let s = pretty_json(&v);
        assert!(s.contains("\"a\""));
        assert!(s.contains("\n"));
    }

    #[test]
    fn fmt_clock_handles_none() {
        assert_eq!(fmt_clock(None), "--:--:--");
        // Nonzero timestamp returns *something* of the right shape.
        let s = fmt_clock(Some(1_700_000_000_000_000_000));
        assert_eq!(s.len(), 8);
        assert_eq!(s.as_bytes()[2], b':');
    }
}
