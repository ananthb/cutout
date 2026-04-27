//! HTML templates for the management UI.
//!
//! Layout: a "pipeline workbench": left pane lists routing rules as a
//! vertical pipeline (top-to-bottom evaluation order), right pane is an
//! inspector for the selected rule. Edit/add forms render as overlays.

use crate::bots::EnabledChannels;
use crate::db::MessageListItem;
use crate::helpers::html_escape;
use crate::manage::viewer::RenderedEmail;
use crate::stats::Stats7d;
use crate::types::{Action, Destination, PendingDispatch, Rule, ViewerAuth};
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
  /* Lock the viewport so iOS/Android address-bar offsets don't push
     the live-feed (or selected inspector) below the fold. The shell
     uses dvh below to claim the actually-visible viewport. */
  overflow: hidden;
  overscroll-behavior: none;
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
  /* `dvh` follows the visible viewport as the mobile browser chrome
     (address bar, toolbar) shows and hides; `vh` is the static fallback
     for browsers without dvh support. */
  height: 100vh;
  height: 100dvh;
  overflow-x: hidden;
}
.topbar {
  display: flex; align-items: center; gap: 16px;
  padding: 10px 18px;
  background: var(--bg-1);
  border-bottom: 1px solid var(--line);
}
.topbar .brand { display: flex; align-items: center; gap: 14px; flex-shrink: 0; }
.topbar .brand .title { display: flex; flex-direction: column; line-height: 1.1; }
.topbar .brand .title b { font-weight: 600; font-size: 14px; }
.topbar .brand .title small { font-family: var(--font-mono); font-size: 10.5px; color: var(--fg-2); }
.topbar .right { display: flex; align-items: center; gap: 12px; font-size: 11.5px; color: var(--fg-2); flex-shrink: 0; }
.topbar .right .user { font-family: var(--font-mono); }
.microstats { display: flex; align-items: center; gap: 14px; }
.microstat { display: flex; flex-direction: column; line-height: 1.1; gap: 1px; }
.microstat .v {
  font-size: 14px; font-weight: 600;
  font-family: var(--font-mono);
}
.microstat .v.fwd  { color: var(--accent); }
.microstat .v.str  { color: var(--info); }
.microstat .v.drp  { color: var(--bad); }
.microstat .v.muted { color: var(--fg-3); }
.microstat .k {
  font-family: var(--font-mono);
  font-size: 9.5px; color: var(--fg-2);
  text-transform: uppercase; letter-spacing: 0.06em;
}

/* top senders ticker tape, sits in the topbar gap. Label stays in
   normal flow on the left; only the scrolling track gets the edge
   mask. Doubled track scrolls left at a slow constant rate and
   pauses on hover so an operator can read it. */
.topbar-ticker {
  flex: 1; min-width: 0;
  display: flex; align-items: center; gap: 10px;
  height: 22px;
}
.topbar-ticker .label {
  font-family: var(--font-mono); font-size: 9.5px;
  text-transform: uppercase; letter-spacing: 0.08em;
  color: var(--fg-3); flex-shrink: 0;
}
.topbar-ticker .ticker-window {
  flex: 1; min-width: 0; overflow: hidden;
  -webkit-mask-image: linear-gradient(to right, transparent, black 4%, black 96%, transparent);
          mask-image: linear-gradient(to right, transparent, black 4%, black 96%, transparent);
}
.topbar-ticker .track {
  display: flex; align-items: center; gap: 24px;
  white-space: nowrap;
  animation: topbar-ticker-roll 24s linear infinite;
  will-change: transform;
}
.topbar-ticker .ticker-window:hover .track { animation-play-state: paused; }
.topbar-ticker .item {
  display: inline-flex; align-items: baseline; gap: 6px;
  font-family: var(--font-mono); font-size: 11px;
}
.topbar-ticker .item .n { color: var(--accent); font-weight: 600; }
.topbar-ticker .item .addr { color: var(--fg-2); }
@keyframes topbar-ticker-roll {
  0%   { transform: translateX(0); }
  100% { transform: translateX(-50%); }
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
  overflow: hidden;
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
.pipeline-tester {
  flex-shrink: 0;
  padding: 12px 14px 12px;
  border-bottom: 1px dashed var(--line-2);
}
.pipeline-tester .card { background: var(--bg); }
.pipeline-tester .card > header {
  padding: 8px 10px; min-height: 0;
}
.pipeline-tester .card > header h3 { font-size: 12px; }
.pipeline-tester .card > .card-body { padding: 8px 10px 12px; gap: 8px; }
.pipeline-list {
  flex: 1; min-height: 0; overflow-y: auto;
  padding: 16px 14px;
}
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
.pipeline-node.selectable {
  text-decoration: none;
  cursor: pointer;
  transition: border-color 0.15s, box-shadow 0.15s, background 0.15s;
}
.pipeline-node.selectable:hover {
  border-color: var(--line);
  background: var(--bg-1);
  text-decoration: none;
}
.pipeline-node.selectable.selected {
  border-style: solid;
  border-color: var(--accent);
  background: color-mix(in oklch, var(--accent) 8%, var(--bg-1));
  color: var(--accent);
  box-shadow: 0 0 0 3px color-mix(in oklch, var(--accent) 18%, transparent);
}
.pipeline-node.selectable.selected .dot { background: var(--accent); }
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
.rule-card .order.store { background: color-mix(in oklch, var(--info) 12%, var(--bg-1)); color: var(--info); }
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

/* tester-driven evaluation animation. While the tester input parses to an
   address, the firing card pulses with an accent halo and a soft sweep;
   non-firing cards dim slightly so the eye lands on the firing one. */
.rule-card.tester-firing {
  border-color: var(--accent);
  box-shadow:
    0 0 0 3px color-mix(in oklch, var(--accent) 25%, transparent),
    0 0 22px color-mix(in oklch, var(--accent) 32%, transparent);
  animation: rule-firing-pulse 1.6s ease-in-out infinite;
  z-index: 2;
}
.rule-card.tester-firing::after {
  content: ""; position: absolute; inset: 0; border-radius: inherit;
  pointer-events: none;
  background: linear-gradient(120deg,
    transparent 30%,
    color-mix(in oklch, var(--accent) 18%, transparent) 50%,
    transparent 70%);
  background-size: 220% 100%;
  animation: rule-firing-sweep 2.4s linear infinite;
  mix-blend-mode: plus-lighter;
}
.rule-card.tester-dim { opacity: 0.55; }
@keyframes rule-firing-pulse {
  0%, 100% { box-shadow:
    0 0 0 3px color-mix(in oklch, var(--accent) 22%, transparent),
    0 0 18px color-mix(in oklch, var(--accent) 26%, transparent); }
  50%      { box-shadow:
    0 0 0 6px color-mix(in oklch, var(--accent) 30%, transparent),
    0 0 30px color-mix(in oklch, var(--accent) 45%, transparent); }
}
@keyframes rule-firing-sweep {
  0%   { background-position: 100% 0; }
  100% { background-position: -100% 0; }
}
@media (prefers-reduced-motion: reduce) {
  .rule-card.tester-firing { animation: none; }
  .rule-card.tester-firing::after { animation: none; opacity: 0.4; }
}

/* inspector pane ---------------------------------------------------- */
.inspector-pane { overflow: hidden; min-height: 0; display: flex; flex-direction: column; }
.inspector-rule-section {
  flex: 1; min-height: 0;
  display: flex; flex-direction: column;
  background: var(--bg-1);
}
.inspector-empty {
  flex: 1; display: flex; align-items: center; justify-content: center;
  padding: 32px;
}
.inspector-empty .hint {
  max-width: 360px; text-align: center;
  color: var(--fg-2); font-size: 13px; line-height: 1.6;
}
.inspector-empty .hint .arrow {
  display: block; font-family: var(--font-mono); font-size: 22px;
  color: var(--fg-3); margin-bottom: 8px;
}
.inspector-header {
  padding: 16px 24px; border-bottom: 1px solid var(--line);
  display: flex; align-items: flex-start; justify-content: space-between; gap: 16px;
}
/* Mobile-only "back to rules" link inside the inspector header. Hidden
   here on desktop; the mobile media query un-hides it. */
.inspector-back { display: none; }
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
.inspector-body {
  padding: 24px;
  display: flex; flex-direction: column; gap: 18px;
  flex: 1; min-height: 0; overflow: hidden;
}
.inspector-body > .messages-card {
  flex: 1; min-height: 240px;
  display: flex; flex-direction: column; overflow: hidden;
}
.inspector-body > .messages-card > .card-body {
  flex: 1; min-height: 0; overflow-y: auto;
}

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
.stat-strip .v.str { color: var(--info); }
.stat-strip .v.drp { color: var(--bad); }
.stat-strip .v.muted { color: var(--fg-2); }
.stat-strip .sub {
  font-family: var(--font-mono); font-size: 10.5px; color: var(--fg-3);
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
  position: relative;
}
.live-feed.collapsed { height: 36px; }
.live-feed.resizing { transition: none; user-select: none; }
.live-feed-handle {
  position: absolute; top: -3px; left: 0; right: 0;
  height: 6px;
  cursor: ns-resize;
  z-index: 5;
}
.live-feed-handle:hover,
.live-feed.resizing .live-feed-handle {
  background: color-mix(in oklch, var(--accent) 35%, transparent);
}
.live-feed.collapsed .live-feed-handle { display: none; }
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
.live-row.k-store .evt { color: var(--info); }
.live-row.k-drop .evt { color: var(--bad); }
.live-row.k-reply .evt { color: var(--info); }
.live-row.k-reject .evt { color: var(--warn); }
.live-row.k-error .evt { color: var(--bad); }
.live-row .err {
  grid-column: 1 / -1;
  font-size: 11px;
  color: var(--bad);
  margin-left: 88px;
  white-space: nowrap; overflow: hidden; text-overflow: ellipsis;
}
.live-feed-bar .pending-pill {
  margin-left: 8px;
  font-size: 11px;
  color: var(--bad);
  text-decoration: none;
}
.live-feed-bar .pending-pill[data-empty="1"] { display: none; }
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
.tag.store   { background: var(--info-soft); color: var(--info); }
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
.tip.below::after { bottom: auto; top: calc(100% + 6px); }
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
.dest-chip-mod {
  display: inline-block; padding: 0 4px; margin-left: 4px;
  border-radius: 3px; font-size: 9.5px; font-weight: 600;
  letter-spacing: 0.04em; text-transform: uppercase;
  background: color-mix(in oklch, currentColor 18%, transparent);
}
.dest-card-mod {
  display: inline-block; margin-left: auto; padding: 2px 6px;
  border-radius: 3px; font-size: 9.5px; font-weight: 600;
  letter-spacing: 0.04em; text-transform: uppercase;
  background: var(--bg-inset); color: var(--fg-2);
}
.chip-input-wrap { position: relative; flex: 1; min-width: 180px; display: flex; }
.chip-input-wrap .chip-input { flex: 1; min-width: 0; }
.chip-suggest {
  position: absolute; left: 0; right: 0; top: 100%;
  margin-top: 4px; z-index: 20;
  background: var(--bg); border: 1px solid var(--line);
  border-radius: var(--r-sm); box-shadow: 0 6px 14px rgba(0,0,0,0.12);
  max-height: 220px; overflow-y: auto;
}
.chip-suggest-item {
  display: flex; justify-content: space-between; align-items: baseline;
  padding: 6px 10px; cursor: pointer; font-size: 12px;
  font-family: var(--font-mono); color: var(--fg-1);
}
.chip-suggest-item.active,
.chip-suggest-item:hover { background: var(--bg-inset); color: var(--fg); }
.chip-suggest-hint { color: var(--fg-3); font-size: 11px; }

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

/* per-rule stored emails ------------------------------------------ */
.rule-messages {
  list-style: none; margin: 0; padding: 0;
  display: flex; flex-direction: column;
  border-top: 1px solid var(--line);
}
.rule-message { border-bottom: 1px solid var(--line); }
.rule-message-row {
  display: grid;
  grid-template-columns: 88px minmax(0, 1.2fr) minmax(0, 2fr) 18px;
  gap: 12px; align-items: baseline;
  width: 100%; padding: 8px 12px;
  background: transparent; border: 0; text-align: left;
  font: inherit; color: var(--fg-1); cursor: pointer;
}
.rule-message-row:hover { background: var(--bg-1); }
.rule-message-row .ts {
  font-family: var(--font-mono); font-size: 11px; color: var(--fg-2);
  white-space: nowrap;
}
.rule-message-row .from {
  font-size: 12.5px; color: var(--fg-1);
  overflow: hidden; text-overflow: ellipsis; white-space: nowrap;
}
.rule-message-row .subj {
  font-size: 12.5px; color: var(--fg-2);
  overflow: hidden; text-overflow: ellipsis; white-space: nowrap;
}
.rule-message-row .caret {
  color: var(--fg-2); font-size: 11px;
  transition: transform 120ms ease;
}
.rule-message-row .caret.open { transform: rotate(90deg); }
.rule-message-body { background: var(--bg-1); padding: 12px; }
.rule-message-body .embed-header {
  display: flex; flex-direction: column; gap: 4px;
  padding: 10px 12px; margin-bottom: 8px;
  background: var(--bg); border: 1px solid var(--line); border-radius: 8px;
}
.rule-message-body .embed-header .row { display: flex; gap: 10px; font-size: 12px; line-height: 1.5; }
.rule-message-body .embed-header .row .k {
  width: 48px; flex-shrink: 0;
  color: var(--fg-2); text-transform: uppercase; letter-spacing: 0.4px;
  font-size: 10px; padding-top: 1px;
}
.rule-message-body .embed-header .row span:not(.k) { word-break: break-all; }
.rule-message-body iframe.email-body {
  width: 100%; min-height: 480px; max-height: 70vh;
  border: 1px solid var(--line); border-radius: 8px; background: #fff;
}
.rule-message-body pre.email-text {
  margin: 0; padding: 12px;
  background: var(--bg); border: 1px solid var(--line); border-radius: 8px;
  white-space: pre-wrap; font-family: var(--font-mono); font-size: 12px; line-height: 1.55;
  max-height: 70vh; overflow: auto;
}
.rule-message-body .embed-footer { margin-top: 8px; display: flex; justify-content: flex-end; }
[x-cloak] { display: none !important; }

/* sticky banner shown when a CRUD save loses a concurrency race;
   triggered by `HX-Trigger: rule-conflict` from the server. */
.conflict-banner {
  position: fixed; left: 50%; bottom: 24px;
  transform: translateX(-50%);
  z-index: 1000; max-width: 540px;
  display: flex; align-items: center; gap: 12px;
  padding: 12px 14px;
  background: color-mix(in oklch, var(--bad) 14%, var(--bg-1));
  border: 1px solid color-mix(in oklch, var(--bad) 50%, var(--line));
  border-radius: var(--r-md);
  box-shadow: 0 12px 28px rgba(0,0,0,0.12);
  font-size: 12.5px;
}
.conflict-banner .msg { flex: 1; line-height: 1.45; }
.conflict-banner .msg strong { display: block; color: var(--bad); margin-bottom: 2px; }
.conflict-banner .conflict-dismiss { font-size: 16px; line-height: 1; padding: 4px 8px; }

/* misc ------------------------------------------------------------ */
.empty {
  padding: 28px; color: var(--fg-2); font-size: 12.5px;
  text-align: center;
}
::-webkit-scrollbar { width: 10px; height: 10px; }
::-webkit-scrollbar-thumb { background: var(--line-2); border-radius: 6px; border: 2px solid var(--bg); }
::-webkit-scrollbar-track { background: transparent; }

@media (max-width: 880px) {
  /* topbar: brand + microstats + right cluster on the first row, ticker
     drops to its own full-width row underneath. Hide the "routing
     pipeline" subtitle to make room for microstats next to the brand. */
  .topbar {
    flex-wrap: wrap;
    padding: 8px 12px;
    column-gap: 12px; row-gap: 6px;
  }
  .topbar .brand .title small { display: none; }
  .topbar .brand { gap: 10px; }
  .microstats { gap: 10px; }
  .microstat .v { font-size: 13px; }
  .topbar-ticker {
    order: 99;
    flex: 1 1 100%;
    min-width: 0; height: 20px;
    padding-top: 4px;
    border-top: 1px dashed var(--line-2);
  }

  /* workbench shows ONE pane at a time on mobile, full-screen.
     - no rule selected: pipeline + tester own the viewport.
     - rule selected:    inspector owns the viewport, pipeline hides.
     The visible pane gets the entire 1fr row. */
  .workbench {
    grid-template-columns: 1fr;
    grid-template-rows: minmax(0, 1fr);
  }
  .workbench.no-selection .inspector-pane { display: none; }
  .workbench.has-selection .pipeline-pane { display: none; }

  /* Pipeline becomes a single scroll container so the tester scrolls
     away with the rules instead of staying pinned at the top. */
  .pipeline-pane { border-right: none; overflow-y: auto; }
  .pipeline-tester {
    border-bottom: 1px solid var(--line);
    padding: 12px 14px;
  }
  .pipeline-list { overflow: visible; flex: 0 0 auto; }

  /* Inspector tweaks: stack the header so the back link sits on its
     own line, tighten body padding, and let the whole body scroll as
     one unit (the desktop pattern of "messages card grows + scrolls
     internally" clips on a small viewport because the other cards
     plus the 240px floor on the messages card add up to more than
     the inspector's height). */
  .inspector-header { flex-direction: column; align-items: stretch; }
  .inspector-back { display: inline-flex; align-self: flex-start; }
  .inspector-body {
    padding: 16px 14px; gap: 14px;
    overflow-y: auto;
  }
  .inspector-body > .messages-card {
    flex: 0 0 auto; min-height: 0;
  }
  .inspector-body > .messages-card > .card-body { overflow: visible; }
  .stat-strip { grid-template-columns: repeat(2, 1fr); }

  /* The live-feed drag handle is 6px tall by default which is unusable
     with a finger. Bump the touch target on mobile and give it a faint
     visible bar so users know it's draggable. */
  .live-feed-handle {
    height: 16px; top: -8px;
  }
  .live-feed-handle::after {
    content: ""; display: block;
    position: absolute; left: 50%; top: 50%;
    transform: translate(-50%, -50%);
    width: 36px; height: 4px; border-radius: 999px;
    background: var(--line-2);
  }

  /* live feed bar: trim the filter chips (low value on mobile) so the
     pending pill + event count survive on one row. */
  .live-feed { height: 180px; }
  .live-feed-bar { gap: 8px; padding: 0 10px; }
  .live-feed-bar .filters { display: none; }

  /* live-row: drop fixed-width columns that caused the horizontal
     overflow; show ts / kind / from only and rely on the live-feed bar
     "N events" counter for the size signal. */
  .live-feed-body { padding: 4px 10px 8px; }
  .live-row {
    grid-template-columns: 60px 60px minmax(0, 1fr);
    column-gap: 8px;
    padding: 5px 0;
    border-bottom: 1px dashed var(--line-2);
  }
  .live-row .arrow,
  .live-row > .addr ~ .addr,
  .live-row .chs,
  .live-row .size { display: none; }
  .live-row .err { margin-left: 0; }

  /* pipeline-card reorder buttons live behind a hover state on desktop;
     touch can't hover, so always show them on mobile. */
  .rule-card .move-tools { opacity: 1; pointer-events: auto; }

  /* conflict banner: full-width on narrow screens instead of clipped
     center column. */
  .conflict-banner {
    left: 12px; right: 12px; bottom: 12px;
    transform: none; max-width: none;
    flex-wrap: wrap;
  }
  .conflict-banner .msg { flex-basis: 100%; }
}
"##;

/// Alpine.js component factory for the rule editor modal: owns form state
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
    local: initial.local || '*',
    domain: initial.domain || '*',
    suggestions: [],
    sugIdx: -1,
    recentChats: null,    // null = not fetched; [] = fetched, empty
    chipLabel(c) {
      const base = c.kind + ':' + c.value;
      if (c.kind === 'email' && c.proxy) return base + ':proxy';
      if (c.auth && c.auth !== 'access') return base + ':' + c.auth;
      return base;
    },
    autoLabel() {
      const isCatch = this.local === '*' && this.domain === '*';
      if (this.action === 'drop' && isCatch) return 'Catch-all';
      const prefix = isCatch ? 'Catch-all' : (this.local + '@' + this.domain);
      let suffix;
      if (this.action === 'drop') suffix = 'drop';
      else if (this.action === 'store') suffix = 'store';
      else if (this.chips.length === 0) suffix = 'forward (no destinations)';
      else {
        const counts = { email: 0, telegram: 0, discord: 0 };
        this.chips.forEach(c => { counts[c.kind] = (counts[c.kind] || 0) + 1; });
        const labels = { email: 'Email', telegram: 'Telegram', discord: 'Discord' };
        suffix = ['email', 'telegram', 'discord']
          .filter(k => counts[k] > 0)
          .map(k => counts[k] === 1 ? labels[k] : labels[k] + ' \u00d7' + counts[k])
          .join(' + ');
      }
      return prefix + ' \u2192 ' + suffix;
    },
    serialize() {
      return this.chips.map(c => this.chipLabel(c)).join('\n');
    },
    parse(raw) {
      const text = raw.trim();
      if (!text) return { empty: true };
      const idx = text.indexOf(':');
      if (idx < 0) return { err: "use 'kind:value' (e.g. email:you@example.com)" };
      const kindIn = text.slice(0, idx).trim().toLowerCase();
      const rest   = text.slice(idx + 1).trim();
      if (!rest) return { err: "value missing after ':'" };
      const alias = { email: 'email', telegram: 'telegram', tg: 'telegram', discord: 'discord', dc: 'discord' };
      const kind = alias[kindIn];
      if (!kind) return { err: "unknown kind (use email, telegram, or discord)" };
      if (this.enabled.indexOf(kind) < 0) return { err: kind + " is not enabled on this deployment" };
      if (kind === 'email') {
        // Mirror Rust's `rsplit_once(':')` behavior. A non-empty trailing
        // segment after the last ':' is treated as the modifier and must
        // be 'proxy'.
        const lastColon = rest.lastIndexOf(':');
        let value, modifier = '';
        if (lastColon >= 0) {
          const candidate = rest.slice(lastColon + 1).trim();
          if (candidate !== '') {
            modifier = candidate;
            value = rest.slice(0, lastColon).trim();
          } else {
            value = rest;
          }
        } else {
          value = rest;
        }
        if (!value.includes('@') || value.startsWith('@') || value.endsWith('@'))
          return { err: "email address must contain '@'" };
        const m = modifier.toLowerCase();
        let proxy = false;
        if (m === 'proxy') proxy = true;
        else if (m !== '') return { err: "email modifier must be 'proxy'" };
        return { kind, value: value.toLowerCase(), proxy };
      }
      // Chat kinds: optional ':access' or ':token' suffix.
      let value = rest, auth = 'access';
      const subIdx = rest.indexOf(':');
      if (subIdx >= 0) {
        value = rest.slice(0, subIdx).trim();
        const suffix = rest.slice(subIdx + 1).trim().toLowerCase();
        if (suffix === '' || suffix === 'access') auth = 'access';
        else if (suffix === 'token' || suffix === 'public') auth = 'token';
        else return { err: "link auth must be 'access' or 'token'" };
      }
      if (kind === 'telegram') {
        if (!/^-?\d+$/.test(value)) return { err: "telegram chat_id must be an integer" };
        return { kind, value, auth };
      }
      if (kind === 'discord') {
        if (!/^\d+$/.test(value)) return { err: "discord channel_id must be a positive integer" };
        return { kind, value, auth };
      }
      return { err: "unknown kind" };
    },
    commit() {
      const r = this.parse(this.draft);
      if (r.empty) { this.err = ''; return true; }
      if (r.err)   { this.err = r.err; return false; }
      const chip = { kind: r.kind, value: r.value };
      if (r.proxy) chip.proxy = true;
      if (r.auth) chip.auth = r.auth;
      this.chips.push(chip);
      this.draft = ''; this.err = ''; this.suggestions = []; this.sugIdx = -1;
      return true;
    },
    onSubmit(e) {
      if (this.action === 'forward' && this.draft && !this.commit()) e.preventDefault();
    },
    // ---------- autocomplete ----------
    onChipKey(e) {
      // Snippet expansion when draft is empty: t/e/d -> kind:.
      if (!this.draft && e.key.length === 1) {
        const ch = e.key.toLowerCase();
        const map = { t: 'telegram', e: 'email', d: 'discord' };
        if (map[ch] && this.enabled.indexOf(map[ch]) >= 0) {
          e.preventDefault();
          this.draft = map[ch] + ':';
          this.refreshSuggestions();
          return;
        }
      }
      // Suggestion navigation.
      if (this.suggestions.length > 0) {
        if (e.key === 'ArrowDown') { e.preventDefault(); this.sugIdx = (this.sugIdx + 1) % this.suggestions.length; return; }
        if (e.key === 'ArrowUp')   { e.preventDefault(); this.sugIdx = (this.sugIdx - 1 + this.suggestions.length) % this.suggestions.length; return; }
        if (e.key === 'Tab' && this.sugIdx >= 0) { e.preventDefault(); this.applySuggestion(this.suggestions[this.sugIdx]); return; }
        if (e.key === 'Escape') { this.suggestions = []; this.sugIdx = -1; return; }
      }
      if (e.key === 'Enter') {
        if (this.sugIdx >= 0 && this.suggestions[this.sugIdx]) {
          e.preventDefault();
          this.applySuggestion(this.suggestions[this.sugIdx]);
          return;
        }
        e.preventDefault(); this.commit(); return;
      }
      if (e.key === ',' || e.key === ' ') {
        if (this.draft.trim()) { e.preventDefault(); this.commit(); return; }
      }
      if (e.key === 'Backspace' && !this.draft && this.chips.length) { this.chips.pop(); return; }
      // Anything else: queue a refresh on next tick (after x-model updates).
      this.err = '';
      queueMicrotask(() => this.refreshSuggestions());
    },
    refreshSuggestions() {
      const d = this.draft;
      const colon1 = d.indexOf(':');
      // Before the first ':', no suggestions yet (snippet expansion handles this).
      if (colon1 < 0) { this.suggestions = []; this.sugIdx = -1; return; }
      const kindRaw = d.slice(0, colon1).toLowerCase();
      const alias = { email: 'email', telegram: 'telegram', tg: 'telegram', discord: 'discord', dc: 'discord' };
      const kind = alias[kindRaw];
      if (!kind) { this.suggestions = []; this.sugIdx = -1; return; }
      const rest = d.slice(colon1 + 1);
      const colon2 = rest.indexOf(':');
      // value-position completion
      if (colon2 < 0) {
        if (kind === 'telegram') {
          this.ensureRecentChats();
          const q = rest.trim().toLowerCase();
          const matches = (this.recentChats || []).filter(c => !q || c.chat_id.includes(q) || (c.label || '').toLowerCase().includes(q));
          this.suggestions = matches.map(c => ({ insert: 'telegram:' + c.chat_id, label: c.label || c.chat_id, hint: 'telegram:' + c.chat_id }));
          this.sugIdx = this.suggestions.length ? 0 : -1;
        } else {
          this.suggestions = []; this.sugIdx = -1;
        }
        return;
      }
      // modifier-position completion (after second ':')
      const modRaw = rest.slice(colon2 + 1).toLowerCase();
      const value = rest.slice(0, colon2);
      const candidates = kind === 'email' ? ['proxy'] : ['token', 'access'];
      const filtered = candidates.filter(m => !modRaw || m.startsWith(modRaw));
      this.suggestions = filtered.map(m => ({ insert: kind + ':' + value + ':' + m, label: m, hint: ':' + m }));
      this.sugIdx = this.suggestions.length ? 0 : -1;
    },
    applySuggestion(s) {
      this.draft = s.insert;
      this.suggestions = []; this.sugIdx = -1;
    },
    ensureRecentChats() {
      if (this.recentChats !== null) return;
      this.recentChats = []; // mark as loading
      fetch('/manage/api/recent-telegram-chats', { credentials: 'same-origin' })
        .then(r => r.ok ? r.json() : [])
        .then(list => { this.recentChats = Array.isArray(list) ? list : []; this.refreshSuggestions(); })
        .catch(() => { this.recentChats = []; });
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
  // Persist the collapsed/filter/height state across navigation.
  // localStorage is per-origin, scoped to the manage host, so it follows
  // the operator's session naturally without server round-trips.
  const STORE_KEY = 'cutout.liveFeed.v1';
  const DEFAULT_H = 220;
  const MIN_H = 120;
  const maxH = () => Math.max(MIN_H, Math.floor(window.innerHeight * 0.8));
  const clampH = (h) => Math.max(MIN_H, Math.min(maxH(), h));
  let saved = {};
  try { saved = JSON.parse(localStorage.getItem(STORE_KEY) || '{}'); } catch (_) {}
  return {
    events: [],
    lastTs: 0,
    filter: typeof saved.filter === 'string' ? saved.filter : 'all',
    collapsed: saved.collapsed !== false,
    height: clampH(typeof saved.height === 'number' ? saved.height : DEFAULT_H),
    resizing: false,
    intervalId: null,
    pendingId: null,
    pending: { queued: 0, dead: 0 },
    save() {
      try {
        localStorage.setItem(STORE_KEY, JSON.stringify({
          collapsed: this.collapsed, filter: this.filter, height: this.height,
        }));
      } catch (_) {}
    },
    startResize(e) {
      if (this.collapsed) return;
      e.preventDefault();
      // Unified mouse + touch: read clientY from the right place and
      // bind the matching pair of move/up listeners. Without this the
      // handle is dead on touch devices.
      const isTouch = e.type === 'touchstart';
      const yOf = (ev) => isTouch ? ev.touches[0].clientY : ev.clientY;
      const startY = yOf(e);
      const startHeight = this.height;
      this.resizing = true;
      const moveEvt = isTouch ? 'touchmove' : 'mousemove';
      const endEvt  = isTouch ? 'touchend'  : 'mouseup';
      const onMove = (ev) => {
        // Drag up = bigger feed; drag down = smaller.
        const y = isTouch ? ev.touches[0].clientY : ev.clientY;
        this.height = clampH(startHeight + (startY - y));
        if (isTouch) ev.preventDefault();
      };
      const onUp = () => {
        this.resizing = false;
        this.save();
        document.removeEventListener(moveEvt, onMove);
        document.removeEventListener(endEvt, onUp);
      };
      document.addEventListener(moveEvt, onMove, isTouch ? { passive: false } : undefined);
      document.addEventListener(endEvt, onUp);
    },
    init() {
      this.pollPending();
      // Honour the persisted expanded state on page load.
      if (!this.collapsed) this.start();
      // Persist filter changes too.
      this.$watch('filter', () => this.save());
    },
    start() {
      if (this.intervalId) return;
      this.poll(true);
      this.intervalId = setInterval(() => this.poll(false), 2000);
      if (!this.pendingId) {
        this.pendingId = setInterval(() => this.pollPending(), 10000);
      }
    },
    stop() {
      if (this.intervalId) {
        clearInterval(this.intervalId);
        this.intervalId = null;
      }
      if (this.pendingId) {
        clearInterval(this.pendingId);
        this.pendingId = null;
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
      } catch (_) { /* swallow: polling will retry */ }
    },
    async pollPending() {
      try {
        const r = await fetch('/manage/pending/count', { credentials: 'same-origin' });
        if (!r.ok) return;
        const j = await r.json();
        this.pending.queued = j.queued || 0;
        this.pending.dead = j.dead_lettered || 0;
      } catch (_) { /* swallow */ }
    },
    toggle() {
      this.collapsed = !this.collapsed;
      this.save();
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
<title>{title} : Cutout</title>
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
<div id="rule-conflict-banner"
  x-data="{{ show: false, currentVersion: 0 }}"
  x-on:rule-conflict.window="show = true; currentVersion = ($event.detail && $event.detail.current_version) || 0"
  x-show="show" x-cloak class="conflict-banner">
  <div class="msg">
    <strong>Another operator just saved.</strong>
    <span>Your last action was rejected to avoid overwriting their changes. Reload to see the latest rules (now version <span x-text="currentVersion"></span>).</span>
  </div>
  <button class="btn primary sm" type="button" @click="window.location.reload()">Reload</button>
  <button class="btn ghost sm conflict-dismiss" type="button" @click="show = false" title="Dismiss">×</button>
</div>
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
  <div class="microstat"><span class="v str">{str}</span><span class="k">stored · 7d</span></div>
  <div class="microstat"><span class="v drp">{drp}</span><span class="k">dropped · 7d</span></div>
</div>"##,
            fwd = s.forwarded_total,
            str = s.stored_total,
            drp = s.dropped_total,
        ),
        None => String::new(),
    };
    let ticker = stats
        .map(|s| top_senders_ticker(&s.top_senders))
        .unwrap_or_default();
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
  {ticker}
  <div class="right">
    <span class="user">{email}</span>
    <a class="btn ghost sm" href="/cdn-cgi/access/logout"
       title="Sign out via Cloudflare Access">Log out</a>
  </div>
</header>"##,
        logo = LOGO_SVG,
        email = html_escape(email),
    )
}

/// Top senders rendered as a horizontally-scrolling marquee for the
/// topbar gap. The track is duplicated so the CSS animation can loop
/// seamlessly (translate from 0 to -50% repeats without a visible jump).
/// Nothing renders when there are no senders.
fn top_senders_ticker(senders: &[crate::stats::TopSender]) -> String {
    if senders.is_empty() {
        return String::new();
    }
    let one_pass: String = senders
        .iter()
        .map(|s| {
            format!(
                r##"<span class="item"><span class="n">{n}</span><span class="addr">{addr}</span></span>"##,
                n = s.n,
                addr = html_escape(&s.address),
            )
        })
        .collect::<Vec<_>>()
        .join("");
    format!(
        r##"<div class="topbar-ticker" title="Top senders · 7d (forwarded)">
  <span class="label">top senders · 7d</span>
  <div class="ticker-window">
    <div class="track" aria-hidden="false">{one_pass}{one_pass}</div>
  </div>
</div>"##,
    )
}

/// Full rules management page (the workbench).
pub fn rules_page(
    rules: &[Rule],
    rules_version: u64,
    email: &str,
    report: &Report,
    enabled: &EnabledChannels,
    selected_id: Option<&str>,
    stats: Option<&Stats7d>,
    messages: Option<&[MessageListItem]>,
) -> String {
    let selected_idx = pick_selected_idx(rules, selected_id);
    let workbench = workbench(
        rules,
        rules_version,
        report,
        enabled,
        selected_idx,
        stats,
        messages,
    );
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
const LIVE_FEED_PANE: &str = r##"<div class="live-feed"
  :class="{ collapsed, resizing }"
  :style="!collapsed ? ('height:' + height + 'px') : ''"
  x-data="liveFeed()" x-init="init()">
  <div class="live-feed-handle"
    @mousedown="startResize($event)"
    @touchstart="startResize($event)"
    title="Drag to resize"></div>
  <div class="live-feed-bar">
    <button class="btn ghost sm" @click="toggle()" type="button" style="padding:0 6px">
      <span x-text="collapsed ? '▸' : '▾'"></span>
      <span class="mono" style="text-transform:uppercase;letter-spacing:0.08em;font-size:11px">Live feed</span>
    </button>
    <span class="chip" :class="collapsed ? '' : 'ok'" style="height:20px;font-size:10.5px">
      <span class="dot" x-show="!collapsed"></span>
      <span x-text="collapsed ? 'paused: click to stream' : 'streaming'"></span>
    </span>
    <div class="filters" x-show="!collapsed">
      <template x-for="f in ['all','forward','drop','reply','reject','error']" :key="f">
        <button class="filter-chip" :class="{ active: filter === f }" @click="filter = f" x-text="f" type="button"></button>
      </template>
    </div>
    <a class="pending-pill" href="/manage/pending"
       :data-empty="(pending.queued + pending.dead) === 0 ? '1' : '0'">
      <span x-show="pending.queued > 0"><span x-text="pending.queued"></span> queued</span>
      <span x-show="pending.queued > 0 && pending.dead > 0" style="opacity:0.5"> · </span>
      <span x-show="pending.dead > 0"><span x-text="pending.dead"></span> DLQ</span>
    </a>
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
        <span class="err" x-show="e.error" :title="e.error" x-text="e.error"></span>
      </div>
    </template>
    <div x-show="visible().length === 0" class="empty">No events yet: they'll appear here as mail flows.</div>
  </div>
</div>"##;

/// Pick the index of the rule to show in the inspector. Returns `None` for
/// the overview view (default when no `?rule=` is supplied or the requested
/// id doesn't match any current rule).
pub fn pick_selected_idx(rules: &[Rule], requested: Option<&str>) -> Option<usize> {
    let id = requested?;
    rules.iter().position(|r| r.id == id)
}

/// Standalone SVG for the favicon: same mark as `LOGO_SVG`, but with
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
/// so HTMX endpoints can swap the whole region after CRUD. The hidden
/// `#workbench-form` carries `rules_version` (and `selected` when a
/// rule is open) so every CRUD button can `hx-include` it; `rules_version`
/// drives the optimistic-concurrency check on the server.
pub fn workbench(
    rules: &[Rule],
    rules_version: u64,
    report: &Report,
    enabled: &EnabledChannels,
    selected_idx: Option<usize>,
    stats: Option<&Stats7d>,
    messages: Option<&[MessageListItem]>,
) -> String {
    let pipeline = pipeline_pane(rules, selected_idx);
    let inspector = match selected_idx {
        Some(i) if i < rules.len() => inspector_pane(rules, i, report, enabled, stats, messages),
        _ => inspector_globals(rules, stats),
    };
    let selected_value = selected_idx
        .and_then(|i| rules.get(i).map(|r| r.id.as_str()))
        .unwrap_or("");
    // `has-selection` / `no-selection` lets the mobile CSS show only the
    // relevant pane full-screen instead of cramming both into half the
    // viewport.
    let sel_cls = if selected_idx.is_some() {
        "has-selection"
    } else {
        "no-selection"
    };
    format!(
        r##"<div id="workbench" class="workbench {sel_cls}">
{pipeline}
{inspector}
<form id="workbench-form" style="display:none">
  <input type="hidden" name="rules_version" value="{rules_version}">
  <input type="hidden" name="selected" value="{selected_value}">
</form>
</div>"##,
        selected_value = html_escape(selected_value),
    )
}

/// Inspector when no rule is selected: a soft hint pointing the user at
/// the pipeline. The tester now lives at the top of the pipeline pane and
/// top senders ride the topbar ticker, so this view stays out of the way.
fn inspector_globals(_rules: &[Rule], _stats: Option<&Stats7d>) -> String {
    r##"<section class="inspector-pane">
  <div class="inspector-empty">
    <div class="hint">
      <span class="arrow">←</span>
      Pick a rule from the pipeline to see its destinations, recent stats and the emails it has stored.
    </div>
  </div>
</section>"##
        .to_string()
}

/// HTMX-targeted response: same as `workbench`, plus an out-of-band
/// `#editor-modal` clear so any open modal closes after a successful CRUD.
pub fn workbench_response(
    rules: &[Rule],
    rules_version: u64,
    report: &Report,
    enabled: &EnabledChannels,
    selected_idx: Option<usize>,
    stats: Option<&Stats7d>,
    messages: Option<&[MessageListItem]>,
) -> String {
    let body = workbench(
        rules,
        rules_version,
        report,
        enabled,
        selected_idx,
        stats,
        messages,
    );
    format!("{body}\n<div id=\"editor-modal\" hx-swap-oob=\"true\"></div>")
}

/// Left pane: tester + pipeline of rule cards, top-to-bottom. The whole
/// pane shares one `tester(...)` Alpine scope so typing in the tester
/// input animates the matching rule card via `:class` bindings.
fn pipeline_pane(rules: &[Rule], selected_idx: Option<usize>) -> String {
    let cards: String = rules
        .iter()
        .enumerate()
        .map(|(i, r)| {
            let connector = r#"<div class="pipeline-connector"></div>"#;
            format!(
                "{connector}{}",
                pipeline_card(r, i, selected_idx == Some(i))
            )
        })
        .collect();
    let clear_btn = if selected_idx.is_some() {
        r##"<a class="btn ghost sm" href="/manage" title="Clear selection">Clear</a>"##
    } else {
        ""
    };
    let selected_id = selected_idx
        .and_then(|i| rules.get(i).map(|r| r.id.as_str()))
        .unwrap_or("");
    let init_attr = tester_init_attr(rules, selected_id);
    let tester = tester_card(selected_idx.is_some());
    format!(
        r##"<aside class="pipeline-pane" x-data='tester({init_attr})'>
  <header>
    <div>
      <h3>Routing pipeline</h3>
      <small>{n} {rule_word} · evaluated top → bottom</small>
    </div>
    <div style="display:flex;gap:6px;align-items:center">
      {clear_btn}
      <button class="btn primary sm"
        hx-get="/manage/rules/new"
        hx-target="#editor-modal"
        hx-swap="innerHTML">
        + Rule
      </button>
    </div>
  </header>
  <div class="pipeline-tester">{tester}</div>
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
    let is_store = matches!(rule.action, Action::Store { .. });
    let order_cls = if is_catch {
        "order catch"
    } else if is_fwd {
        "order fwd"
    } else if is_store {
        "order store"
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
        Action::Store { .. } => r#"<span class="tag store">store</span>"#.to_string(),
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
    hx-include="#workbench-form">↑</button>
  <button class="btn ghost icon sm" title="Move down"
    hx-post="/manage/rules/reorder" hx-vals='{down_vals}'
    hx-target="#workbench" hx-swap="outerHTML"
    hx-ext="json-enc"
    hx-include="#workbench-form">↓</button>
</div>"##,
        )
    };

    let href = if selected {
        "/manage".to_string()
    } else {
        format!("/manage?rule={}", html_escape(&rule.id))
    };
    let title = if selected {
        " title=\"Click to clear selection\""
    } else {
        ""
    };
    let id_e = html_escape(&rule.id);
    // Reactive class bindings consume the surrounding pipeline-pane's
    // tester() Alpine scope: this card glows when the typed address
    // resolves to it; siblings dim so the eye lands on the firing one.
    let alpine_class = format!(
        r##":class="{{ 'tester-firing': to.includes('@') && firstMatch() && firstMatch().id === '{id_e}', 'tester-dim': to.includes('@') && firstMatch() && firstMatch().id !== '{id_e}' }}""##,
    );
    format!(
        r##"<a href="{href}" class="{card_cls}" {alpine_class} data-rule-id="{id_e}"{title}>
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
        order = index + 1,
        label = html_escape(&rule.display_label()),
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
    messages: Option<&[MessageListItem]>,
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
        Action::Drop | Action::Store { .. } => 0,
    };
    let stat_strip = render_stat_strip(rule, dest_count, stats);
    let messages_card = rule_messages_card(&rule.id, messages.unwrap_or(&[]));

    let action_tag = match &rule.action {
        Action::Forward { destinations } => {
            let any_proxy = destinations
                .iter()
                .any(|d| matches!(d, Destination::Email { proxy: true, .. }));
            if any_proxy {
                r#"<span class="tag proxy tip below" data-tip="At least one email destination is in proxy mode: the message is reconstructed and Reply-To is rewritten so replies route back through the worker via the same custom domain. Strips PGP signatures and attachments on those destinations.">forward · proxy</span>"#.to_string()
            } else {
                r#"<span class="tag forward tip below" data-tip="Forward in native mode: uses Cloudflare EmailMessage.forward(): original bytes (PGP, attachments) pass through untouched; Reply-To is overlaid but may be ignored by some clients.">forward</span>"#.to_string()
            }
        }
        Action::Drop if is_catch => {
            r#"<span class="tag catch tip below" data-tip="Pinned catch-all rule: silently drops anything no earlier rule matched. Always sits at the end and can't be deleted or moved.">drop · catch-all</span>"#.to_string()
        }
        Action::Drop => r#"<span class="tag drop tip below" data-tip="Silently discard inbound mail matching this rule. No notification, no bounce.">drop</span>"#.to_string(),
        Action::Store { .. } => r#"<span class="tag store tip below" data-tip="Accept the email and record it. Optionally persist the message body to the database.">store</span>"#.to_string(),
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
  hx-include="#workbench-form">Delete</button>"##,
            id = html_escape(&rule.id),
        )
    };

    let destinations_card = match &rule.action {
        Action::Forward { destinations } => destinations_card(destinations),
        Action::Drop | Action::Store { .. } => String::new(),
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

    let action_summary_card = if is_fwd {
        String::new()
    } else {
        let (title, description) = match &rule.action {
            Action::Store { persist } => {
                let p = if *persist {
                    "Message body is persisted to the database."
                } else {
                    "Message body is NOT persisted."
                };
                ("Store", format!("Inbound mail matching this rule is recorded as a 'store' event. {p}"))
            }
            _ => ("Action", format!("Inbound mail matching this rule is silently dropped: no notification, no bounce. {}", if is_catch { "This is the pinned catch-all; it always sits at the end and can't be deleted or moved." } else { "" }))
        };
        format!(
            r##"<div class="card">
  <header><h3>{title}</h3></header>
  <div class="card-body">
    <p style="margin:0;color:var(--fg-2);font-size:12.5px">
      {description}
    </p>
  </div>
</div>"##,
        )
    };

    format!(
        r##"<section class="inspector-pane">
  <div class="inspector-rule-section">
    <div class="inspector-header">
      <a class="inspector-back btn ghost sm" href="/manage" title="Back to rules">← Rules</a>
      <div class="meta">
        <div class="id-row"><span class="tip below" data-tip="Stable random identifier (UUID v4) for this rule. Used in the URL when editing or deleting and in stats keyed by rule.">rule · {id}</span>{action_tag}</div>
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
      {messages_card}
    </div>
  </div>
</section>"##,
        id = html_escape(&rule.id),
        label = html_escape(&rule.display_label()),
        pattern = pattern_html(&rule.local_pattern, &rule.domain_pattern),
    )
}

/// Card wrapping the per-rule "Stored emails" list. The `messages-card`
/// class hooks into `.inspector-body` styling so this card grows to fill
/// remaining vertical space and scrolls internally instead of forcing
/// the whole inspector to scroll.
pub fn rule_messages_card(rule_id: &str, items: &[MessageListItem]) -> String {
    let count = items.len();
    let count_label = if count == 25 {
        "25+".to_string()
    } else {
        count.to_string()
    };
    let body = if items.is_empty() {
        r##"<div class="empty">No stored emails for this rule yet. Emails appear here when the rule stores them or forwards to a Telegram or Discord destination.</div>"##.to_string()
    } else {
        rule_messages_list(rule_id, items, false)
    };
    format!(
        r##"<div class="card messages-card">
  <header><h3>Stored emails</h3><small>{count_label}</small></header>
  <div class="card-body" style="padding:0">{body}</div>
</div>"##,
    )
}

/// HTML for the email list. When `append` is false, emits the wrapping
/// `<ul>` plus all rows; when true, emits only the rows so an HTMX
/// "Load more" can append into an existing `<ul>` via beforeend swap.
pub fn rule_messages_list(rule_id: &str, items: &[MessageListItem], append: bool) -> String {
    let rows: String = items
        .iter()
        .map(|m| render_message_row(rule_id, m))
        .collect();
    let oldest = items.last().and_then(|m| m.created_at.as_deref());
    let load_more = if items.len() == 25 {
        load_more_button(rule_id, oldest)
    } else {
        String::new()
    };
    if append {
        format!("{rows}\n{load_more}")
    } else {
        format!(
            r##"<ul class="rule-messages" id="rule-messages-{id}">
{rows}
</ul>
<div id="rule-messages-{id}-more">{load_more}</div>"##,
            id = html_escape(rule_id),
        )
    }
}

fn render_message_row(rule_id: &str, m: &MessageListItem) -> String {
    let url = format!(
        "/manage/rules/{}/messages/{}",
        html_escape(rule_id),
        html_escape(&m.id),
    );
    let ts_display = m.created_at.as_deref().unwrap_or("-");
    format!(
        r##"<li class="rule-message" x-data="{{ open: false }}">
  <button class="rule-message-row" type="button"
    @click="open = !open; if (open && !$refs.body.dataset.loaded) {{ htmx.ajax('GET','{url}',{{ target: $refs.body, swap: 'innerHTML' }}).then(() => {{ $refs.body.dataset.loaded = '1'; }}); }}">
    <span class="ts">{ts}</span>
    <span class="from">{from}</span>
    <span class="subj">{subj}</span>
    <span class="caret" :class="{{ open }}">▸</span>
  </button>
  <div class="rule-message-body" x-show="open" x-cloak x-ref="body"></div>
</li>"##,
        url = url,
        ts = html_escape(ts_display),
        from = html_escape(&m.sender),
        subj = html_escape(&m.subject),
    )
}

fn load_more_button(rule_id: &str, before: Option<&str>) -> String {
    let before = before.unwrap_or("");
    let encoded: String = before
        .bytes()
        .map(|b| match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                (b as char).to_string()
            }
            _ => format!("%{:02X}", b),
        })
        .collect();
    format!(
        r##"<div style="padding:10px;display:flex;justify-content:center">
  <button class="btn ghost sm"
    hx-get="/manage/rules/{rid}/messages?before={enc}"
    hx-target="#rule-messages-{rid}"
    hx-swap="beforeend"
    hx-on::after-request="this.parentElement.remove()">Load more</button>
</div>"##,
        rid = html_escape(rule_id),
        enc = encoded,
    )
}

/// Inline-expanded body fragment for one stored email. Reuses the
/// sandboxed iframe / sanitized HTML that `viewer::build_rendered`
/// produced; adds an "Open in new tab" footer link.
pub fn rule_message_fragment(message_id: &str, rendered: &RenderedEmail) -> String {
    format!(
        r##"<div class="embed-header">
  <div class="row"><span class="k">From</span><span>{from}</span></div>
  <div class="row"><span class="k">To</span><span>{to}</span></div>
  {date_line}
  <div class="row"><span class="k">Subject</span><span>{subject}</span></div>
</div>
{body}
<div class="embed-footer">
  <a class="btn ghost sm" href="/manage/m/{id}" target="_blank" rel="noopener">Open in new tab</a>
</div>"##,
        from = html_escape(&rendered.from),
        to = html_escape(&rendered.to),
        date_line = rendered.date_line,
        subject = html_escape(&rendered.subject),
        body = rendered.body_section,
        id = html_escape(message_id),
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
            r##"<div><span class="k tip" {m_tip}>matches · 7d</span><span class="v muted">-</span><span class="sub">{}</span></div>"##,
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
            r##"<div><span class="k tip" {l_tip}>last match</span><span class="v muted">-</span></div>"##
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
                "-".to_string()
            } else {
                parts.join(" · ")
            }
        }
        Action::Store { .. } | Action::Drop => "-".to_string(),
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

/// Render a "5m ago" / "3h ago" style relative time from a unix-second
/// timestamp, anchored to `now_ms` (unix milliseconds).
fn relative_time_from_seconds(ts_s: i64, now_ms: i64) -> String {
    if ts_s <= 0 {
        return "-".into();
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

/// "Destinations" card in the inspector. Per-destination modifiers (`proxy`
/// for email, `token` for chat) render as small badges next to each card.
fn destinations_card(destinations: &[Destination]) -> String {
    let body = if destinations.is_empty() {
        r#"<div class="empty">No destinations: this forward does nothing until you add at least one.</div>"#.to_string()
    } else {
        let cards: String = destinations
            .iter()
            .map(|d| {
                let kind = d.kind_label();
                let icon = channel_icon(kind);
                let modifier = match d {
                    Destination::Email { proxy: true, .. } => Some(("proxy", "Proxy mode: message is reconstructed and Reply-To is rewritten to a reverse alias. Replies route through the worker. Strips PGP signatures and attachments.")),
                    Destination::Telegram { link_auth: ViewerAuth::Token, .. }
                    | Destination::Discord { link_auth: ViewerAuth::Token, .. } => Some(("token", "Token mode: 'View full email' link is signed and shareable; viewer doesn't require Cloudflare Access.")),
                    _ => None,
                };
                let mod_badge = match modifier {
                    Some((label, tip)) => format!(
                        r##"<span class="dest-card-mod tip" data-tip="{tip}">{label}</span>"##,
                        tip = html_escape(tip),
                    ),
                    None => String::new(),
                };
                format!(
                    r##"<div class="dest-card {kind}">
  <span class="icon-wrap">{icon}</span>
  <div class="meta">
    <span class="kind">{kind}</span>
    <span class="value">{value}</span>
  </div>
  {mod_badge}
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

/// JSON init payload for the `tester(...)` Alpine factory. Pre-escaped
/// for use inside an HTML attribute (the `x-data='...'` wrapper lives on
/// the surrounding aside, see [`pipeline_pane`]).
fn tester_init_attr(all_rules: &[Rule], selected_id: &str) -> String {
    let rules_json: Vec<serde_json::Value> = all_rules
        .iter()
        .map(|r| {
            let action = match &r.action {
                Action::Drop => "drop",
                Action::Store { .. } => "store",
                Action::Forward { destinations } => {
                    let any_proxy = destinations
                        .iter()
                        .any(|d| matches!(d, Destination::Email { proxy: true, .. }));
                    if any_proxy {
                        "forward · proxy"
                    } else {
                        "forward"
                    }
                }
            };
            serde_json::json!({
                "id": r.id,
                "local": r.local_pattern,
                "domain": r.domain_pattern,
                "label": r.display_label(),
                "action": action,
            })
        })
        .collect();
    let init = serde_json::json!({
        "rules": rules_json,
        "selectedId": selected_id,
    })
    .to_string();
    html_escape(&init)
}

/// Interactive tester card. Mirrors the routing engine's glob matcher in
/// JS so each keystroke runs the full ruleset client-side and shows:
/// (a) does the *selected* rule's pattern match this address,
/// (b) which rule actually fires (top-down, first match wins),
/// (c) if a different rule fires earlier, link to it.
///
/// Lives at the top of the pipeline pane and shares its `tester(...)`
/// Alpine scope with the surrounding pipeline cards: typing in the input
/// drives the firing-card animation in [`pipeline_card`].
fn tester_card(has_selected: bool) -> String {
    let header_small = if has_selected {
        "this rule + full ruleset"
    } else {
        "full ruleset"
    };
    format!(
        r##"<div class="card tester-card">
  <header><h3>Tester</h3><small>{header_small}</small></header>
  <div class="card-body">
    <div class="field">
      <label>Test address</label>
      <input class="input" type="text"
        placeholder="shop@yourdomain.example"
        x-model="to" autocomplete="off" spellcheck="false">
    </div>
    <div class="tester-results" x-show="to" x-cloak>
      <div class="tester-row" x-show="selectedId">
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
          <span class="chip bad"><span class="dot"></span>no rule matches: bounced</span>
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
// Editor modal: used for both add and edit.
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
    <template x-for="(c, i) in chips" :key="i + ':' + chipLabel(c)">
      <span class="dest-chip" :class="'dest-' + c.kind">
        <span x-text="c.kind + ':' + c.value"></span>
        <template x-if="c.kind === 'email' && c.proxy">
          <span class="dest-chip-mod">proxy</span>
        </template>
        <template x-if="c.auth && c.auth !== 'access'">
          <span class="dest-chip-mod" x-text="c.auth"></span>
        </template>
        <button type="button" aria-label="remove" @click="chips.splice(i, 1)">×</button>
      </span>
    </template>
    <div class="chip-input-wrap">
      <input x-ref="chipInput"
        class="chip-input" type="text" x-model="draft"
        placeholder="type t / e / d to start"
        autocomplete="off" spellcheck="false"
        @keydown.window.escape="cutoutCloseModal()"
        @keydown="onChipKey($event)"
        @focus="refreshSuggestions()"
        @blur="setTimeout(() => {{ suggestions = []; sugIdx = -1; }}, 120)">
      <div class="chip-suggest" x-show="suggestions.length > 0" x-cloak>
        <template x-for="(s, i) in suggestions" :key="i + ':' + s.insert">
          <div class="chip-suggest-item" :class="{{ active: i === sugIdx }}"
            @mousedown.prevent="applySuggestion(s)">
            <span class="chip-suggest-label" x-text="s.label"></span>
            <span class="chip-suggest-hint" x-text="s.hint"></span>
          </div>
        </template>
      </div>
    </div>
    <input type="hidden" name="destinations" :value="serialize()">
  </div>
  <div class="chip-error" :class="{{ visible: !!err }}" x-text="err"></div>
</div>
<div class="help" style="margin-top:6px">Type <code>t</code>, <code>e</code>, or <code>d</code> to start an entry. Press space, comma, or enter to add. Each entry is <code>kind:value</code>. Available: {kinds_help}.{missing} Append <code>:proxy</code> on email to rewrite Reply-To, or <code>:token</code> on chat destinations to use a public signed-link viewer.</div>"##,
    )
}

/// New-rule modal.
pub fn new_rule_modal(enabled: &EnabledChannels) -> String {
    editor_modal(None, enabled, "/manage/rules", "post", "Create rule")
}

/// Edit-rule modal: returned for HTMX swap into `#editor-modal`.
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

    let (action_type, destinations, persist): (&str, &[Destination], bool) =
        match rule.map(|r| &r.action) {
            Some(Action::Forward { destinations }) => ("forward", destinations.as_slice(), false),
            Some(Action::Drop) => ("drop", &[], false),
            Some(Action::Store { persist }) => ("store", &[], *persist),
            None => ("forward", &[], false),
        };

    let title = if rule.is_some() {
        format!("Edit rule: {}", html_escape(id))
    } else {
        "New rule".to_string()
    };

    let dest_field = destinations_field(enabled);
    let persist_checked = if persist { " checked" } else { "" };
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
        .map(|d| {
            let mut obj = serde_json::json!({ "kind": d.kind_label(), "value": d.value() });
            if let Destination::Email { proxy: true, .. } = d {
                obj["proxy"] = serde_json::Value::Bool(true);
            }
            if let Some(auth) = d.link_auth() {
                obj["auth"] = serde_json::Value::String(auth.as_token().to_string());
            }
            obj
        })
        .collect();
    let init_json = serde_json::json!({
        "action": action_type,
        "chips": chips_json,
        "enabled": enabled_kinds,
        "local": local,
        "domain": domain,
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
    hx-include="#workbench-form"
    @submit="onSubmit($event)">
    <div class="modal-body">
      <div class="field">
        <label>Label <span class="help" style="text-transform:none;letter-spacing:0;font-family:var(--font-sans);font-size:11px;color:var(--fg-3);font-weight:400">optional</span></label>
        <input class="input" name="label" type="text" value="{label}" :placeholder="autoLabel()" style="font-family:var(--font-sans)">
      </div>
      <div class="field">
        <label>Pattern</label>
        <div class="pat-input">
          <input name="local_pattern" type="text" value="{local}" placeholder="*" required x-model="local"
            @keydown="if ($event.key === '@') {{ $event.preventDefault(); $refs.domainInput.focus(); $refs.domainInput.setSelectionRange(0, 0); }}">
          <span class="at">@</span>
          <input name="domain_pattern" x-ref="domainInput" type="text" value="{domain}" placeholder="*" required x-model="domain">
        </div>
        <span class="help"><span style="color:var(--accent)">*</span> matches anything: <span style="color:var(--accent)">?</span> matches one char. Type <code>@</code> to jump to the domain.</span>
      </div>
      <div class="field">
        <label>Action</label>
        <div class="action-toggle">
          <button type="button" :class="{{ active: action === 'forward' }}" @click="action = 'forward'">
            <strong>Forward</strong>
            <small>Send to one or more destinations</small>
          </button>
          <button type="button" :class="{{ active: action === 'store' }}" @click="action = 'store'">
            <strong>Store</strong>
            <small>Accept and record inbound mail</small>
          </button>
          <button type="button" :class="{{ active: action === 'drop' }}" @click="action = 'drop'">
            <strong>Drop</strong>
            <small>Silently discard inbound mail</small>
          </button>
        </div>
        <input type="hidden" name="action_type" :value="action">
      </div>
      <div class="field" x-show="action === 'store'">
        <label>Store options</label>
        <label style="display:flex;align-items:center;gap:6px;font-family:var(--font-sans);font-size:11.5px;text-transform:none;letter-spacing:0;color:var(--fg-1);cursor:pointer">
          <input type="checkbox" name="persist"{persist_checked}>
          Persist message body to database
        </label>
        <span class="help">If enabled, the parsed email content (subject, text, html) is saved to the <code>messages</code> table.</span>
      </div>
      <div class="field" x-show="action === 'forward'">
        <label>Destinations</label>
        {dest_field}
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

/// /manage/pending: a flat table of queued + dead-lettered rows. Lets the
/// operator see what's stuck in the retry pipeline and either kick a row
/// back into the queue or discard it.
pub fn pending_page(rows: &[PendingDispatch]) -> String {
    let body_rows = if rows.is_empty() {
        r#"<tr><td colspan="6" class="empty">No pending rows. Failed dispatches will appear here.</td></tr>"#.to_string()
    } else {
        rows.iter()
            .map(|r| {
                let kind = if r.dead_lettered { "dead" } else { "queued" };
                let kind_label = if r.dead_lettered { "DEAD-LETTERED" } else { "queued" };
                let err = r.last_error.as_deref().unwrap_or("");
                format!(
                    r##"<tr>
                        <td><span class="pstatus k-{k}">{kl}</span></td>
                        <td class="mono">{from} <span class="arrow">→</span> {to}</td>
                        <td class="mono">{attempts} attempts</td>
                        <td class="errcell" title="{err_full}">{err_short}</td>
                        <td>
                          <form method="post" action="/manage/pending/{id}/retry" style="display:inline">
                            <button class="btn sm" type="submit">Retry</button>
                          </form>
                          <form method="post" action="/manage/pending/{id}/discard" style="display:inline"
                                onsubmit="return confirm('Discard this email permanently?')">
                            <button class="btn sm danger" type="submit">Discard</button>
                          </form>
                        </td>
                    </tr>"##,
                    k = kind,
                    kl = kind_label,
                    from = html_escape(&r.sender),
                    to = html_escape(&r.recipient),
                    attempts = r.attempts,
                    err_full = html_escape(err),
                    err_short = html_escape(if err.len() > 200 { &err[..200] } else { err }),
                    id = html_escape(&r.id),
                )
            })
            .collect::<String>()
    };

    let content = format!(
        r##"<header class="topbar">
  <a href="/manage" class="brandwrap">
    <img src="/manage/assets/cutout-mark.svg" width="22" height="22" alt="">
    <span class="brand">Cutout</span>
  </a>
  <span class="title">Pending dispatches</span>
  <a href="/manage" class="btn ghost sm" style="margin-left:auto">← Back to rules</a>
</header>
<main class="pending-main">
  <p class="lede">Rows here are inbound emails that hit a dispatch failure. The retry queue
  re-runs them on a backoff; rows marked <strong>dead-lettered</strong> have exhausted retries
  and the original sender has been notified by DSN.</p>
  <table class="pending-table">
    <thead>
      <tr>
        <th>Status</th>
        <th>From → To</th>
        <th>Attempts</th>
        <th>Last error</th>
        <th>Action</th>
      </tr>
    </thead>
    <tbody>{body_rows}</tbody>
  </table>
</main>
<style>
.pending-main {{ max-width: 1100px; margin: 0 auto; padding: 24px 32px 60px; }}
.pending-main .lede {{ font-size: 13.5px; color: var(--fg-1); margin: 0 0 18px; max-width: 720px; line-height: 1.5; }}
.pending-table {{ width: 100%; border-collapse: collapse; font-size: 13px; }}
.pending-table th, .pending-table td {{
  padding: 10px 12px; text-align: left; border-bottom: 1px solid var(--line);
  vertical-align: top;
}}
.pending-table th {{
  font-family: var(--font-mono); text-transform: uppercase; font-size: 10.5px;
  letter-spacing: 0.06em; color: var(--fg-2);
}}
.pending-table td.errcell {{
  font-family: var(--font-mono); font-size: 11.5px; color: var(--bad);
  max-width: 360px; overflow: hidden; text-overflow: ellipsis;
}}
.pending-table .empty {{ text-align: center; color: var(--fg-2); padding: 40px; }}
.pstatus {{
  display: inline-block; padding: 2px 8px; border-radius: 999px;
  font-family: var(--font-mono); font-size: 10.5px; letter-spacing: 0.04em;
}}
.pstatus.k-queued {{ background: var(--info-soft); color: var(--info); }}
.pstatus.k-dead   {{ background: var(--bad-soft); color: var(--bad); }}
</style>"##,
        body_rows = body_rows,
    );
    base_html("Pending", &content)
}
