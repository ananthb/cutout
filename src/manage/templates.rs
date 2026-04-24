//! HTML templates for the management UI.

use crate::bots::EnabledChannels;
use crate::helpers::html_escape;
use crate::types::{Action, Destination, Rule};
use crate::validation::{Issue, Report};

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
.dest-tag { display: inline-block; padding: 1px 6px; border-radius: 4px;
  font-family: ui-monospace, monospace; font-size: 0.75rem; margin-right: 4px; }
.dest-email    { background: #dbeafe; color: #1e40af; }
.dest-telegram { background: #e0f2fe; color: #075985; }
.dest-discord  { background: #ede9fe; color: #5b21b6; }
@media (prefers-color-scheme: dark) {
  .dest-email    { background: #1e3a5f; color: #93bbfd; }
  .dest-telegram { background: #0c4a6e; color: #7dd3fc; }
  .dest-discord  { background: #3b2463; color: #c4b5fd; }
}
.issues { margin-top: 4px; display: flex; flex-direction: column; gap: 2px; }
.issue { font-size: 0.75rem; padding: 2px 6px; border-radius: 3px; }
.issue-err  { background: #fef2f2; color: #991b1b; }
.issue-warn { background: #fef3c7; color: #92400e; }
@media (prefers-color-scheme: dark) {
  .issue-err  { background: #3b1111; color: #f87171; }
  .issue-warn { background: #3a2a0a; color: #fbbf24; }
}
.form-group textarea { width: 100%; padding: 8px 10px; border: 1px solid var(--border);
  border-radius: var(--r-sm); background: var(--bg); color: var(--fg); }
.tester-result { border: 1px solid var(--border); border-radius: var(--r-md);
  padding: 1rem; margin-top: 1rem; }
.tester-result.matched { border-left: 3px solid var(--ok); }
.tester-result.nomatch { border-left: 3px solid var(--warn); }
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
pub fn rules_page(
    rules: &[Rule],
    email: &str,
    report: &Report,
    enabled: &EnabledChannels,
) -> String {
    let rows: String = rules
        .iter()
        .enumerate()
        .map(|(i, rule)| {
            rule_row(
                rule,
                i,
                report.issues.get(i).map(|v| v.as_slice()).unwrap_or(&[]),
            )
        })
        .collect();

    let nav = nav_links("rules");

    let content = format!(
        r#"{nav}
<div style="display:flex;align-items:center;justify-content:space-between;margin-bottom:1rem">
  <h2 style="margin:0">Routing Rules</h2>
  <button class="btn primary" onclick="document.getElementById(&#39;add-form&#39;).style.display=&#39;block&#39;">Add Rule</button>
</div>
<p style="color:var(--muted);font-size:0.85rem;margin-bottom:1.5rem">
  Rules are evaluated top-to-bottom. The first match wins. The <code>*@*</code> catch-all is always last.
  Email destinations must be verified in Cloudflare's
  <a href="https://dash.cloudflare.com/?to=/:account/email/routing/routes" target="_blank">Email Routing → Destination Addresses</a>
  list.
</p>
<div id="rules-list" class="rule-list">
  <div class="rule-head">
    <div>#</div><div>Label</div><div>Pattern</div><div>Action</div><div></div>
  </div>
  {rows}
</div>
{add_form}"#,
        nav = nav,
        rows = rows,
        add_form = add_rule_form(enabled),
    );

    base_html("Rules", email, &content)
}

fn render_destinations(destinations: &[Destination]) -> String {
    destinations
        .iter()
        .map(|d| {
            format!(
                r#"<span class="dest-tag dest-{kind}" title="{kind}">{kind}:{value}</span>"#,
                kind = d.kind_label(),
                value = html_escape(d.value()),
            )
        })
        .collect::<Vec<_>>()
        .join(" ")
}

/// Single rule row. `issues` is the validation report for this rule only.
pub fn rule_row(rule: &Rule, index: usize, issues: &[Issue]) -> String {
    let pattern = format!(
        "{}@{}",
        html_escape(&rule.local_pattern),
        html_escape(&rule.domain_pattern)
    );

    let (action_class, action_label, action_detail) = match &rule.action {
        Action::Forward { destinations } => (
            "action-forward",
            "Forward",
            render_destinations(destinations),
        ),
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

    let issues_html = if issues.is_empty() {
        String::new()
    } else {
        let lines: String = issues
            .iter()
            .map(|i| {
                let class = if i.is_error() { "err" } else { "warn" };
                format!(
                    r#"<div class="issue issue-{class}">{msg}</div>"#,
                    class = class,
                    msg = html_escape(i.message())
                )
            })
            .collect();
        format!(r#"<div class="issues">{lines}</div>"#)
    };

    format!(
        r#"<div class="{row_class}">
  <div class="order">{order}</div>
  <div>{label}{issues}</div>
  <div class="pattern"><code>{pattern}</code></div>
  <div>{action_html}</div>
  <div class="actions">{move_buttons}{edit_button}{delete_button}</div>
</div>"#,
        row_class = row_class,
        order = index + 1,
        label = html_escape(&rule.label),
        issues = issues_html,
        pattern = pattern,
        action_html = action_html,
        move_buttons = move_buttons,
        edit_button = edit_button,
        delete_button = delete_button,
    )
}

/// Rules list partial (just the rows, for HTMX swap).
pub fn rules_list_partial(rules: &[Rule], report: &Report) -> String {
    let rows: String = rules
        .iter()
        .enumerate()
        .map(|(i, rule)| {
            rule_row(
                rule,
                i,
                report.issues.get(i).map(|v| v.as_slice()).unwrap_or(&[]),
            )
        })
        .collect();

    format!(
        r#"<div class="rule-head">
  <div>#</div><div>Label</div><div>Pattern</div><div>Action</div><div></div>
</div>
{rows}
<div id="edit-area" hx-swap-oob="true"></div>"#,
    )
}

/// Navigation links at the top of every manage page.
fn nav_links(current: &str) -> String {
    fn link(href: &str, label: &str, active: bool) -> String {
        let style = if active {
            "color:var(--fg);font-weight:600"
        } else {
            "color:var(--muted)"
        };
        format!(r#"<a href="{href}" style="{style};margin-right:1rem">{label}</a>"#)
    }
    format!(
        r#"<nav style="margin-bottom:1.5rem;font-size:0.9rem">{rules}{test}</nav>"#,
        rules = link("/manage", "Rules", current == "rules"),
        test = link("/manage/test", "Rule tester", current == "test"),
    )
}

/// Produce the `kind:value` help and placeholder for the destinations field,
/// listing only channels whose bot secrets are configured.
fn destinations_help(enabled: &EnabledChannels) -> (String, String) {
    let mut kinds = vec!["email"];
    let mut lines = vec!["email:you@example.com"];
    if enabled.telegram {
        kinds.push("telegram");
        lines.push("telegram:-100123456789");
    }
    if enabled.discord {
        kinds.push("discord");
        lines.push("discord:987654321098765432");
    }
    let help_inner = kinds
        .iter()
        .map(|k| format!("<code>{k}</code>"))
        .collect::<Vec<_>>()
        .join(", ");
    let help = format!("One <code>kind:value</code> per line. Available kinds: {help_inner}.");
    let missing = match (enabled.telegram, enabled.discord) {
        (true, true) => String::new(),
        (false, true) => " (set <code>TELEGRAM_BOT_TOKEN</code> to enable telegram)".into(),
        (true, false) => " (set <code>DISCORD_BOT_TOKEN</code> + <code>DISCORD_APP_ID</code> + <code>DISCORD_PUBLIC_KEY</code> to enable discord)".into(),
        (false, false) => " (set the telegram / discord bot secrets to enable those kinds)".into(),
    };
    (format!("{help}{missing}"), lines.join("\n"))
}

/// Add rule form (hidden by default).
fn add_rule_form(enabled: &EnabledChannels) -> String {
    let (help, placeholder) = destinations_help(enabled);
    format!(
        r##"<div id="add-form" class="form-card" style="display:none">
  <h3 style="margin-bottom:1rem">Add Rule</h3>
  <form hx-post="/manage/rules" hx-target="#rules-list" hx-swap="innerHTML" hx-ext="json-enc">
    <div class="form-group">
      <label for="label">Label</label>
      <input id="label" name="label" type="text" placeholder="e.g. Newsletter drop" required>
    </div>
    <div class="form-row">
      <div class="form-group">
        <label for="local_pattern">Local part pattern</label>
        <input id="local_pattern" name="local_pattern" type="text" placeholder="*" value="*" required>
        <div class="form-help">The part before @. Use <code>*</code> for any.</div>
      </div>
      <div class="form-group">
        <label for="domain_pattern">Domain pattern</label>
        <input id="domain_pattern" name="domain_pattern" type="text" placeholder="*" value="*" required>
        <div class="form-help">The part after @. Use <code>*</code> for any.</div>
      </div>
    </div>
    <div class="form-group">
      <label for="action_type">Action</label>
      <select id="action_type" name="action_type" onchange="document.getElementById('dest-group').style.display=this.value==='forward'?'block':'none'">
        <option value="forward">Forward</option>
        <option value="drop">Drop</option>
      </select>
    </div>
    <div class="form-group" id="dest-group">
      <label for="destinations">Destinations (one per line)</label>
      <textarea id="destinations" name="destinations" rows="3" placeholder="{placeholder}" style="font-family:ui-monospace,monospace;font-size:0.85rem"></textarea>
      <div class="form-help">{help}</div>
    </div>
    <div style="display:flex;gap:8px">
      <button type="submit" class="btn primary">Add Rule</button>
      <button type="button" class="btn" onclick="document.getElementById('add-form').style.display='none'">Cancel</button>
    </div>
  </form>
</div>
<div id="edit-area"></div>"##,
        placeholder = html_escape(&placeholder),
        help = help,
    )
}

/// Edit rule form partial (returned for HTMX swap into #edit-area).
pub fn edit_rule_form(rule: &Rule) -> String {
    let (action_type, destinations) = match &rule.action {
        Action::Forward { destinations } => ("forward", Destination::format_list(destinations)),
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
        r##"<div class="form-card">
  <h3 style="margin-bottom:1rem">Edit Rule</h3>
  <form hx-put="/manage/rules/{id}" hx-target="#rules-list" hx-swap="innerHTML" hx-ext="json-enc">
    <div class="form-group">
      <label for="edit-label">Label</label>
      <input id="edit-label" name="label" type="text" value="{label}" required>
    </div>
    <div class="form-row">
      <div class="form-group">
        <label for="edit-local">Local part pattern</label>
        <input id="edit-local" name="local_pattern" type="text" value="{local}" required>
      </div>
      <div class="form-group">
        <label for="edit-domain">Domain pattern</label>
        <input id="edit-domain" name="domain_pattern" type="text" value="{domain}" required>
      </div>
    </div>
    <div class="form-group">
      <label for="edit-action">Action</label>
      <select id="edit-action" name="action_type" onchange="document.getElementById('edit-dest-group').style.display=this.value==='forward'?'block':'none'">
        <option value="forward"{forward_selected}>Forward</option>
        <option value="drop"{drop_selected}>Drop</option>
      </select>
    </div>
    <div class="form-group" id="edit-dest-group" style="display:{dest_display}">
      <label for="edit-destinations">Destinations (one per line)</label>
      <textarea id="edit-destinations" name="destinations" rows="4" style="font-family:ui-monospace,monospace;font-size:0.85rem">{destinations}</textarea>
    </div>
    <div style="display:flex;gap:8px">
      <button type="submit" class="btn primary">Save</button>
      <button type="button" class="btn" onclick="document.getElementById('edit-area').innerHTML=''">Cancel</button>
    </div>
  </form>
</div>"##,
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

/// Result of running the rule tester.
pub struct TesterResult {
    pub to: String,
    pub matched_index: Option<usize>,
}

/// Rule tester page.
pub fn tester_page(
    rules: &[Rule],
    email: &str,
    result: Option<TesterResult>,
    enabled: &EnabledChannels,
) -> String {
    let nav = nav_links("test");
    let channels_badge = {
        let badges = [
            ("email", true),
            ("telegram", enabled.telegram),
            ("discord", enabled.discord),
        ];
        badges
            .iter()
            .map(|(kind, on)| {
                let style = if *on {
                    "background:var(--code-bg);color:var(--fg)"
                } else {
                    "background:var(--code-bg);color:var(--muted);text-decoration:line-through"
                };
                format!(
                    r#"<span class="dest-tag dest-{kind}" style="{style}">{kind}</span>"#,
                    kind = kind,
                    style = style
                )
            })
            .collect::<String>()
    };
    let result_html = match result {
        None => String::new(),
        Some(r) => {
            let to_esc = html_escape(&r.to);
            match r.matched_index {
                Some(idx) => {
                    let rule = &rules[idx];
                    let action_detail = match &rule.action {
                        Action::Forward { destinations } => {
                            format!(
                                "Forward to {}",
                                destinations
                                    .iter()
                                    .map(|d| format!("{}:{}", d.kind_label(), d.value()))
                                    .collect::<Vec<_>>()
                                    .join(", ")
                            )
                        }
                        Action::Drop => "Drop".to_string(),
                    };
                    format!(
                        r#"<div class="tester-result matched">
<p>Input: <code>{to}</code></p>
<p>Matched <strong>rule {n}</strong>: <em>{label}</em> ({pattern})</p>
<p>Action: {action}</p>
</div>"#,
                        to = to_esc,
                        n = idx + 1,
                        label = html_escape(&rule.label),
                        pattern =
                            html_escape(&format!("{}@{}", rule.local_pattern, rule.domain_pattern)),
                        action = html_escape(&action_detail),
                    )
                }
                None => format!(
                    r#"<div class="tester-result nomatch"><p>No rule matches <code>{to}</code>.</p></div>"#,
                    to = to_esc
                ),
            }
        }
    };

    let content = format!(
        r##"{nav}
<h2>Rule tester</h2>
<p style="color:var(--muted);font-size:0.85rem;margin-bottom:0.5rem">Enter an inbound recipient address. The tester runs the configured rule set against it and shows which rule would fire.</p>
<p style="font-size:0.85rem;margin-bottom:1rem">Enabled destination kinds: {badges}</p>
<form hx-post="/manage/test" hx-target="#tester-target" hx-swap="innerHTML" hx-ext="json-enc">
  <div class="form-group">
    <label for="to">Inbound address</label>
    <input id="to" name="to" type="text" placeholder="shop.swizzles@kedi.dev" required>
  </div>
  <button type="submit" class="btn primary">Test</button>
</form>
<div id="tester-target">{result}</div>"##,
        nav = nav,
        badges = channels_badge,
        result = result_html,
    );

    base_html("Rule tester", email, &content)
}
