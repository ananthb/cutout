//! HTML templates for the management UI.
//!
//! Layout: a "pipeline workbench" — left pane lists routing rules as a
//! vertical pipeline (top-to-bottom evaluation order), right pane is an
//! inspector for the selected rule. Edit/add forms render as overlays.

use crate::bots::EnabledChannels;
use crate::helpers::html_escape;
use crate::stats::Stats7d;
use crate::types::{Action, Destination, Rule};
use crate::validation::Report;

/// Cutout brand mark, inline. Filled bottom-right square interlocks with
/// an outlined top-left square; the overlap is "cut out". Colors honor
/// `--accent` / `--bg-1` so it adapts to light/dark.
const LOGO_SVG: &str = r##"<svg width="22" height="22" viewBox="0 0 22 22" aria-hidden="true" style="display:block">
  <rect x="1.5" y="1.5" width="13" height="13" rx="2" fill="none" stroke="var(--accent)" stroke-width="1.6"/>
  <rect x="7.5" y="7.5" width="13" height="13" rx="2" fill="var(--accent)"/>
  <rect x="7.5" y="7.5" width="7" height="7" fill="var(--bg-1)" stroke="var(--accent)" stroke-width="1.6"/>
</svg>"##;

const CSS: &str = r##"
:root {
  --font-mono: "JetBrains Mono", ui-monospace, "SF Mono", Menlo, monospace;
  --font-sans: "Inter", system-ui, -apple-system, sans-serif;

  --bg: oklch(0.99 0.004 80);
  --bg-1: oklch(0.975 0.005 80);
  --bg-2: oklch(0.955 0.006 80);
  --bg-inset: oklch(0.93 0.008 80);
  --fg: oklch(0.18 0.01 60);
  --fg-1: oklch(0.36 0.01 60);
  --fg-2: oklch(0.56 0.012 60);
  --fg-3: oklch(0.72 0.012 60);
  --line: oklch(0.9 0.008 70);
  --line-2: oklch(0.85 0.01 70);

  --accent: oklch(0.68 0.17 48);
  --accent-fg: oklch(0.99 0.004 80);
  --accent-soft: oklch(0.95 0.04 60);

  --ok: oklch(0.62 0.14 150);
  --ok-soft: oklch(0.95 0.04 150);
  --warn: oklch(0.7 0.15 80);
  --warn-soft: oklch(0.95 0.04 80);
  --bad: oklch(0.6 0.17 25);
  --bad-soft: oklch(0.95 0.04 25);
  --info: oklch(0.62 0.13 240);
  --info-soft: oklch(0.95 0.04 240);

  --ch-email: oklch(0.62 0.13 240);
  --ch-discord: oklch(0.55 0.15 285);
  --ch-telegram: oklch(0.65 0.13 230);

  --r-xs: 3px; --r-sm: 5px; --r-md: 8px;
}
@media (prefers-color-scheme: dark) {
  :root {
    --bg: oklch(0.16 0.005 60);
    --bg-1: oklch(0.19 0.006 60);
    --bg-2: oklch(0.22 0.007 60);
    --bg-inset: oklch(0.13 0.005 60);
    --fg: oklch(0.95 0.004 80);
    --fg-1: oklch(0.82 0.005 80);
    --fg-2: oklch(0.62 0.008 80);
    --fg-3: oklch(0.45 0.008 80);
    --line: oklch(0.28 0.008 60);
    --line-2: oklch(0.35 0.01 60);
    --accent: oklch(0.74 0.16 52);
    --accent-fg: oklch(0.16 0.005 60);
    --accent-soft: oklch(0.28 0.04 50);
    --ok: oklch(0.72 0.14 150);
    --ok-soft: oklch(0.26 0.04 150);
    --warn: oklch(0.78 0.14 80);
    --warn-soft: oklch(0.26 0.04 80);
    --bad: oklch(0.7 0.16 25);
    --bad-soft: oklch(0.26 0.04 25);
    --info: oklch(0.74 0.12 240);
    --info-soft: oklch(0.26 0.04 240);
    --ch-email: oklch(0.74 0.12 240);
    --ch-discord: oklch(0.7 0.14 285);
    --ch-telegram: oklch(0.74 0.12 230);
  }
}

*, *::before, *::after { box-sizing: border-box; }
html, body { margin: 0; padding: 0; }
body {
  font-family: var(--font-sans);
  font-size: 14px; line-height: 1.45;
  color: var(--fg); background: var(--bg);
  -webkit-font-smoothing: antialiased;
}
button { font: inherit; color: inherit; background: none; border: 0; cursor: pointer; padding: 0; }
input, select, textarea { font: inherit; color: inherit; }
a { color: var(--accent); text-decoration: none; }
a:hover { text-decoration: underline; }
code { font-family: var(--font-mono); font-size: 0.88em; }
.mono { font-family: var(--font-mono); }

/* page shell -------------------------------------------------------- */
.workbench-shell {
  display: flex; flex-direction: column;
  min-height: 100vh;
}
.topbar {
  display: flex; align-items: center; justify-content: space-between;
  padding: 10px 18px;
  background: var(--bg-1);
  border-bottom: 1px solid var(--line);
}
.topbar .brand { display: flex; align-items: center; gap: 14px; }
.topbar .brand .title { display: flex; flex-direction: column; line-height: 1.1; }
.topbar .brand .title b { font-weight: 600; font-size: 14px; }
.topbar .brand .title small { font-family: var(--font-mono); font-size: 10.5px; color: var(--fg-2); }
.topbar .right { display: flex; align-items: center; gap: 12px; font-size: 11.5px; color: var(--fg-2); }
.topbar .right .health { display: inline-flex; align-items: center; gap: 6px; font-family: var(--font-mono); }
.topbar .right .health .dot { width: 6px; height: 6px; border-radius: 999px; background: var(--ok); }
.topbar .right .user { font-family: var(--font-mono); }
.microstats { display: flex; align-items: center; gap: 14px; }
.microstat { display: flex; flex-direction: column; line-height: 1.1; gap: 1px; }
.microstat .v {
  font-size: 14px; font-weight: 600;
  font-family: var(--font-mono);
}
.microstat .v.fwd  { color: var(--accent); }
.microstat .v.drp  { color: var(--bad); }
.microstat .v.muted { color: var(--fg-3); }
.microstat .k {
  font-family: var(--font-mono);
  font-size: 9.5px; color: var(--fg-2);
  text-transform: uppercase; letter-spacing: 0.06em;
}

.workbench {
  flex: 1; min-height: 0;
  display: grid; grid-template-columns: 440px 1fr;
}
@media (max-width: 880px) {
  .workbench { grid-template-columns: 1fr; }
}

/* pipeline pane ----------------------------------------------------- */
.pipeline-pane {
  border-right: 1px solid var(--line);
  background: var(--bg-1);
  display: flex; flex-direction: column;
  min-height: 0;
}
.pipeline-pane > header {
  padding: 12px 16px; border-bottom: 1px solid var(--line);
  display: flex; align-items: center; justify-content: space-between;
}
.pipeline-pane > header h3 {
  margin: 0; font-size: 12px; font-family: var(--font-mono);
  text-transform: uppercase; letter-spacing: 0.08em;
}
.pipeline-pane > header small {
  display: block;
  font-family: var(--font-mono); font-size: 10.5px; color: var(--fg-2);
}
.pipeline-list { padding: 16px 14px; }
.pipeline-node {
  display: inline-flex; align-items: center; gap: 10px;
  padding: 8px 10px;
  border: 1px dashed var(--line-2);
  border-radius: var(--r-sm);
  background: var(--bg-2);
  font-family: var(--font-mono); font-size: 11px;
  text-transform: uppercase; letter-spacing: 0.07em; color: var(--fg-1);
}
.pipeline-node .dot { width: 8px; height: 8px; border-radius: 999px; background: var(--fg-3); }
.pipeline-node.enter .dot { background: var(--ok); }
.pipeline-connector { margin-left: 14px; height: 16px; border-left: 2px dotted var(--line-2); }

.rule-card {
  display: block;
  border-radius: var(--r-md);
  border: 1px solid var(--line);
  background: var(--bg);
  overflow: hidden;
  position: relative;
  transition: box-shadow 0.15s, border-color 0.15s;
  text-decoration: none;
  color: inherit;
}
.rule-card:hover { text-decoration: none; border-color: var(--line-2); }
.rule-card.selected {
  border-color: var(--accent);
  box-shadow: 0 0 0 3px color-mix(in oklch, var(--accent) 18%, transparent);
}
.rule-card .grid { display: grid; grid-template-columns: 40px 1fr; }
.rule-card .order {
  display: flex; flex-direction: column; align-items: center; justify-content: center;
  gap: 4px; padding: 10px 0;
  border-right: 1px solid var(--line);
  font-family: var(--font-mono); font-size: 14px; font-weight: 700;
}
.rule-card .order.fwd  { background: color-mix(in oklch, var(--accent) 12%, var(--bg-1)); color: var(--accent); }
.rule-card .order.drop { background: color-mix(in oklch, var(--bad) 12%, var(--bg-1)); color: var(--bad); }
.rule-card .order.catch { background: var(--bg-inset); color: var(--fg-2); }
.rule-card .body { padding: 10px 12px; display: flex; flex-direction: column; gap: 6px; min-width: 0; }
.rule-card .row1 { display: flex; align-items: center; justify-content: space-between; gap: 8px; }
.rule-card .label {
  font-size: 13px; font-weight: 600;
  overflow: hidden; text-overflow: ellipsis; white-space: nowrap;
}
.rule-card .row2 { display: flex; align-items: center; justify-content: space-between; gap: 8px; }
.rule-card .pat-pill {
  padding: 3px 6px; background: var(--bg-inset); border-radius: 4px;
  font-family: var(--font-mono); font-size: 11.5px;
}
.rule-card .channels { display: inline-flex; gap: 6px; align-items: center; color: var(--fg-2); }
.rule-card .move-tools {
  position: absolute; top: 6px; right: 6px;
  display: flex; gap: 2px;
  background: var(--bg-1); padding: 2px;
  border: 1px solid var(--line); border-radius: 4px;
  opacity: 0; pointer-events: none; transition: opacity 0.12s;
}
.rule-card.selected .move-tools, .rule-card:hover .move-tools { opacity: 1; pointer-events: auto; }

/* inspector pane ---------------------------------------------------- */
.inspector-pane { overflow-y: auto; min-height: 0; display: flex; flex-direction: column; }
.inspector-header {
  padding: 16px 24px; border-bottom: 1px solid var(--line);
  display: flex; align-items: flex-start; justify-content: space-between; gap: 16px;
}
.inspector-header .meta { display: flex; flex-direction: column; gap: 6px; min-width: 0; }
.inspector-header .meta .id-row {
  display: flex; align-items: center; gap: 8px;
  font-family: var(--font-mono); font-size: 10.5px;
  text-transform: uppercase; letter-spacing: 0.08em; color: var(--fg-2);
}
.inspector-header h2 { margin: 0; font-size: 22px; font-weight: 600; }
.inspector-header .pat-display {
  display: inline-flex; align-items: center; gap: 6px;
  background: var(--bg-inset); padding: 4px 10px; border-radius: 5px;
  width: fit-content;
  font-family: var(--font-mono); font-size: 13px; font-weight: 500;
}
.inspector-body { padding: 24px; display: flex; flex-direction: column; gap: 18px; }

.stat-strip {
  display: grid; grid-template-columns: repeat(4, 1fr);
  border: 1px solid var(--line); border-radius: var(--r-md);
  background: var(--bg-1); overflow: hidden;
}
.stat-strip > div { padding: 14px 16px; display: flex; flex-direction: column; gap: 4px; }
.stat-strip > div + div { border-left: 1px solid var(--line); }
.stat-strip .k {
  font-family: var(--font-mono); font-size: 10px;
  text-transform: uppercase; letter-spacing: 0.07em; color: var(--fg-2);
}
.stat-strip .v {
  font-family: var(--font-mono);
  font-size: 22px; font-weight: 600;
}
.stat-strip .v.fwd { color: var(--accent); }
.stat-strip .v.drp { color: var(--bad); }
.stat-strip .v.muted { color: var(--fg-2); }
.stat-strip .sub {
  font-family: var(--font-mono); font-size: 10.5px; color: var(--fg-3);
}

.senders { display: flex; flex-direction: column; gap: 6px; }
.sender-row { display: grid; grid-template-columns: 1fr 44px; gap: 8px; align-items: center; }
.sender-bar {
  position: relative; height: 22px;
  background: var(--bg-inset); border-radius: 3px; overflow: hidden;
}
.sender-bar > .fill {
  position: absolute; inset: 0;
  background: color-mix(in oklch, var(--accent) 55%, transparent);
}
.sender-bar > .label {
  position: relative; padding: 0 8px;
  line-height: 22px; font-family: var(--font-mono);
  font-size: 11.5px; color: var(--fg);
  overflow: hidden; text-overflow: ellipsis; white-space: nowrap;
  display: block;
}
.sender-row .n {
  text-align: right; font-family: var(--font-mono); font-size: 11.5px;
}

.stats-missing {
  font-size: 11.5px; color: var(--fg-2);
  font-family: var(--font-mono);
}

/* live feed (bottom pane) ----------------------------------------- */
.live-feed {
  flex-shrink: 0;
  border-top: 1px solid var(--line);
  background: var(--bg-inset);
  height: 220px;
  display: flex; flex-direction: column;
  transition: height 0.18s ease;
}
.live-feed.collapsed { height: 36px; }
.live-feed-bar {
  height: 36px; flex-shrink: 0;
  display: flex; align-items: center; gap: 12px;
  padding: 0 16px;
  border-bottom: 1px solid var(--line);
}
.live-feed.collapsed .live-feed-bar { border-bottom: 0; }
.live-feed-bar .filters { display: flex; gap: 4px; }
.live-feed-bar .filter-chip {
  font-family: var(--font-mono); font-size: 11px;
  padding: 2px 7px; border-radius: 3px;
  border: 1px solid transparent;
  background: transparent; color: var(--fg-2);
  cursor: pointer;
}
.live-feed-bar .filter-chip.active {
  background: var(--bg-1); color: var(--fg);
  border-color: var(--line-2);
}
.live-feed-body {
  flex: 1; overflow-y: auto;
  font-family: var(--font-mono); font-size: 11.5px;
  padding: 6px 16px 12px;
}
.live-row {
  display: grid;
  grid-template-columns: 78px 70px 1fr 18px 1fr 70px 60px;
  gap: 10px; align-items: center;
  padding: 3px 0;
  color: var(--fg-1);
}
.live-row .ts { color: var(--fg-3); }
.live-row .evt { font-weight: 600; }
.live-row.k-forward .evt { color: var(--accent); }
.live-row.k-drop .evt { color: var(--bad); }
.live-row.k-reply .evt { color: var(--info); }
.live-row.k-reject .evt { color: var(--warn); }
.live-row .addr { overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
.live-row .arrow { color: var(--fg-3); }
.live-row .chs { display: flex; gap: 3px; }
.live-row .ch {
  display: inline-flex; align-items: center; justify-content: center;
  width: 14px; height: 14px; border-radius: 3px;
  font-size: 9px; font-weight: 700; color: #fff;
}
.live-row .ch.ch-email    { background: var(--ch-email); }
.live-row .ch.ch-telegram { background: var(--ch-telegram); }
.live-row .ch.ch-discord  { background: var(--ch-discord); }
.live-row .size { text-align: right; color: var(--fg-2); }
.live-feed-body .empty { font-size: 12px; color: var(--fg-2); text-align: center; padding: 18px; }

.card {
  background: var(--bg-1);
  border: 1px solid var(--line);
  border-radius: var(--r-md);
}
.card > header {
  display: flex; align-items: center; justify-content: space-between;
  padding: 12px 16px; border-bottom: 1px solid var(--line);
}
.card > header h3 {
  margin: 0; font-size: 12px; font-weight: 600;
  font-family: var(--font-mono);
  text-transform: uppercase; letter-spacing: 0.08em;
  color: var(--fg-1);
}
.card > header small {
  font-family: var(--font-mono); font-size: 11px; color: var(--fg-2);
}
.card-body { padding: 16px; }

.dest-grid { display: grid; grid-template-columns: repeat(auto-fill, minmax(260px, 1fr)); gap: 10px; }
.dest-card {
  display: flex; align-items: center; gap: 10px;
  border: 1px solid var(--line); border-radius: var(--r-sm);
  padding: 10px 12px; background: var(--bg);
}
.dest-card .icon-wrap {
  width: 28px; height: 28px; border-radius: 6px;
  display: inline-flex; align-items: center; justify-content: center;
}
.dest-card.email    .icon-wrap { background: color-mix(in oklch, var(--ch-email) 18%, transparent);    color: var(--ch-email); }
.dest-card.telegram .icon-wrap { background: color-mix(in oklch, var(--ch-telegram) 18%, transparent); color: var(--ch-telegram); }
.dest-card.discord  .icon-wrap { background: color-mix(in oklch, var(--ch-discord) 18%, transparent);  color: var(--ch-discord); }
.dest-card .meta { min-width: 0; flex: 1; display: flex; flex-direction: column; gap: 1px; }
.dest-card .kind {
  font-family: var(--font-mono); font-size: 11px; color: var(--fg-2);
  text-transform: uppercase; letter-spacing: 0.06em;
}
.dest-card .value {
  font-family: var(--font-mono); font-size: 12.5px;
  overflow: hidden; text-overflow: ellipsis; white-space: nowrap;
}

/* tester ----------------------------------------------------------- */
[x-cloak] { display: none !important; }
.tester-results {
  display: flex; flex-direction: column; gap: 8px;
  margin-top: 12px;
}
.tester-row {
  display: flex; align-items: center; gap: 10px;
  flex-wrap: wrap;
}
.tester-row-label {
  font-family: var(--font-mono); font-size: 10.5px;
  text-transform: uppercase; letter-spacing: 0.06em;
  color: var(--fg-2);
  min-width: 84px;
}

/* atoms (chip / tag / btn / field / input) -------------------------- */
.chip {
  display: inline-flex; align-items: center; gap: 6px;
  height: 22px; padding: 0 8px;
  border-radius: 999px; border: 1px solid var(--line);
  background: var(--bg-1); color: var(--fg-1);
  font-family: var(--font-mono); font-size: 11px;
  letter-spacing: 0.01em; white-space: nowrap;
}
.chip .dot { width: 6px; height: 6px; border-radius: 999px; background: currentColor; }
.chip.email    { color: var(--ch-email); }
.chip.discord  { color: var(--ch-discord); }
.chip.telegram { color: var(--ch-telegram); }
.chip.ok    { color: var(--ok); }
.chip.warn  { color: var(--warn); }
.chip.bad   { color: var(--bad); }
.chip.info  { color: var(--info); }

.tag {
  display: inline-flex; align-items: center; gap: 4px;
  height: 20px; padding: 0 6px;
  border-radius: var(--r-xs);
  font-family: var(--font-mono); font-size: 11px; font-weight: 500;
  background: var(--bg-inset); color: var(--fg-1);
}
.tag.forward { background: var(--info-soft); color: var(--info); }
.tag.proxy   { background: var(--accent-soft); color: var(--accent); }
.tag.drop    { background: var(--bad-soft); color: var(--bad); }
.tag.catch   { background: var(--warn-soft); color: var(--warn); }

.tip { position: relative; cursor: help; }
.tip::after {
  content: attr(data-tip);
  position: absolute; bottom: calc(100% + 6px); left: 50%;
  transform: translateX(-50%);
  background: var(--bg-2); color: var(--fg);
  border: 1px solid var(--line-2); border-radius: var(--r-sm);
  padding: 7px 9px;
  font-family: var(--font-sans); font-size: 11.5px; font-weight: 400;
  text-transform: none; letter-spacing: 0; line-height: 1.4;
  text-align: left; white-space: normal;
  width: max-content; max-width: 280px;
  pointer-events: none; opacity: 0; visibility: hidden;
  transition: opacity 0.12s ease, visibility 0.12s ease;
  transition-delay: 0.35s;
  box-shadow: 0 4px 14px rgba(0,0,0,0.18);
  z-index: 30;
}
.tip:hover::after, .tip:focus-visible::after { opacity: 1; visibility: visible; }

.btn {
  display: inline-flex; align-items: center; gap: 6px;
  height: 30px; padding: 0 12px;
  border: 1px solid var(--line);
  border-radius: var(--r-sm);
  background: var(--bg-1); color: var(--fg);
  font-size: 12.5px; font-weight: 500;
  white-space: nowrap;
  transition: background 0.12s, border-color 0.12s;
  text-decoration: none;
}
.btn:hover { background: var(--bg-2); border-color: var(--line-2); text-decoration: none; }
.btn.primary { background: var(--accent); border-color: var(--accent); color: var(--accent-fg); }
.btn.primary:hover { filter: brightness(1.04); background: var(--accent); }
.btn.ghost { border-color: transparent; background: transparent; }
.btn.ghost:hover { background: var(--bg-2); }
.btn.danger { color: var(--bad); }
.btn.danger:hover { background: var(--bad-soft); border-color: var(--bad); }
.btn.sm { height: 24px; padding: 0 8px; font-size: 11.5px; }
.btn.icon { width: 30px; padding: 0; justify-content: center; }
.btn.icon.sm { width: 24px; }
.btn:disabled { opacity: 0.4; cursor: not-allowed; }
.htmx-request.btn, .htmx-request .btn { opacity: 0.6; pointer-events: none; }

.field { display: flex; flex-direction: column; gap: 4px; }
.field > label {
  font-family: var(--font-mono); font-size: 10.5px;
  text-transform: uppercase; letter-spacing: 0.06em;
  color: var(--fg-2);
}
.field > .help { font-size: 11.5px; color: var(--fg-2); }
.input, .select {
  height: 32px; padding: 0 10px;
  border: 1px solid var(--line); border-radius: var(--r-sm);
  background: var(--bg); color: var(--fg);
  font-size: 13px; font-family: var(--font-mono);
  outline: none;
  transition: border-color 0.12s, box-shadow 0.12s;
}
.input:focus, .select:focus {
  border-color: var(--accent);
  box-shadow: 0 0 0 3px color-mix(in oklch, var(--accent) 20%, transparent);
}

.pat-input {
  display: grid; grid-template-columns: 1fr auto 1fr; gap: 6px;
  align-items: center; padding: 4px;
  border: 1px solid var(--line); border-radius: var(--r-sm);
  background: var(--bg);
}
.pat-input input { border: 0; height: 30px; background: transparent; outline: none;
  font-family: var(--font-mono); font-size: 13px; padding: 0 8px; color: var(--fg); }
.pat-input .at { color: var(--fg-3); font-family: var(--font-mono); padding: 0 4px; }

.action-toggle { display: flex; gap: 6px; }
.action-toggle button {
  flex: 1; padding: 10px; text-align: left;
  border: 1px solid var(--line); border-radius: var(--r-sm);
  background: var(--bg); color: var(--fg-1);
  display: flex; flex-direction: column; gap: 2px;
}
.action-toggle button.active {
  border-color: var(--accent); background: var(--accent-soft); color: var(--accent);
}
.action-toggle button strong { font-weight: 600; font-size: 12.5px; }
.action-toggle button small { font-size: 11px; color: var(--fg-2); }
.action-toggle button.active small { color: var(--accent); opacity: 0.8; }

/* destinations chip input (existing widget, restyled) -------------- */
.dest-field {
  border: 1px solid var(--line); border-radius: var(--r-sm);
  padding: 6px; background: var(--bg);
  display: flex; flex-wrap: wrap; gap: 4px; align-items: center;
  min-height: 38px; cursor: text;
}
.dest-field:focus-within {
  border-color: var(--accent);
  box-shadow: 0 0 0 3px color-mix(in oklch, var(--accent) 20%, transparent);
}
.dest-field .chip-input {
  flex: 1; min-width: 180px; border: none; background: transparent;
  outline: none; padding: 4px; font-size: 13px;
  color: var(--fg); font-family: var(--font-mono);
}
.dest-chip {
  display: inline-flex; align-items: center; gap: 2px;
  padding: 2px 2px 2px 8px;
  border-radius: 4px;
  font-family: var(--font-mono); font-size: 11.5px;
  background: var(--bg-inset); color: var(--fg-1);
}
.dest-chip.dest-email    { background: var(--info-soft); color: var(--info); }
.dest-chip.dest-telegram { background: color-mix(in oklch, var(--ch-telegram) 16%, transparent); color: var(--ch-telegram); }
.dest-chip.dest-discord  { background: color-mix(in oklch, var(--ch-discord) 16%, transparent);  color: var(--ch-discord); }
.dest-chip button {
  background: none; border: none; cursor: pointer;
  font-size: 1rem; line-height: 1; padding: 0 6px; color: inherit;
  opacity: 0.6; border-radius: 3px;
}
.dest-chip button:hover { opacity: 1; background: rgba(0,0,0,0.08); }
@media (prefers-color-scheme: dark) {
  .dest-chip button:hover { background: rgba(255,255,255,0.08); }
}
.chip-error { display: none; font-size: 11.5px; padding: 4px 6px; border-radius: 3px;
  margin-top: 4px; background: var(--bad-soft); color: var(--bad); }
.chip-error.visible { display: block; }

/* validation issues ------------------------------------------------ */
.issues { display: flex; flex-direction: column; gap: 6px; }
.issue {
  display: flex; gap: 8px; align-items: flex-start;
  padding: 8px 10px; border-radius: var(--r-sm);
  font-size: 12px;
}
.issue.err  { background: var(--bad-soft); color: var(--bad); }
.issue.warn { background: var(--warn-soft); color: var(--warn); }
.issue .label {
  font-family: var(--font-mono); font-size: 10px;
  text-transform: uppercase; letter-spacing: 0.07em;
  flex-shrink: 0; padding-top: 1px;
}

/* modal overlay ---------------------------------------------------- */
.modal-overlay {
  position: fixed; inset: 0; z-index: 50;
  background: rgba(0,0,0,0.45);
  display: flex; align-items: center; justify-content: center;
  padding: 20px;
}
.modal {
  width: 540px; max-width: 100%;
  background: var(--bg-1); border: 1px solid var(--line);
  border-radius: var(--r-md); overflow: hidden;
  display: flex; flex-direction: column;
  max-height: calc(100vh - 40px);
}
.modal > header {
  padding: 14px 18px; border-bottom: 1px solid var(--line);
  display: flex; justify-content: space-between; align-items: center;
}
.modal > header h3 {
  margin: 0; font-size: 13px;
  font-family: var(--font-mono);
  text-transform: uppercase; letter-spacing: 0.08em;
}
.modal-body { padding: 18px; display: flex; flex-direction: column; gap: 14px; overflow-y: auto; }
.modal-footer {
  padding: 14px 18px; border-top: 1px solid var(--line);
  display: flex; justify-content: flex-end; gap: 8px;
}

/* misc ------------------------------------------------------------ */
.empty {
  padding: 28px; color: var(--fg-2); font-size: 12.5px;
  text-align: center;
}
::-webkit-scrollbar { width: 10px; height: 10px; }
::-webkit-scrollbar-thumb { background: var(--line-2); border-radius: 6px; border: 2px solid var(--bg); }
::-webkit-scrollbar-track { background: transparent; }

@media (max-width: 880px) {
  .pipeline-pane { border-right: none; border-bottom: 1px solid var(--line); }
  .inspector-header { flex-direction: column; align-items: stretch; }
}
"##;

/// Alpine.js component factory for the rule editor modal — owns form state
/// (action type, destination chips, draft input, error message) and the
/// chip-input parsing/validation logic. Mirrors `Destination::parse_line`
/// in src/types.rs so the client gives the same errors the server would.
/// Also exposes `cutoutCloseModal` for HTMX-injected modals.
const ALPINE_SCRIPT: &str = r##"
function ruleEditor(initial) {
  return {
    action: initial.action,
    chips: initial.chips || [],
    draft: '',
    err: '',
    enabled: initial.enabled || [],
    serialize() { return this.chips.map(c => c.kind + ':' + c.value).join('\n'); },
    parse(raw) {
      const text = raw.trim();
      if (!text) return { empty: true };
      const idx = text.indexOf(':');
      if (idx < 0) return { err: "use 'kind:value' (e.g. email:you@example.com)" };
      const kindIn = text.slice(0, idx).trim().toLowerCase();
      const value  = text.slice(idx + 1).trim();
      if (!value) return { err: "value missing after ':'" };
      const alias = { email: 'email', telegram: 'telegram', tg: 'telegram', discord: 'discord', dc: 'discord' };
      const kind = alias[kindIn];
      if (!kind) return { err: "unknown kind (use email, telegram, or discord)" };
      if (this.enabled.indexOf(kind) < 0) return { err: kind + " is not enabled on this deployment" };
      if (kind === 'email') {
        if (!value.includes('@') || value.startsWith('@') || value.endsWith('@'))
          return { err: "email address must contain '@'" };
        return { kind, value: value.toLowerCase() };
      }
      if (kind === 'telegram') {
        if (!/^-?\d+$/.test(value)) return { err: "telegram chat_id must be an integer" };
        return { kind, value };
      }
      if (kind === 'discord') {
        if (!/^\d+$/.test(value)) return { err: "discord channel_id must be a positive integer" };
        return { kind, value };
      }
      return { err: "unknown kind" };
    },
    commit() {
      const r = this.parse(this.draft);
      if (r.empty) { this.err = ''; return true; }
      if (r.err)   { this.err = r.err; return false; }
      this.chips.push({ kind: r.kind, value: r.value });
      this.draft = ''; this.err = '';
      return true;
    },
    onSubmit(e) {
      if (this.action === 'forward' && this.draft && !this.commit()) e.preventDefault();
    },
  };
}

function cutoutCloseModal() {
  const slot = document.getElementById('editor-modal');
  if (slot) slot.innerHTML = '';
}

function tester(initial) {
  return {
    to: '',
    rules: initial.rules,
    selectedId: initial.selectedId,
    glob(p, v) {
      p = (p || '').toLowerCase();
      v = (v || '').toLowerCase();
      let pi = 0, vi = 0, sp = -1, sv = 0;
      while (vi < v.length) {
        if (pi < p.length && (p[pi] === '?' || p[pi] === v[vi])) { pi++; vi++; }
        else if (pi < p.length && p[pi] === '*') { sp = pi; sv = vi; pi++; }
        else if (sp >= 0) { pi = sp + 1; sv++; vi = sv; }
        else return false;
      }
      while (pi < p.length && p[pi] === '*') pi++;
      return pi === p.length;
    },
    parts() {
      const i = (this.to || '').lastIndexOf('@');
      if (i < 0) return null;
      return [this.to.slice(0, i).trim(), this.to.slice(i + 1).trim()];
    },
    matches(rule) {
      const p = this.parts();
      if (!p || !p[1]) return false;
      return this.glob(rule.local, p[0]) && this.glob(rule.domain, p[1]);
    },
    selectedRule() {
      return this.rules.find(r => r.id === this.selectedId);
    },
    selectedMatches() {
      const r = this.selectedRule();
      return !!(r && this.matches(r));
    },
    firstMatch() {
      return this.rules.find(r => this.matches(r));
    },
    selectedFires() {
      const f = this.firstMatch();
      return !!(f && f.id === this.selectedId);
    },
  };
}

function liveFeed() {
  return {
    events: [],
    lastTs: 0,
    filter: 'all',
    collapsed: true,
    intervalId: null,
    init() {},
    start() {
      if (this.intervalId) return;
      this.poll(true);
      this.intervalId = setInterval(() => this.poll(false), 2000);
    },
    stop() {
      if (this.intervalId) {
        clearInterval(this.intervalId);
        this.intervalId = null;
      }
    },
    async poll(initial) {
      try {
        const url = initial ? '/manage/events'
                            : '/manage/events?since=' + this.lastTs;
        const r = await fetch(url, { credentials: 'same-origin' });
        if (!r.ok) return;
        const j = await r.json();
        if (initial) {
          this.events = j.events || [];
        } else if (Array.isArray(j.events) && j.events.length) {
          this.events = j.events.concat(this.events).slice(0, 80);
        }
        if (typeof j.now === 'number') this.lastTs = j.now;
      } catch (_) { /* swallow — polling will retry */ }
    },
    toggle() {
      this.collapsed = !this.collapsed;
      if (this.collapsed) this.stop();
      else this.start();
    },
    visible() {
      if (this.filter === 'all') return this.events;
      return this.events.filter(e => e.kind === this.filter);
    },
    fmt_ts(ms) {
      const d = new Date(ms);
      const pad = n => String(n).padStart(2, '0');
      return pad(d.getHours()) + ':' + pad(d.getMinutes()) + ':' + pad(d.getSeconds());
    },
    fmt_size(b) {
      if (!b && b !== 0) return '';
      if (b < 1024) return b + 'B';
      if (b < 1048576) return (b / 1024).toFixed(1) + 'kb';
      return (b / 1048576).toFixed(1) + 'mb';
    },
  };
}
"##;

/// Render the base HTML wrapper used by every /manage page.
pub fn base_html(title: &str, content: &str) -> String {
    format!(
        r##"<!DOCTYPE html>
<html lang="en">
<head>
<meta charset="utf-8">
<meta name="viewport" content="width=device-width, initial-scale=1">
<title>{title} — Cutout</title>
<link rel="icon" type="image/svg+xml" href="/manage/assets/cutout-mark.svg">
<link rel="preconnect" href="https://fonts.googleapis.com">
<link rel="preconnect" href="https://fonts.gstatic.com" crossorigin>
<link href="https://fonts.googleapis.com/css2?family=Inter:wght@400;500;600;700&family=JetBrains+Mono:wght@400;500;600;700&display=swap" rel="stylesheet">
<style>{css}</style>
<script src="https://unpkg.com/htmx.org@2.0.8/dist/htmx.min.js" crossorigin="anonymous"></script>
<script src="https://unpkg.com/htmx-ext-json-enc@2.0.3/json-enc.js" crossorigin="anonymous"></script>
<script>{alpine_script}</script>
<script defer src="https://unpkg.com/alpinejs@3.14.1/dist/cdn.min.js" crossorigin="anonymous"></script>
</head>
<body>
{content}
<div id="editor-modal"></div>
</body>
</html>"##,
        title = html_escape(title),
        css = CSS,
        alpine_script = ALPINE_SCRIPT,
        content = content,
    )
}

/// Top bar shared across /manage pages.
fn topbar(email: &str, stats: Option<&Stats7d>) -> String {
    let microstats = match stats {
        Some(s) => format!(
            r##"<span style="width:1px;height:22px;background:var(--line)"></span>
<div class="microstats">
  <div class="microstat"><span class="v fwd">{fwd}</span><span class="k">forwarded · 7d</span></div>
  <div class="microstat"><span class="v drp">{drp}</span><span class="k">dropped · 7d</span></div>
</div>"##,
            fwd = s.forwarded_total,
            drp = s.dropped_total,
        ),
        None => String::new(),
    };
    format!(
        r##"<header class="topbar">
  <div class="brand">
    {logo}
    <div class="title">
      <b>Cutout</b>
      <small>routing pipeline</small>
    </div>
    {microstats}
  </div>
  <div class="right">
    <span class="health"><span class="dot"></span>worker live</span>
    <span class="user">{email}</span>
  </div>
</header>"##,
        logo = LOGO_SVG,
        email = html_escape(email),
    )
}

/// Full rules management page (the workbench).
pub fn rules_page(
    rules: &[Rule],
    email: &str,
    report: &Report,
    enabled: &EnabledChannels,
    selected_id: Option<&str>,
    stats: Option<&Stats7d>,
) -> String {
    let selected_idx = pick_selected_idx(rules, selected_id);
    let workbench = workbench(rules, report, enabled, selected_idx, stats);
    let content = format!(
        r##"<div class="workbench-shell">
{topbar}
{workbench}
{live_feed}
</div>"##,
        topbar = topbar(email, stats),
        workbench = workbench,
        live_feed = LIVE_FEED_PANE,
    );
    base_html("Rules", &content)
}

/// The bottom live feed pane. Sits outside `#workbench` so its Alpine
/// component (and the polling interval it owns) survives HTMX swaps.
const LIVE_FEED_PANE: &str = r##"<div class="live-feed" :class="{ collapsed }"
  x-data="liveFeed()" x-init="init()">
  <div class="live-feed-bar">
    <button class="btn ghost sm" @click="toggle()" type="button" style="padding:0 6px">
      <span x-text="collapsed ? '▸' : '▾'"></span>
      <span class="mono" style="text-transform:uppercase;letter-spacing:0.08em;font-size:11px">Live feed</span>
    </button>
    <span class="chip" :class="collapsed ? '' : 'ok'" style="height:20px;font-size:10.5px">
      <span class="dot" x-show="!collapsed"></span>
      <span x-text="collapsed ? 'paused — click to stream' : 'streaming'"></span>
    </span>
    <div class="filters" x-show="!collapsed">
      <template x-for="f in ['all','forward','drop','reply','reject']" :key="f">
        <button class="filter-chip" :class="{ active: filter === f }" @click="filter = f" x-text="f" type="button"></button>
      </template>
    </div>
    <span class="mono" style="margin-left:auto;font-size:11px;color:var(--fg-2)" x-text="visible().length + ' events'"></span>
  </div>
  <div class="live-feed-body" x-show="!collapsed">
    <template x-for="(e, i) in visible()" :key="i + ':' + e.ts">
      <div class="live-row" :class="'k-' + e.kind">
        <span class="ts" x-text="fmt_ts(e.ts)"></span>
        <span class="evt" x-text="e.kind.toUpperCase()"></span>
        <span class="addr" x-text="e.from"></span>
        <span class="arrow">→</span>
        <span class="addr" x-text="e.to"></span>
        <span class="chs">
          <template x-for="c in e.channels" :key="c">
            <span class="ch" :class="'ch-' + c" x-text="c[0].toUpperCase()"></span>
          </template>
        </span>
        <span class="size" x-text="fmt_size(e.size_bytes)"></span>
      </div>
    </template>
    <div x-show="visible().length === 0" class="empty">No events yet — they'll appear here as mail flows.</div>
  </div>
</div>"##;

/// Pick the index of the rule to show in the inspector. Falls back to the
/// first non-catch-all rule, or the catch-all if it's the only one.
pub fn pick_selected_idx(rules: &[Rule], requested: Option<&str>) -> usize {
    if let Some(id) = requested {
        if let Some(i) = rules.iter().position(|r| r.id == id) {
            return i;
        }
    }
    rules
        .iter()
        .position(|r| !r.is_catch_all())
        .unwrap_or(0)
        .min(rules.len().saturating_sub(1))
}

/// Standalone SVG for the favicon — same mark as `LOGO_SVG`, but with
/// colors hardcoded (no CSS vars) so it renders correctly when served as
/// a static asset. Inner cutout uses `prefers-color-scheme` so it sits
/// flush against light or dark browser-tab backgrounds.
pub const LOGO_SVG_FILE: &str = r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 22 22" width="22" height="22">
  <style>
    .bg { fill: #ffffff; }
    @media (prefers-color-scheme: dark) { .bg { fill: #1a1a1a; } }
  </style>
  <rect x="1.5" y="1.5" width="13" height="13" rx="2" fill="none" stroke="#dd793a" stroke-width="1.6"/>
  <rect x="7.5" y="7.5" width="13" height="13" rx="2" fill="#dd793a"/>
  <rect class="bg" x="7.5" y="7.5" width="7" height="7" stroke="#dd793a" stroke-width="1.6"/>
</svg>"##;

/// The two-pane workbench (pipeline + inspector). Wrapped in `#workbench`
/// so HTMX endpoints can swap the whole region after CRUD.
pub fn workbench(
    rules: &[Rule],
    report: &Report,
    enabled: &EnabledChannels,
    selected_idx: usize,
    stats: Option<&Stats7d>,
) -> String {
    let pipeline = pipeline_pane(rules, selected_idx);
    let inspector = if rules.is_empty() {
        r#"<section class="inspector-pane"><div class="empty">No rules yet — add one to get started.</div></section>"#.to_string()
    } else {
        inspector_pane(rules, selected_idx, report, enabled, stats)
    };
    format!(
        r##"<div id="workbench" class="workbench">
{pipeline}
{inspector}
</div>"##
    )
}

/// HTMX-targeted response: same as `workbench`, plus an out-of-band
/// `#editor-modal` clear so any open modal closes after a successful CRUD.
pub fn workbench_response(
    rules: &[Rule],
    report: &Report,
    enabled: &EnabledChannels,
    selected_idx: usize,
    stats: Option<&Stats7d>,
) -> String {
    let body = workbench(rules, report, enabled, selected_idx, stats);
    format!("{body}\n<div id=\"editor-modal\" hx-swap-oob=\"true\"></div>")
}

/// Left pane: pipeline of rule cards, top-to-bottom.
fn pipeline_pane(rules: &[Rule], selected_idx: usize) -> String {
    let cards: String = rules
        .iter()
        .enumerate()
        .map(|(i, r)| {
            let connector = r#"<div class="pipeline-connector"></div>"#;
            format!("{connector}{}", pipeline_card(r, i, i == selected_idx))
        })
        .collect();
    format!(
        r##"<aside class="pipeline-pane">
  <header>
    <div>
      <h3>Routing pipeline</h3>
      <small>{n} {rule_word} · evaluated top → bottom</small>
    </div>
    <button class="btn primary sm"
      hx-get="/manage/rules/new"
      hx-target="#editor-modal"
      hx-swap="innerHTML">
      + Rule
    </button>
  </header>
  <div class="pipeline-list">
    <div class="pipeline-node enter"><span class="dot"></span>INBOUND · email_routing</div>
    {cards}
    <div class="pipeline-connector"></div>
    <div class="pipeline-node"><span class="dot"></span>END · all rules evaluated</div>
  </div>
</aside>"##,
        n = rules.len(),
        rule_word = if rules.len() == 1 { "rule" } else { "rules" },
    )
}

/// One card in the pipeline.
fn pipeline_card(rule: &Rule, index: usize, selected: bool) -> String {
    let is_catch = rule.is_catch_all();
    let is_fwd = matches!(rule.action, Action::Forward { .. });
    let order_cls = if is_catch {
        "order catch"
    } else if is_fwd {
        "order fwd"
    } else {
        "order drop"
    };
    let card_cls = if selected {
        "rule-card selected"
    } else {
        "rule-card"
    };

    let action_cell = match &rule.action {
        Action::Drop if is_catch => {
            r#"<span class="tag catch">drop · catch-all</span>"#.to_string()
        }
        Action::Drop => r#"<span class="tag drop">drop</span>"#.to_string(),
        Action::Forward { destinations, .. } => channel_dots(destinations),
    };

    let move_tools = if is_catch {
        String::new()
    } else {
        let id_e = html_escape(&rule.id);
        let up_vals = format!(r#"{{"id":"{id_e}","direction":"up"}}"#);
        let down_vals = format!(r#"{{"id":"{id_e}","direction":"down"}}"#);
        format!(
            r##"<div class="move-tools" onclick="event.preventDefault();event.stopPropagation();">
  <button class="btn ghost icon sm" title="Move up"
    hx-post="/manage/rules/reorder" hx-vals='{up_vals}'
    hx-target="#workbench" hx-swap="outerHTML"
    hx-ext="json-enc"
    hx-include="[name=selected]">↑</button>
  <button class="btn ghost icon sm" title="Move down"
    hx-post="/manage/rules/reorder" hx-vals='{down_vals}'
    hx-target="#workbench" hx-swap="outerHTML"
    hx-ext="json-enc"
    hx-include="[name=selected]">↓</button>
</div>"##,
        )
    };

    format!(
        r##"<a href="/manage?rule={id}" class="{card_cls}" data-rule-id="{id}">
  <div class="grid">
    <div class="{order_cls}"><span>{order:02}</span></div>
    <div class="body">
      <div class="row1">
        <span class="label">{label}</span>
      </div>
      <div class="row2">
        <span class="pat-pill">{pattern}</span>
        {action_cell}
      </div>
    </div>
  </div>
  {move_tools}
</a>"##,
        id = html_escape(&rule.id),
        order = index + 1,
        label = html_escape(&rule.label),
        pattern = pattern_html(&rule.local_pattern, &rule.domain_pattern),
    )
}

/// Compact channel-dot row for the pipeline card (no labels, just icons).
fn channel_dots(destinations: &[Destination]) -> String {
    if destinations.is_empty() {
        return r#"<span class="tag forward">forward · 0</span>"#.to_string();
    }
    let mut email = 0;
    let mut telegram = 0;
    let mut discord = 0;
    for d in destinations {
        match d {
            Destination::Email { .. } => email += 1,
            Destination::Telegram { .. } => telegram += 1,
            Destination::Discord { .. } => discord += 1,
        }
    }
    let mut parts: Vec<String> = Vec::new();
    let push = |parts: &mut Vec<String>, ch: &str, n: usize| {
        if n == 0 {
            return;
        }
        let count = if n > 1 {
            format!(" {n}")
        } else {
            String::new()
        };
        parts.push(format!(
            r##"<span class="chip {ch}" style="height:20px;font-size:10.5px;padding:0 6px"><span class="dot"></span>{ch}{count}</span>"##,
        ));
    };
    push(&mut parts, "email", email);
    push(&mut parts, "telegram", telegram);
    push(&mut parts, "discord", discord);
    format!(r#"<span class="channels">{}</span>"#, parts.join("&nbsp;"))
}

/// Pattern with the wildcard pieces tinted accent.
fn pattern_html(local: &str, domain: &str) -> String {
    let piece = |s: &str| {
        if s == "*" {
            format!(
                r##"<span style="color:var(--accent);font-weight:600">{}</span>"##,
                html_escape(s)
            )
        } else {
            format!(r##"<span>{}</span>"##, html_escape(s))
        }
    };
    format!(
        r##"{}<span style="color:var(--fg-3);padding:0 1px">@</span>{}"##,
        piece(local),
        piece(domain),
    )
}

/// Right pane: details for the selected rule.
fn inspector_pane(
    rules: &[Rule],
    selected_idx: usize,
    report: &Report,
    _enabled: &EnabledChannels,
    stats: Option<&Stats7d>,
) -> String {
    let rule = &rules[selected_idx];
    let issues = report
        .issues
        .get(selected_idx)
        .map(Vec::as_slice)
        .unwrap_or(&[]);
    let is_catch = rule.is_catch_all();
    let is_fwd = matches!(rule.action, Action::Forward { .. });
    let dest_count = match &rule.action {
        Action::Forward { destinations, .. } => destinations.len(),
        Action::Drop => 0,
    };
    let stat_strip = render_stat_strip(rule, dest_count, stats);
    let top_senders_card = stats
        .map(|s| render_top_senders(&s.top_senders))
        .unwrap_or_default();

    let action_tag = match &rule.action {
        Action::Forward {
            replace_reply_to: true,
            ..
        } => r#"<span class="tag proxy tip" data-tip="Forward in proxy/rewrite mode: the message is reconstructed and Reply-To is rewritten so replies route back through the worker via the same custom domain. Strips PGP signatures and attachments.">forward · proxy</span>"#.to_string(),
        Action::Forward { .. } => r#"<span class="tag forward tip" data-tip="Forward in native mode: uses Cloudflare EmailMessage.forward(). Original bytes (PGP, attachments) pass through untouched; Reply-To is overlaid but may be ignored by some clients.">forward</span>"#.to_string(),
        Action::Drop if is_catch => {
            r#"<span class="tag catch tip" data-tip="Pinned catch-all rule: silently drops anything no earlier rule matched. Always sits at the end and can't be deleted or moved.">drop · catch-all</span>"#.to_string()
        }
        Action::Drop => r#"<span class="tag drop tip" data-tip="Silently discard inbound mail matching this rule. No notification, no bounce.">drop</span>"#.to_string(),
    };

    let edit_btn = format!(
        r##"<button class="btn"
  hx-get="/manage/rules/{id}/edit"
  hx-target="#editor-modal"
  hx-swap="innerHTML">Edit</button>"##,
        id = html_escape(&rule.id),
    );
    let delete_btn = if is_catch {
        String::new()
    } else {
        format!(
            r##"<button class="btn danger"
  hx-delete="/manage/rules/{id}"
  hx-confirm="Delete this rule?"
  hx-target="#workbench" hx-swap="outerHTML"
  hx-ext="json-enc"
  hx-include="[name=selected]">Delete</button>"##,
            id = html_escape(&rule.id),
        )
    };

    // Hidden form holding the currently-selected rule id, picked up by
    // hx-include on every CRUD button so the server can preserve selection
    // across swaps.
    let selected_form = format!(
        r#"<form id="selection-form" style="display:none">
  <input type="hidden" name="selected" value="{id}">
</form>"#,
        id = html_escape(&rule.id),
    );

    let destinations_card = match &rule.action {
        Action::Forward {
            destinations,
            replace_reply_to,
        } => destinations_card(destinations, *replace_reply_to),
        Action::Drop => String::new(),
    };

    let issues_card = if issues.is_empty() {
        String::new()
    } else {
        let lines: String = issues
            .iter()
            .map(|i| {
                let (cls, label) = if i.is_error() {
                    ("err", "ERROR")
                } else {
                    ("warn", "WARN")
                };
                format!(
                    r##"<div class="issue {cls}"><span class="label">{label}</span><span>{msg}</span></div>"##,
                    msg = html_escape(i.message()),
                )
            })
            .collect();
        format!(
            r##"<div class="card">
  <header><h3>Validation</h3></header>
  <div class="card-body"><div class="issues">{lines}</div></div>
</div>"##,
        )
    };

    let sandbox = inspector_tester(rules, rule);

    let action_summary_card = if is_fwd {
        String::new()
    } else {
        format!(
            r##"<div class="card">
  <header><h3>Action</h3></header>
  <div class="card-body">
    <p style="margin:0;color:var(--fg-2);font-size:12.5px">
      Inbound mail matching this rule is silently dropped — no notification, no bounce.
      {extra}
    </p>
  </div>
</div>"##,
            extra = if is_catch {
                "This is the pinned catch-all; it always sits at the end and can't be deleted or moved."
            } else {
                ""
            },
        )
    };

    format!(
        r##"<section class="inspector-pane">
  <div class="inspector-header">
    <div class="meta">
      <div class="id-row"><span class="tip" data-tip="Stable random identifier (UUID v4) for this rule. Used in the URL when editing or deleting and in stats keyed by rule.">rule · {id}</span>{action_tag}</div>
      <h2>{label}</h2>
      <span class="pat-display">{pattern}</span>
    </div>
    <div style="display:flex;gap:6px;flex-shrink:0">{edit_btn}{delete_btn}</div>
  </div>
  <div class="inspector-body">
    {stat_strip}
    {issues_card}
    {destinations_card}
    {action_summary_card}
    {top_senders_card}
    {sandbox}
    {selected_form}
  </div>
</section>"##,
        id = html_escape(&rule.id),
        label = html_escape(&rule.label),
        pattern = pattern_html(&rule.local_pattern, &rule.domain_pattern),
    )
}

/// Stats strip at the top of the inspector body. Shows matches·7d /
/// last-match (from AE) when available, plus destinations count + channels.
fn render_stat_strip(rule: &Rule, dest_count: usize, stats: Option<&Stats7d>) -> String {
    let rule_stats = stats.and_then(|s| s.by_rule.get(&rule.id));
    let m_tip = r#"data-tip="Number of inbound emails this rule matched in the last 7 days. Sourced from Cloudflare Analytics Engine.""#;
    let l_tip = r#"data-tip="When this rule last matched an inbound email. Sourced from Cloudflare Analytics Engine.""#;
    let matches_cell = match rule_stats {
        Some(rs) => format!(
            r##"<div><span class="k tip" {m_tip}>matches · 7d</span><span class="v fwd">{}</span></div>"##,
            rs.matches
        ),
        None => format!(
            r##"<div><span class="k tip" {m_tip}>matches · 7d</span><span class="v muted">—</span><span class="sub">{}</span></div>"##,
            if stats.is_none() {
                "stats unavailable"
            } else {
                "no recent traffic"
            }
        ),
    };
    let last_cell = match rule_stats.and_then(|rs| rs.last_match_s) {
        Some(ts_s) => format!(
            r##"<div><span class="k tip" {l_tip}>last match</span><span class="v">{}</span><span class="sub">unix · {ts_s}</span></div>"##,
            relative_time_from_seconds(ts_s, stats.map(|s| s.generated_at).unwrap_or(0))
        ),
        None => format!(
            r##"<div><span class="k tip" {l_tip}>last match</span><span class="v muted">—</span></div>"##
        ),
    };

    let channels = match &rule.action {
        Action::Forward { destinations, .. } => {
            let mut e = 0;
            let mut t = 0;
            let mut d = 0;
            for x in destinations {
                match x {
                    Destination::Email { .. } => e += 1,
                    Destination::Telegram { .. } => t += 1,
                    Destination::Discord { .. } => d += 1,
                }
            }
            let mut parts = Vec::new();
            if e > 0 {
                parts.push(format!("{e} email"));
            }
            if t > 0 {
                parts.push(format!("{t} tg"));
            }
            if d > 0 {
                parts.push(format!("{d} dc"));
            }
            if parts.is_empty() {
                "—".to_string()
            } else {
                parts.join(" · ")
            }
        }
        Action::Drop => "—".to_string(),
    };

    format!(
        r##"<div class="stat-strip">
  {matches_cell}
  {last_cell}
  <div><span class="k tip" data-tip="Total destination targets attached to this forward rule. Each can be an email address, Telegram chat, or Discord channel.">destinations</span><span class="v">{dest_count}</span></div>
  <div><span class="k tip" data-tip="Breakdown of destinations by channel kind: email · tg (Telegram) · dc (Discord).">channels</span><span class="v" style="font-size:14px">{channels}</span></div>
</div>"##,
    )
}

/// "Top senders" card. Always renders (under the inspector body), since
/// the data is global rather than per-rule.
fn render_top_senders(senders: &[crate::stats::TopSender]) -> String {
    if senders.is_empty() {
        return r##"<div class="card">
  <header><h3>Top senders · 7d</h3><small>global</small></header>
  <div class="card-body"><div class="empty">No forwarded mail in the last 7 days.</div></div>
</div>"##
            .to_string();
    }
    let max = senders.iter().map(|s| s.n).max().unwrap_or(1).max(1);
    let rows: String = senders
        .iter()
        .map(|s| {
            let pct = (s.n as f64 / max as f64 * 100.0).round();
            format!(
                r##"<div class="sender-row">
  <div class="sender-bar">
    <div class="fill" style="width:{pct}%"></div>
    <span class="label">{addr}</span>
  </div>
  <span class="n">{n}</span>
</div>"##,
                addr = html_escape(&s.address),
                n = s.n,
            )
        })
        .collect();
    format!(
        r##"<div class="card">
  <header><h3>Top senders · 7d</h3><small>global · forwarded only</small></header>
  <div class="card-body"><div class="senders">{rows}</div></div>
</div>"##,
    )
}

/// Render a "5m ago" / "3h ago" style relative time from a unix-second
/// timestamp, anchored to `now_ms` (unix milliseconds).
fn relative_time_from_seconds(ts_s: i64, now_ms: i64) -> String {
    if ts_s <= 0 {
        return "—".into();
    }
    let now_s = if now_ms > 0 { now_ms / 1000 } else { 0 };
    if now_s == 0 {
        return format!("@{ts_s}");
    }
    let delta = (now_s - ts_s).max(0);
    if delta < 60 {
        format!("{delta}s ago")
    } else if delta < 3600 {
        format!("{}m ago", delta / 60)
    } else if delta < 86400 {
        format!("{}h ago", delta / 3600)
    } else {
        format!("{}d ago", delta / 86400)
    }
}

/// "Destinations" card in the inspector, with a card per destination plus
/// a tag indicating native vs. proxy mode.
fn destinations_card(destinations: &[Destination], replace_reply_to: bool) -> String {
    let mode_tag = if replace_reply_to {
        r#"<span class="tag proxy tip" data-tip="Proxy/rewrite mode: messages are reconstructed via Email Service so Reply-To works when replying via the same custom domain. Strips PGP signatures and attachments.">proxy mode</span>"#
    } else {
        r#"<span class="tag forward tip" data-tip="Native mode: uses EmailMessage.forward() — original bytes (PGP, attachments, formatting) pass through untouched. Reply-To is overlaid but may be ignored by some mail clients.">native mode</span>"#
    };
    let body = if destinations.is_empty() {
        r#"<div class="empty">No destinations — this forward is a no-op until you add at least one.</div>"#.to_string()
    } else {
        let cards: String = destinations
            .iter()
            .map(|d| {
                let kind = d.kind_label();
                let icon = channel_icon(kind);
                format!(
                    r##"<div class="dest-card {kind}">
  <span class="icon-wrap">{icon}</span>
  <div class="meta">
    <span class="kind">{kind}</span>
    <span class="value">{value}</span>
  </div>
</div>"##,
                    value = html_escape(d.value()),
                )
            })
            .collect();
        format!(r#"<div class="dest-grid">{cards}</div>"#)
    };

    format!(
        r##"<div class="card">
  <header>
    <h3>Destinations <small>({n})</small></h3>
    {mode_tag}
  </header>
  <div class="card-body">{body}</div>
</div>"##,
        n = destinations.len(),
    )
}

/// Inline channel icon (12px). Matches the design's atoms.jsx ICONS table.
fn channel_icon(kind: &str) -> &'static str {
    match kind {
        "email" => {
            r##"<svg width="14" height="14" viewBox="0 0 16 16" fill="none" stroke="currentColor" stroke-width="1.4"><rect x="2" y="3.5" width="12" height="9" rx="1.5"/><path d="M2.5 4.5l5.5 4 5.5-4"/></svg>"##
        }
        "discord" => {
            r##"<svg width="14" height="14" viewBox="0 0 16 16" fill="currentColor"><path d="M13 3.5a11 11 0 0 0-2.7-.8l-.2.5a9.7 9.7 0 0 0-4.2 0l-.2-.5a11 11 0 0 0-2.7.8C1 7 1 9.5 1 12c0 0 1.4 1 3.4 1.4l.5-.9a6 6 0 0 1-1.1-.5l.3-.2a8 8 0 0 0 7.8 0l.3.2a6 6 0 0 1-1.1.5l.5.9c2-.4 3.4-1.4 3.4-1.4 0-2.5 0-5-2-8.5zM6 10c-.6 0-1.1-.6-1.1-1.3 0-.7.5-1.3 1.1-1.3.7 0 1.2.6 1.1 1.3 0 .7-.5 1.3-1.1 1.3zm4 0c-.6 0-1.1-.6-1.1-1.3 0-.7.5-1.3 1.1-1.3.7 0 1.2.6 1.1 1.3 0 .7-.5 1.3-1.1 1.3z"/></svg>"##
        }
        "telegram" => {
            r##"<svg width="14" height="14" viewBox="0 0 16 16" fill="currentColor"><path d="M14.4 2.3 1.7 7.2c-.9.3-.8 1.5.1 1.7l3 .8 1.2 3.7c.2.6.9.7 1.3.3l1.7-1.5 3.3 2.4c.6.4 1.4 0 1.5-.7l2-9.7c.2-.9-.7-1.5-1.4-1.2zm-2.3 3-5.3 4.7-.2 2.5-1-2.9 6.5-4.3z"/></svg>"##
        }
        _ => "",
    }
}

/// Interactive tester card in the inspector. Mirrors the routing engine's
/// glob matcher in JS so each keystroke runs the full ruleset client-side
/// and shows: (a) does the *selected* rule's pattern match this address,
/// (b) which rule actually fires (top-down, first match wins), (c) if a
/// different rule fires earlier, link to it.
fn inspector_tester(all_rules: &[Rule], _selected: &Rule) -> String {
    // Pass enough rule metadata for client-side eval. Action label drives
    // the result tag colour.
    let rules_json: Vec<serde_json::Value> = all_rules
        .iter()
        .map(|r| {
            let action = match &r.action {
                Action::Drop => "drop",
                Action::Forward {
                    replace_reply_to: true,
                    ..
                } => "forward · proxy",
                Action::Forward { .. } => "forward",
            };
            serde_json::json!({
                "id": r.id,
                "local": r.local_pattern,
                "domain": r.domain_pattern,
                "label": r.label,
                "action": action,
            })
        })
        .collect();
    let init = serde_json::json!({
        "rules": rules_json,
        "selectedId": _selected.id,
    })
    .to_string();
    let init_attr = html_escape(&init);

    format!(
        r##"<div class="card" x-data='tester({init_attr})'>
  <header><h3>Tester</h3><small>this rule + full ruleset</small></header>
  <div class="card-body">
    <div class="field">
      <label>Test address</label>
      <input class="input" type="text"
        placeholder="shop@yourdomain.example"
        x-model="to" autocomplete="off" spellcheck="false">
    </div>
    <div class="tester-results" x-show="to" x-cloak>
      <div class="tester-row">
        <span class="tester-row-label">This rule</span>
        <template x-if="selectedMatches()">
          <span class="chip ok"><span class="dot"></span>matches</span>
        </template>
        <template x-if="!selectedMatches()">
          <span class="chip" style="opacity:0.7">✗ no match</span>
        </template>
      </div>
      <div class="tester-row">
        <span class="tester-row-label">Would fire</span>
        <template x-if="selectedFires()">
          <span class="chip ok"><span class="dot"></span>this rule · <span x-text="firstMatch().action"></span></span>
        </template>
        <template x-if="firstMatch() &amp;&amp; !selectedFires()">
          <a class="chip warn" :href="'/manage?rule=' + firstMatch().id">
            <span class="dot"></span>
            <span x-text="firstMatch().label"></span>
            <span style="opacity:0.6;margin-left:2px">→</span>
          </a>
        </template>
        <template x-if="!firstMatch() &amp;&amp; to.includes('@')">
          <span class="chip bad"><span class="dot"></span>no rule matches — bounced</span>
        </template>
        <template x-if="!firstMatch() &amp;&amp; !to.includes('@')">
          <span class="chip" style="opacity:0.7">waiting for full address…</span>
        </template>
      </div>
      <div class="tester-row" x-show="firstMatch() &amp;&amp; selectedMatches() &amp;&amp; !selectedFires()">
        <span class="tester-row-label" style="color:var(--warn)">Note</span>
        <span style="font-size:11.5px;color:var(--fg-2)">This rule would match, but a higher rule catches first.</span>
      </div>
    </div>
  </div>
</div>"##
    )
}

// ---------------------------------------------------------------------------
// Editor modal — used for both add and edit.
// ---------------------------------------------------------------------------

/// Render the destinations chip-input field. The Alpine `x-data` factory is
/// installed once on the surrounding form (`ruleEditor(...)`); this just
/// emits the markup that binds to that scope.
fn destinations_field(enabled: &EnabledChannels) -> String {
    let mut kinds: Vec<&str> = vec!["email"];
    if enabled.telegram {
        kinds.push("telegram");
    }
    if enabled.discord {
        kinds.push("discord");
    }
    let kinds_help = kinds
        .iter()
        .map(|k| format!("<code>{k}</code>"))
        .collect::<Vec<_>>()
        .join(", ");
    let missing = match (enabled.telegram, enabled.discord) {
        (true, true) => String::new(),
        (false, true) => " (set <code>TELEGRAM_BOT_TOKEN</code> to enable telegram)".into(),
        (true, false) => " (set <code>DISCORD_BOT_TOKEN</code> + <code>DISCORD_APP_ID</code> + <code>DISCORD_PUBLIC_KEY</code> to enable discord)".into(),
        (false, false) => " (set the telegram / discord bot secrets to enable those kinds)".into(),
    };
    format!(
        r##"<div class="dest-wrapper">
  <div class="dest-field" @click.self="$refs.chipInput.focus()">
    <template x-for="(c, i) in chips" :key="i + ':' + c.kind + ':' + c.value">
      <span class="dest-chip" :class="'dest-' + c.kind">
        <span x-text="c.kind + ':' + c.value"></span>
        <button type="button" aria-label="remove" @click="chips.splice(i, 1)">×</button>
      </span>
    </template>
    <input x-ref="chipInput"
      class="chip-input" type="text" x-model="draft"
      placeholder="email:you@example.com"
      autocomplete="off" spellcheck="false"
      @keydown.enter.prevent="commit()"
      @keydown.window.escape="cutoutCloseModal()"
      @keydown="if ($event.key === ',') {{ $event.preventDefault(); commit(); }}
                else if ($event.key === 'Backspace' && !draft && chips.length) {{ chips.pop(); }}
                else {{ err = ''; }}">
    <input type="hidden" name="destinations" :value="serialize()">
  </div>
  <div class="chip-error" :class="{{ visible: !!err }}" x-text="err"></div>
</div>
<div class="help" style="margin-top:6px">Press Enter or comma to add. Each entry must be <code>kind:value</code>. Available: {kinds_help}.{missing}</div>"##,
    )
}

/// New-rule modal.
pub fn new_rule_modal(enabled: &EnabledChannels) -> String {
    editor_modal(None, enabled, "/manage/rules", "post", "Create rule")
}

/// Edit-rule modal — returned for HTMX swap into `#editor-modal`.
pub fn edit_rule_form(rule: &Rule, enabled: &EnabledChannels) -> String {
    let action = format!("/manage/rules/{}", html_escape(&rule.id));
    editor_modal(Some(rule), enabled, &action, "put", "Save")
}

fn editor_modal(
    rule: Option<&Rule>,
    enabled: &EnabledChannels,
    form_action: &str,
    method: &str,
    submit_label: &str,
) -> String {
    let id = rule.map(|r| r.id.as_str()).unwrap_or("");
    let label = rule.map(|r| r.label.as_str()).unwrap_or("");
    let local = rule.map(|r| r.local_pattern.as_str()).unwrap_or("*");
    let domain = rule.map(|r| r.domain_pattern.as_str()).unwrap_or("*");

    let (action_type, destinations, replace_reply_to): (&str, &[Destination], bool) =
        match rule.map(|r| &r.action) {
            Some(Action::Forward {
                destinations,
                replace_reply_to,
            }) => ("forward", destinations.as_slice(), *replace_reply_to),
            Some(Action::Drop) => ("drop", &[], false),
            None => ("forward", &[], false),
        };

    let title = if rule.is_some() {
        format!("Edit rule · {}", html_escape(id))
    } else {
        "New rule".to_string()
    };

    let dest_field = destinations_field(enabled);
    let replace_checked = if replace_reply_to { " checked" } else { "" };
    let hx_attr = match method {
        "put" => format!(r#"hx-put="{form_action}""#),
        _ => format!(r#"hx-post="{form_action}""#),
    };

    // Build the Alpine x-data initializer (JSON, then HTML-escape for the
    // single-quoted attribute). The browser will un-escape entities back
    // into the JS expression at parse time.
    let mut enabled_kinds: Vec<&str> = vec!["email"];
    if enabled.telegram {
        enabled_kinds.push("telegram");
    }
    if enabled.discord {
        enabled_kinds.push("discord");
    }
    let chips_json: Vec<serde_json::Value> = destinations
        .iter()
        .map(|d| serde_json::json!({ "kind": d.kind_label(), "value": d.value() }))
        .collect();
    let init_json = serde_json::json!({
        "action": action_type,
        "chips": chips_json,
        "enabled": enabled_kinds,
    })
    .to_string();
    let init_attr = html_escape(&init_json);

    format!(
        r##"<div class="modal-overlay"
  @click.self="cutoutCloseModal()"
  @keydown.escape.window="cutoutCloseModal()">
<div class="modal">
  <header>
    <h3>{title}</h3>
    <button class="btn ghost icon sm" type="button" aria-label="Close" @click="cutoutCloseModal()">×</button>
  </header>
  <form x-data='ruleEditor({init_attr})'
    {hx_attr}
    hx-target="#workbench" hx-swap="outerHTML"
    hx-ext="json-enc"
    hx-include="[name=selected]"
    @submit="onSubmit($event)">
    <div class="modal-body">
      <div class="field">
        <label>Label</label>
        <input class="input" name="label" type="text" value="{label}" placeholder="e.g. Newsletter drop" required style="font-family:var(--font-sans)">
      </div>
      <div class="field">
        <label>Pattern</label>
        <div class="pat-input">
          <input name="local_pattern" type="text" value="{local}" placeholder="*" required>
          <span class="at">@</span>
          <input name="domain_pattern" type="text" value="{domain}" placeholder="*" required>
        </div>
        <span class="help"><span style="color:var(--accent)">*</span> matches anything · <span style="color:var(--accent)">?</span> matches one char</span>
      </div>
      <div class="field">
        <label>Action</label>
        <div class="action-toggle">
          <button type="button" :class="{{ active: action === 'forward' }}" @click="action = 'forward'">
            <strong>Forward</strong>
            <small>Send to one or more destinations</small>
          </button>
          <button type="button" :class="{{ active: action === 'drop' }}" @click="action = 'drop'">
            <strong>Drop</strong>
            <small>Silently discard inbound mail</small>
          </button>
        </div>
        <input type="hidden" name="action_type" :value="action">
      </div>
      <div class="field" x-show="action === 'forward'">
        <label style="display:flex;justify-content:space-between;align-items:center">
          <span>Destinations</span>
          <label style="display:flex;align-items:center;gap:6px;font-family:var(--font-sans);font-size:11.5px;text-transform:none;letter-spacing:0;color:var(--fg-1);cursor:pointer">
            <input type="checkbox" name="replace_reply_to"{replace_checked}>
            Proxy via rewrite mode
          </label>
        </label>
        {dest_field}
        <span class="help">Rewrite mode ensures reply-to works when replying via the same domain, but strips PGP and attachments.</span>
      </div>
    </div>
    <div class="modal-footer">
      <button type="button" class="btn" @click="cutoutCloseModal()">Cancel</button>
      <button type="submit" class="btn primary">{submit_label}</button>
    </div>
  </form>
</div>
</div>"##,
        title = title,
        label = html_escape(label),
        local = html_escape(local),
        domain = html_escape(domain),
    )
}
