// Boots the Elm app and bridges its two ports.
//
// This lives in a file rather than inline in the shell because the Worker's
// Content-Security-Policy is `script-src 'self'` — no inline script, no nonce.
// An external initialiser is what lets every page run under that one policy.
(function () {
  "use strict";

  var root = document.documentElement;

  // Apply the saved theme before Elm mounts, so the workbench renders on the
  // right surface. "system" follows the OS via the media query in tokens.css.
  var THEME_KEY = "cutout.theme";
  var theme = "system";
  try {
    var saved = window.localStorage.getItem(THEME_KEY);
    if (saved === "light" || saved === "dark" || saved === "system") theme = saved;
  } catch (_) {}
  root.dataset.theme = theme;

  if (!window.Elm || !window.Elm.Main) return;

  // Browser.application takes over <body> wholesale and removes whatever it
  // finds there, so clear it first rather than leaving a stray node.
  if (document.body) document.body.replaceChildren();

  var app = window.Elm.Main.init({});

  // A global, not a DOM marker: a DOM marker would be present before the
  // bundle runs and so couldn't prove the app is live.
  window.__cutoutBooted = true;

  if (!app.ports) return;

  // The theme port. Elm drives the toggle; this applies and persists it.
  if (app.ports.applyTheme) {
    app.ports.applyTheme.subscribe(function (value) {
      var v = value === "light" || value === "dark" ? value : "system";
      root.dataset.theme = v;
      try {
        window.localStorage.setItem(THEME_KEY, v);
      } catch (_) {}
    });
  }

  // Elm is UTC-only; hand it the viewer's offset so the live feed can print
  // local wall-clock times.
  if (app.ports.zone) {
    app.ports.zone.send(new Date().getTimezoneOffset());
  }

  // Tell Elm which theme was restored, so the toggle shows the right icon.
  if (app.ports.theme) {
    app.ports.theme.send(theme);
  }
})();
