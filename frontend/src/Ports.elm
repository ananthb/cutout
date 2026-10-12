port module Ports exposing (Theme(..), setTheme, theme, zone)

{-| The only JavaScript this app talks to.

Everything else comes from `/api/*`. Two things genuinely can't:

  - **The theme.** `<html data-theme>` is outside the DOM subtree Elm owns, so
    it takes a port to set it (and to persist the choice in `localStorage`).
  - **The timezone offset.** `Time` in Elm is UTC-only; the viewer's offset
    arrives from `boot.js` so the live feed can print local wall-clock times.

-}

import Json.Encode as E


type Theme
    = Light
    | Dark
    | System


themeToString : Theme -> String
themeToString theme_ =
    case theme_ of
        Light ->
            "light"

        Dark ->
            "dark"

        System ->
            "system"


{-| Apply a theme to `<html>`. `boot.js` also persists it.
-}
setTheme : Theme -> Cmd msg
setTheme theme_ =
    E.string (themeToString theme_) |> applyTheme


port applyTheme : E.Value -> Cmd msg


{-| The theme stored from a previous visit, delivered once at boot.
-}
port theme : (String -> msg) -> Sub msg


{-| Minutes to add to local time to reach UTC (`Date#getTimezoneOffset`),
delivered once at boot.
-}
port zone : (Int -> msg) -> Sub msg
