//! Conservative HTML sanitizer for email-derived HTML.
//!
//! Two consumers:
//! - The viewer route, which embeds the result in a sandboxed `<iframe srcdoc>`.
//!   The iframe `sandbox=""` (no `allow-scripts`, no `allow-same-origin`)
//!   is the primary defense; this pass just strips content that would
//!   otherwise leak as visible text (`<script>` source) or render
//!   phishing-shaped UI (`<form>`/`<input>`).
//! - The screenshot path, which sends the HTML to Cloudflare Browser
//!   Rendering. There is no iframe sandbox there, so this pass also
//!   neutralises JS execution vectors (`on*=` handlers, `javascript:` URLs)
//!   to keep the rendered page deterministic.
//!
//! This is not a full HTML parser. It is a pragmatic scrubber paired with
//! a strict iframe sandbox.
//!
//! # Why bytes, not chars
//!
//! Every scan here is over raw bytes so it stays linear in the input size —
//! inlined emails can carry several megabytes of base64 image data, and a
//! per-`char` copy made this path slow enough to matter. All of the markers
//! it looks for (`<`, `>`, quotes, spaces, `javascript:`) are ASCII, and every
//! cut lands on one of them, so the surviving bytes are always a subsequence
//! of valid UTF-8 cut at char boundaries. `from_utf8_lossy` at the end is a
//! belt-and-braces guard, not a routine repair.

/// Remove dangerous-or-disallowed constructs from `input` and return the
/// cleaned HTML. Idempotent.
pub fn sanitize_email_html(input: &str) -> String {
    let mut s = strip_block_tags(input.as_bytes(), &["script", "noscript", "style"]);
    s = strip_pair_tags(
        &s,
        &[
            "iframe", "object", "embed", "frame", "frameset", "applet", "form",
        ],
    );
    s = strip_void_tags(&s, &["meta", "link", "base", "input", "button"]);
    s = strip_event_handler_attrs(&s);
    s = neutralise_javascript_urls(&s);
    String::from_utf8_lossy(&s).into_owned()
}

/// Strip whole `<tag>…</tag>` blocks including their content, for tags
/// whose content is non-renderable text (`<script>`, `<style>`, `<noscript>`).
fn strip_block_tags(input: &[u8], tags: &[&str]) -> Vec<u8> {
    let mut s = input.to_vec();
    for tag in tags {
        s = strip_block(&s, tag);
    }
    s
}

fn strip_block(input: &[u8], tag: &str) -> Vec<u8> {
    let mut out = Vec::with_capacity(input.len());
    let lower = input.to_ascii_lowercase();
    let open_bytes = format!("<{tag}").into_bytes();
    let close_bytes = format!("</{tag}").into_bytes();
    let mut i = 0;
    while i < input.len() {
        if starts_with_at(&lower, i, &open_bytes)
            && i + open_bytes.len() < input.len()
            && is_tag_break(input[i + open_bytes.len()])
        {
            // Skip past the opening tag's `>`.
            match find_byte(input, b'>', i) {
                Some(open_end) => {
                    // Find the matching close tag.
                    let after_open = open_end + 1;
                    let close_pos = find_subsequence(&lower, &close_bytes, after_open);
                    match close_pos {
                        Some(cp) => match find_byte(input, b'>', cp) {
                            Some(close_end) => {
                                i = close_end + 1;
                                continue;
                            }
                            None => return out,
                        },
                        None => {
                            // No closing tag; drop the rest.
                            return out;
                        }
                    }
                }
                None => return out,
            }
        }
        out.push(input[i]);
        i += 1;
    }
    out
}

/// Strip both the opening `<tag …>` and the closing `</tag>` of the named
/// tags but keep their text content (used for wrapper tags whose content is
/// still useful prose, e.g. `<form>`).
fn strip_pair_tags(input: &[u8], tags: &[&str]) -> Vec<u8> {
    let mut s = input.to_vec();
    for tag in tags {
        s = drop_tag(&s, tag, true);
    }
    s
}

/// Strip the named void / single tags entirely (`<meta …>`, `<link …/>`).
fn strip_void_tags(input: &[u8], tags: &[&str]) -> Vec<u8> {
    let mut s = input.to_vec();
    for tag in tags {
        s = drop_tag(&s, tag, false);
    }
    s
}

/// Drop occurrences of `<tag …>` (and `</tag>` when `also_close`).
fn drop_tag(input: &[u8], tag: &str, also_close: bool) -> Vec<u8> {
    let mut out = Vec::with_capacity(input.len());
    let lower = input.to_ascii_lowercase();
    let open_bytes = format!("<{tag}").into_bytes();
    let close_bytes = format!("</{tag}").into_bytes();
    let mut i = 0;
    while i < input.len() {
        let open_match = starts_with_at(&lower, i, &open_bytes)
            && i + open_bytes.len() < input.len()
            && is_tag_break(input[i + open_bytes.len()]);
        let close_match = also_close
            && starts_with_at(&lower, i, &close_bytes)
            && i + close_bytes.len() < input.len()
            && is_tag_break(input[i + close_bytes.len()]);
        if open_match || close_match {
            match find_byte(input, b'>', i) {
                Some(end) => {
                    i = end + 1;
                    continue;
                }
                None => return out,
            }
        }
        out.push(input[i]);
        i += 1;
    }
    out
}

/// Remove `on…="…"` (and `on…='…'`, `on…=value`) attributes from every tag.
fn strip_event_handler_attrs(input: &[u8]) -> Vec<u8> {
    rewrite_tags(input, strip_event_handlers_in_tag)
}

/// For each `<…>` tag in `input`, run `f` on the tag including the angle
/// brackets and substitute its return value. Text content between tags is
/// passed through unchanged.
fn rewrite_tags<F: Fn(&[u8]) -> Vec<u8>>(input: &[u8], f: F) -> Vec<u8> {
    let mut out = Vec::with_capacity(input.len());
    let mut i = 0;
    while i < input.len() {
        if input[i] == b'<' {
            match find_byte(input, b'>', i) {
                Some(end) => {
                    out.extend_from_slice(&f(&input[i..=end]));
                    i = end + 1;
                    continue;
                }
                None => {
                    out.extend_from_slice(&input[i..]);
                    return out;
                }
            }
        }
        out.push(input[i]);
        i += 1;
    }
    out
}

fn strip_event_handlers_in_tag(tag: &[u8]) -> Vec<u8> {
    // Walk the tag interior, dropping any `on<word>=…` attribute.
    let lower = tag.to_ascii_lowercase();
    let mut out = Vec::with_capacity(tag.len());
    let mut i = 0;
    while i < tag.len() {
        if i + 2 < tag.len()
            && (tag[i] == b' ' || tag[i] == b'\t' || tag[i] == b'\n' || tag[i] == b'\r')
            && lower[i + 1] == b'o'
            && lower[i + 2] == b'n'
        {
            // Look ahead: characters until `=` or whitespace must be alphanumeric
            // (an event-handler attribute name like onclick / onmouseover).
            let mut j = i + 3;
            while j < tag.len() && tag[j].is_ascii_alphabetic() {
                j += 1;
            }
            if j < tag.len() && tag[j] == b'=' {
                // Skip the value (quoted or unquoted).
                let mut k = j + 1;
                if k < tag.len() && (tag[k] == b'"' || tag[k] == b'\'') {
                    let quote = tag[k];
                    k += 1;
                    while k < tag.len() && tag[k] != quote {
                        k += 1;
                    }
                    if k < tag.len() {
                        k += 1;
                    }
                } else {
                    while k < tag.len()
                        && tag[k] != b' '
                        && tag[k] != b'\t'
                        && tag[k] != b'\n'
                        && tag[k] != b'\r'
                        && tag[k] != b'>'
                        && tag[k] != b'/'
                    {
                        k += 1;
                    }
                }
                i = k;
                continue;
            }
        }
        out.push(tag[i]);
        i += 1;
    }
    out
}

fn neutralise_javascript_urls(input: &[u8]) -> Vec<u8> {
    rewrite_tags(input, neutralise_in_tag)
}

fn neutralise_in_tag(tag: &[u8]) -> Vec<u8> {
    // Replace `="javascript:…"` (and `=javascript:…`) with `="#"`.
    let lower = tag.to_ascii_lowercase();
    if !contains(&lower, b"javascript:") {
        return tag.to_vec();
    }
    let needle = b"javascript:";
    let mut out = Vec::with_capacity(tag.len());
    let mut i = 0;
    while i < tag.len() {
        if starts_with_at(&lower, i, needle) {
            out.extend_from_slice(b"about:blank");
            i += needle.len();
            // Skip to the next quote / whitespace / `>` so the URL value is
            // truncated, not just prefix-replaced.
            while i < tag.len()
                && tag[i] != b'"'
                && tag[i] != b'\''
                && tag[i] != b' '
                && tag[i] != b'\t'
                && tag[i] != b'\n'
                && tag[i] != b'\r'
                && tag[i] != b'>'
            {
                i += 1;
            }
            continue;
        }
        out.push(tag[i]);
        i += 1;
    }
    out
}

fn starts_with_at(haystack: &[u8], at: usize, needle: &[u8]) -> bool {
    haystack.len() >= at + needle.len() && &haystack[at..at + needle.len()] == needle
}

fn contains(haystack: &[u8], needle: &[u8]) -> bool {
    !needle.is_empty() && find_subsequence(haystack, needle, 0).is_some()
}

fn find_byte(haystack: &[u8], needle: u8, from: usize) -> Option<usize> {
    haystack
        .get(from..)?
        .iter()
        .position(|&b| b == needle)
        .map(|p| from + p)
}

fn find_subsequence(haystack: &[u8], needle: &[u8], from: usize) -> Option<usize> {
    if needle.is_empty() || from >= haystack.len() {
        return None;
    }
    haystack
        .get(from..)?
        .windows(needle.len())
        .position(|w| w == needle)
        .map(|p| from + p)
}

fn is_tag_break(b: u8) -> bool {
    matches!(b, b' ' | b'\t' | b'\n' | b'\r' | b'>' | b'/')
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn strips_script_block_with_payload() {
        let html = "<p>before</p><script>alert(1)</script><p>after</p>";
        let s = sanitize_email_html(html);
        assert_eq!(s, "<p>before</p><p>after</p>");
    }

    #[test]
    fn strips_style_block() {
        let html = "<p>x</p><style>body{color:red}</style><p>y</p>";
        let s = sanitize_email_html(html);
        assert!(!s.contains("color:red"));
        assert!(s.contains("<p>x</p>"));
    }

    #[test]
    fn strips_noscript_block() {
        let html = "<p>a</p><NoScript>fallback<script>x()</script></NoScript><p>b</p>";
        let s = sanitize_email_html(html);
        assert!(!s.contains("fallback"));
        assert!(s.contains("<p>a</p>"));
        assert!(s.contains("<p>b</p>"));
    }

    #[test]
    fn drops_iframe_pair() {
        let html = "<p>a</p><iframe src=\"https://evil\">b</iframe><p>c</p>";
        let s = sanitize_email_html(html);
        assert!(!s.contains("<iframe"));
        assert!(!s.contains("</iframe>"));
        // Inner text is preserved when stripping pair tags.
        assert!(s.contains("b"));
        assert!(s.contains("<p>c</p>"));
    }

    #[test]
    fn drops_form_input_button() {
        let html = "<form action=\"/x\"><input name=\"u\"/><button>Go</button></form>";
        let s = sanitize_email_html(html);
        assert!(!s.contains("<form"));
        assert!(!s.contains("</form>"));
        assert!(!s.contains("<input"));
        assert!(!s.contains("<button"));
    }

    #[test]
    fn strips_meta_link_base() {
        let html = "<meta http-equiv=\"refresh\" content=\"0;url=http://x\"><link rel=\"stylesheet\" href=\"/x\"><base href=\"/y\"><p>ok</p>";
        let s = sanitize_email_html(html);
        assert!(!s.contains("<meta"));
        assert!(!s.contains("<link"));
        assert!(!s.contains("<base"));
        assert!(s.contains("<p>ok</p>"));
    }

    #[test]
    fn strips_event_handler_attributes() {
        let html = "<img src=\"/a.png\" onerror=\"alert(1)\" onload='do()' alt=\"x\">";
        let s = sanitize_email_html(html);
        assert!(!s.to_ascii_lowercase().contains("onerror"));
        assert!(!s.to_ascii_lowercase().contains("onload"));
        assert!(s.contains("src=\"/a.png\""));
        assert!(s.contains("alt=\"x\""));
    }

    #[test]
    fn neutralises_javascript_href() {
        let html = "<a href=\"javascript:alert(1)\">x</a>";
        let s = sanitize_email_html(html);
        assert!(!s.to_ascii_lowercase().contains("javascript:"));
        assert!(s.contains("about:blank"));
    }

    #[test]
    fn keeps_safe_anchor_and_image() {
        let html =
            "<p><a href=\"https://example.com\">ok</a> <img src=\"data:image/png;base64,AAA\"></p>";
        let s = sanitize_email_html(html);
        assert!(s.contains("href=\"https://example.com\""));
        assert!(s.contains("data:image/png;base64,AAA"));
    }

    #[test]
    fn idempotent_on_clean_input() {
        let html = "<p>plain text</p>";
        let once = sanitize_email_html(html);
        let twice = sanitize_email_html(&once);
        assert_eq!(once, twice);
    }

    #[test]
    fn case_insensitive_script_match() {
        let html = "<SCRIPT>alert(1)</SCRIPT>";
        assert_eq!(sanitize_email_html(html), "");
    }

    #[test]
    fn unclosed_script_block_drops_remainder() {
        let html = "<p>before</p><script>never closes";
        let s = sanitize_email_html(html);
        assert_eq!(s, "<p>before</p>");
    }

    #[test]
    fn preserves_multibyte_text_and_attributes() {
        // Regression: the old char-by-byte copy mangled every non-ASCII byte
        // into mojibake. UTF-8 must survive untouched.
        let html = "<p>café ☕ — ₹500</p><img alt=\"naïve\" src=\"/x.png\">";
        let s = sanitize_email_html(html);
        assert_eq!(s, html);
    }

    #[test]
    fn handles_multibyte_around_a_stripped_tag() {
        let html = "<p>naïve ☕</p><script>bad</script><p>café</p>";
        let s = sanitize_email_html(html);
        assert_eq!(s, "<p>naïve ☕</p><p>café</p>");
    }
}
