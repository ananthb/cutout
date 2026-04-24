//! HTML templates for the management UI.

use std::collections::HashSet;

use crate::helpers::html_escape;
use crate::types::{Action, Rule};

const CSS: &str = r##"
:root {
  --bg: #fff; --fg: #1a1a1a; --muted: #666; --accent: #2563eb; --accent-hover: #1d4ed8;
  --code-bg: #f5f6f8; --border: #e0e0e0; --border-light: #f0f0f0;
  --ok: #16a34a; --warn: #dc2626;
  --r-sm: 6px; --r-md: 10px;
}
@media (prefers-color-scheme: dark) {
  :root {
    --bg: #121212; --fg: #e0e0e0; --muted: #999; --accent: #60a5fa; --accent-hover: #93bbfd;
    --code-bg: #1e1e1e; --border: #333; --border-light: #2a2a2a;
    --ok: #4ade80; --warn: #f87171;
  }
}
* { box-sizing: border-box; margin: 0; padding: 0; }
html, body { background: var(--bg); color: var(--fg); font-family: system-ui, -apple-system, sans-serif;
  font-size: 15px; line-height: 1.5; }
a { color: var(--accent); text-decoration: none; }
a:hover { text-decoration: underline; }
code { background: var(--code-bg); padding: 0.15rem 0.4rem; border-radius: 3px;
  font-family: ui-monospace, monospace; font-size: 0.85em; }
.container { max-width: 800px; margin: 0 auto; padding: 2rem 1.5rem; }
header { border-bottom: 1px solid var(--border); padding: 1rem 1.5rem; }
header .inner { max-width: 800px; margin: 0 auto; display: flex; align-items: center; justify-content: space-between; }
header h1 { font-size: 1.1rem; font-weight: 600; }
header .user { font-size: 0.85rem; color: var(--muted); }
h2 { font-size: 1.25rem; margin-bottom: 1rem; }
.btn { display: inline-flex; align-items: center; gap: 6px; padding: 8px 14px;
  border-radius: var(--r-sm); border: 1px solid var(--border);
  background: var(--bg); color: var(--fg); font-size: 0.85rem; cursor: pointer;
  transition: background 0.15s; }
.btn:hover { background: var(--code-bg); }
.btn.primary { background: var(--accent); color: #fff; border-color: var(--accent); }
.btn.primary:hover { background: var(--accent-hover); }
.btn.danger { color: var(--warn); }
.btn.danger:hover { background: #fef2f2; }
.btn.sm { padding: 4px 8px; font-size: 0.8rem; }
.btn:disabled { opacity: 0.4; cursor: not-allowed; }
.rule-list { border: 1px solid var(--border); border-radius: var(--r-md); overflow: hidden; }
.rule-row { display: grid; grid-template-columns: 40px 1fr 120px 1fr auto; gap: 12px;
  padding: 12px 16px; align-items: center; border-bottom: 1px solid var(--border-light); font-size: 0.9rem; }
.rule-row:last-child { border-bottom: none; }
.rule-row .order { color: var(--muted); font-size: 0.8rem; text-align: center; font-family: ui-monospace, monospace; }
.rule-row .pattern { font-family: ui-monospace, monospace; font-weight: 500; }
.rule-row .action-tag { display: inline-flex; padding: 2px 8px; border-radius: 999px;
  font-size: 0.75rem; font-weight: 500; }
.action-forward { background: #dbeafe; color: #1e40af; }
.action-drop { background: #fef2f2; color: #991b1b; }
@media (prefers-color-scheme: dark) {
  .action-forward { background: #1e3a5f; color: #93bbfd; }
  .action-drop { background: #3b1111; color: #f87171; }
  .btn.danger:hover { background: #3b1111; }
}
.rule-row .actions { display: flex; gap: 4px; }
.rule-head { display: grid; grid-template-columns: 40px 1fr 120px 1fr auto; gap: 12px;
  padding: 10px 16px; background: var(--code-bg); font-size: 0.75rem; font-weight: 600;
  text-transform: uppercase; letter-spacing: 0.05em; color: var(--muted); border-bottom: 1px solid var(--border); }
.catch-all { background: var(--code-bg); }
.form-card { border: 1px solid var(--border); border-radius: var(--r-md); padding: 20px; margin-top: 1.5rem; }
.form-group { margin-bottom: 1rem; }
.form-group label { display: block; margin-bottom: 4px; font-size: 0.85rem; font-weight: 500; }
.form-group input, .form-group select { width: 100%; padding: 8px 10px; border: 1px solid var(--border);
  border-radius: var(--r-sm); font-size: 0.9rem; background: var(--bg); color: var(--fg); }
.form-group input:focus, .form-group select:focus { outline: none; border-color: var(--accent);
  box-shadow: 0 0 0 3px rgba(37,99,235,0.15); }
.form-row { display: grid; grid-template-columns: 1fr 1fr; gap: 12px; }
.form-help { font-size: 0.8rem; color: var(--muted); margin-top: 4px; }
.toast { padding: 12px; border-radius: var(--r-sm); margin-bottom: 1rem; font-size: 0.9rem; }
.toast.success { background: #dcfce7; color: #166534; }
.toast.error { background: #fef2f2; color: #991b1b; }
@media (prefers-color-scheme: dark) {
  .toast.success { background: #14532d; color: #bbf7d0; }
  .toast.error { background: #450a0a; color: #fecaca; }
}
.htmx-request .btn { opacity: 0.6; pointer-events: none; }
@media (max-width: 640px) {
  .rule-row, .rule-head { grid-template-columns: 1fr; gap: 4px; }
  .form-row { grid-template-columns: 1fr; }
}
"##;

/// Base HTML wrapper.
pub fn base_html(title: &str, email: &str, content: &str) -> String {
    format!(
        r##"<!DOCTYPE html>
<html lang="en">
<head>
<meta charset="utf-8">
<meta name="viewport" content="width=device-width, initial-scale=1">
<title>{title} — Cutout</title>
<style>{css}</style>
<script src="https://unpkg.com/htmx.org@2.0.8/dist/htmx.min.js" crossorigin="anonymous"></script>
<script src="https://unpkg.com/htmx-ext-json-enc@2.0.3/json-enc.js" crossorigin="anonymous"></script>
</head>
<body>
<header><div class="inner"><h1>Cutout</h1><span class="user">{email}</span></div></header>
<div class="container">{content}</div>
</body>
</html>"##,
        title = html_escape(title),
        css = CSS,
        email = html_escape(email),
        content = content,
    )
}

/// Full rules management page.
pub fn rules_page(rules: &[Rule], email: &str, verified: &HashSet<String>) -> String {
    let rows: String = rules
        .iter()
        .enumerate()
        .map(|(i, rule)| rule_row(rule, i, verified))
        .collect();

    let content = format!(
        r#"<div style="display:flex;align-items:center;justify-content:space-between;margin-bottom:1rem">
  <h2 style="margin:0">Routing Rules</h2>
  <button class="btn primary" onclick="document.getElementById(&#39;add-form&#39;).style.display=&#39;block&#39;">Add Rule</button>
</div>
<p style="color:var(--muted);font-size:0.85rem;margin-bottom:1.5rem">
  Rules are evaluated top-to-bottom. The first match wins. The <code>*@*</code> catch-all is always last.
  Forwarding to a destination requires the address to be verified — check the inbox for a confirmation link.
</p>
<div id="toast"></div>
<div id="rules-list" class="rule-list">
  <div class="rule-head">
    <div>#</div><div>Label</div><div>Pattern</div><div>Action</div><div></div>
  </div>
  {rows}
</div>
{add_form}"#,
        rows = rows,
        add_form = add_rule_form(),
    );

    base_html("Rules", email, &content)
}

/// Render a single Forward destination with its verification status.
fn render_destination(email: &str, verified: &HashSet<String>) -> String {
    let esc = html_escape(email);
    if verified.contains(email) {
        format!(r#"<span style="color:var(--ok)" title="Verified">{esc} ✓</span>"#)
    } else {
        let vals_json = serde_json::json!({ "email": email }).to_string();
        let vals_attr = html_escape(&vals_json);
        format!(
            r##"<span style="color:var(--warn)" title="Not verified">{esc} <button class="btn sm" style="padding:1px 6px;font-size:0.7rem;margin-left:2px" hx-post="/manage/verify/resend" hx-vals='{vals_attr}' hx-target="#toast" hx-swap="innerHTML" hx-ext="json-enc">resend</button></span>"##
        )
    }
}

/// Render a toast message snippet (for HTMX swap into #toast).
pub fn toast(kind: &str, message: &str) -> String {
    format!(
        r#"<div class="toast {kind}">{message}</div>"#,
        kind = html_escape(kind),
        message = html_escape(message)
    )
}

/// Single rule row.
pub fn rule_row(rule: &Rule, index: usize, verified: &HashSet<String>) -> String {
    let pattern = format!(
        "{}@{}",
        html_escape(&rule.local_pattern),
        html_escape(&rule.domain_pattern)
    );

    let (action_class, action_label, action_detail) = match &rule.action {
        Action::Forward { destinations } => {
            let detail = destinations
                .iter()
                .map(|d| render_destination(d, verified))
                .collect::<Vec<_>>()
                .join(", ");
            ("action-forward", "Forward", detail)
        }
        Action::Drop => ("action-drop", "Drop", String::new()),
    };

    let is_catch_all = rule.is_catch_all();
    let row_class = if is_catch_all {
        "rule-row catch-all"
    } else {
        "rule-row"
    };

    let move_buttons = if is_catch_all {
        String::new()
    } else {
        let id_escaped = html_escape(&rule.id);
        let up_vals = format!(r#"{{"id":"{}","direction":"up"}}"#, id_escaped);
        let down_vals = format!(r#"{{"id":"{}","direction":"down"}}"#, id_escaped);
        format!(
            "<button class=\"btn sm\" hx-post=\"/manage/rules/reorder\" hx-vals='{}' hx-target=\"#rules-list\" hx-swap=\"innerHTML\" hx-ext=\"json-enc\">&uarr;</button>\
             <button class=\"btn sm\" hx-post=\"/manage/rules/reorder\" hx-vals='{}' hx-target=\"#rules-list\" hx-swap=\"innerHTML\" hx-ext=\"json-enc\">&darr;</button>",
            up_vals, down_vals,
        )
    };

    let delete_button = if is_catch_all {
        String::new()
    } else {
        format!(
            "<button class=\"btn sm danger\" hx-delete=\"/manage/rules/{}\" hx-target=\"#rules-list\" hx-swap=\"innerHTML\" hx-confirm=\"Delete this rule?\">Del</button>",
            html_escape(&rule.id),
        )
    };

    let edit_button = format!(
        "<button class=\"btn sm\" hx-get=\"/manage/rules/{}/edit\" hx-target=\"#edit-area\" hx-swap=\"innerHTML\">Edit</button>",
        html_escape(&rule.id),
    );

    let action_html = if action_detail.is_empty() {
        format!(r#"<span class="action-tag {action_class}">{action_label}</span>"#)
    } else {
        format!(
            r#"<span class="action-tag {action_class}">{action_label}</span> <span style="font-size:0.8rem">{action_detail}</span>"#,
        )
    };

    format!(
        r#"<div class="{row_class}">
  <div class="order">{order}</div>
  <div>{label}</div>
  <div class="pattern"><code>{pattern}</code></div>
  <div>{action_html}</div>
  <div class="actions">{move_buttons}{edit_button}{delete_button}</div>
</div>"#,
        row_class = row_class,
        order = index + 1,
        label = html_escape(&rule.label),
        pattern = pattern,
        action_html = action_html,
        move_buttons = move_buttons,
        edit_button = edit_button,
        delete_button = delete_button,
    )
}

/// Rules list partial (just the rows, for HTMX swap).
pub fn rules_list_partial(rules: &[Rule], verified: &HashSet<String>) -> String {
    let rows: String = rules
        .iter()
        .enumerate()
        .map(|(i, rule)| rule_row(rule, i, verified))
        .collect();

    format!(
        r#"<div class="rule-head">
  <div>#</div><div>Label</div><div>Pattern</div><div>Action</div><div></div>
</div>
{rows}
<div id="edit-area" hx-swap-oob="true"></div>"#,
    )
}

/// Add rule form (hidden by default).
fn add_rule_form() -> String {
    let mut s = String::new();
    s.push_str(r#"<div id="add-form" class="form-card" style="display:none">"#);
    s.push_str(r#"<h3 style="margin-bottom:1rem">Add Rule</h3>"#);
    s.push_str(r##"<form hx-post="/manage/rules" hx-target="#rules-list" hx-swap="innerHTML" hx-ext="json-enc">"##);
    s.push_str(r#"<div class="form-group"><label for="label">Label</label>"#);
    s.push_str(r#"<input id="label" name="label" type="text" placeholder="e.g. Newsletter drop" required></div>"#);
    s.push_str(r#"<div class="form-row"><div class="form-group">"#);
    s.push_str(r#"<label for="local_pattern">Local part pattern</label>"#);
    s.push_str(r#"<input id="local_pattern" name="local_pattern" type="text" placeholder="*" value="*" required>"#);
    s.push_str(
        r#"<div class="form-help">The part before @. Use <code>*</code> for any.</div></div>"#,
    );
    s.push_str(r#"<div class="form-group"><label for="domain_pattern">Domain pattern</label>"#);
    s.push_str(r#"<input id="domain_pattern" name="domain_pattern" type="text" placeholder="*" value="*" required>"#);
    s.push_str(
        r#"<div class="form-help">The part after @. Use <code>*</code> for any.</div></div></div>"#,
    );
    s.push_str(r#"<div class="form-group"><label for="action_type">Action</label>"#);
    s.push_str(r#"<select id="action_type" name="action_type" "#);
    s.push_str(r#"onchange="document.getElementById(&#39;dest-group&#39;).style.display=this.value===&#39;forward&#39;?&#39;block&#39;:&#39;none&#39;">"#);
    s.push_str(r#"<option value="forward">Forward</option><option value="drop">Drop</option></select></div>"#);
    s.push_str(r#"<div class="form-group" id="dest-group"><label for="destinations">Forward to (comma-separated)</label>"#);
    s.push_str(r#"<input id="destinations" name="destinations" type="text" placeholder="me@example.com, backup@example.com"></div>"#);
    s.push_str(r#"<div style="display:flex;gap:8px">"#);
    s.push_str(r#"<button type="submit" class="btn primary">Add Rule</button>"#);
    s.push_str(r#"<button type="button" class="btn" onclick="document.getElementById(&#39;add-form&#39;).style.display=&#39;none&#39;">Cancel</button>"#);
    s.push_str(r#"</div></form></div><div id="edit-area"></div>"#);
    s
}

/// Edit rule form partial (returned for HTMX swap into #edit-area).
pub fn edit_rule_form(rule: &Rule) -> String {
    let (action_type, destinations) = match &rule.action {
        Action::Forward { destinations } => ("forward", destinations.join(", ")),
        Action::Drop => ("drop", String::new()),
    };
    let dest_display = if action_type == "forward" {
        "block"
    } else {
        "none"
    };
    let forward_selected = if action_type == "forward" {
        " selected"
    } else {
        ""
    };
    let drop_selected = if action_type == "drop" {
        " selected"
    } else {
        ""
    };

    format!(
        "<div class=\"form-card\">\
<h3 style=\"margin-bottom:1rem\">Edit Rule</h3>\
<form hx-put=\"/manage/rules/{id}\" hx-target=\"#rules-list\" hx-swap=\"innerHTML\" hx-ext=\"json-enc\">\
<div class=\"form-group\"><label for=\"edit-label\">Label</label>\
<input id=\"edit-label\" name=\"label\" type=\"text\" value=\"{label}\" required></div>\
<div class=\"form-row\"><div class=\"form-group\">\
<label for=\"edit-local\">Local part pattern</label>\
<input id=\"edit-local\" name=\"local_pattern\" type=\"text\" value=\"{local}\" required></div>\
<div class=\"form-group\"><label for=\"edit-domain\">Domain pattern</label>\
<input id=\"edit-domain\" name=\"domain_pattern\" type=\"text\" value=\"{domain}\" required></div></div>\
<div class=\"form-group\"><label for=\"edit-action\">Action</label>\
<select id=\"edit-action\" name=\"action_type\" onchange=\"document.getElementById(&#39;edit-dest-group&#39;).style.display=this.value===&#39;forward&#39;?&#39;block&#39;:&#39;none&#39;\">\
<option value=\"forward\"{forward_selected}>Forward</option>\
<option value=\"drop\"{drop_selected}>Drop</option></select></div>\
<div class=\"form-group\" id=\"edit-dest-group\" style=\"display:{dest_display}\">\
<label for=\"edit-destinations\">Forward to (comma-separated)</label>\
<input id=\"edit-destinations\" name=\"destinations\" type=\"text\" value=\"{destinations}\"></div>\
<div style=\"display:flex;gap:8px\">\
<button type=\"submit\" class=\"btn primary\">Save</button>\
<button type=\"button\" class=\"btn\" onclick=\"document.getElementById(&#39;edit-area&#39;).innerHTML=&#39;&#39;\">Cancel</button>\
</div></form></div>",
        id = html_escape(&rule.id),
        label = html_escape(&rule.label),
        local = html_escape(&rule.local_pattern),
        domain = html_escape(&rule.domain_pattern),
        forward_selected = forward_selected,
        drop_selected = drop_selected,
        dest_display = dest_display,
        destinations = html_escape(&destinations),
    )
}
