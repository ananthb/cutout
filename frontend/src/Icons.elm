module Icons exposing (..)

{-| Inline SVG icons.

These use `elm/svg` rather than `Html.node "svg"`. That distinction matters:
`Html.node` creates elements in the _HTML_ namespace, so an `<svg>` built that
way is an unknown HTML element whose `<path>`/`<rect>` children never render —
the icons silently vanish. `Svg.svg` builds them in the SVG namespace.

Icons inherit `currentColor`, so a hover or a state class recolours them for
free.

-}

import Svg exposing (Svg, path, rect, svg)
import Svg.Attributes as SA


stroke : String -> List (Svg msg) -> Svg msg
stroke viewBox children =
    svg
        [ SA.viewBox viewBox
        , SA.width "1em"
        , SA.height "1em"
        , SA.fill "none"
        , SA.stroke "currentColor"
        , SA.strokeWidth "1.8"
        , SA.strokeLinecap "round"
        , SA.strokeLinejoin "round"
        , SA.class "icon"
        ]
        children


fill : String -> List (Svg msg) -> Svg msg
fill viewBox children =
    svg
        [ SA.viewBox viewBox
        , SA.width "1em"
        , SA.height "1em"
        , SA.fill "currentColor"
        , SA.class "icon"
        ]
        children


p : String -> Svg msg
p d =
    path [ SA.d d ] []



-- BRAND


{-| The Cutout mark: an outlined square overlapping a filled one, the overlap
"cut out". Inherits colour so it works on any surface.
-}
logo : Svg msg
logo =
    svg
        [ SA.viewBox "0 0 22 22"
        , SA.width "1em"
        , SA.height "1em"
        , SA.class "icon icon-logo"
        ]
        [ rect
            [ SA.x "1.5"
            , SA.y "1.5"
            , SA.width "13"
            , SA.height "13"
            , SA.rx "2.5"
            , SA.fill "none"
            , SA.stroke "currentColor"
            , SA.strokeWidth "1.7"
            ]
            []
        , rect
            [ SA.x "7.5"
            , SA.y "7.5"
            , SA.width "13"
            , SA.height "13"
            , SA.rx "2.5"
            , SA.fill "currentColor"
            ]
            []
        , rect
            [ SA.x "7.5"
            , SA.y "7.5"
            , SA.width "7"
            , SA.height "7"
            , SA.fill "var(--bg-1)"
            , SA.stroke "currentColor"
            , SA.strokeWidth "1.7"
            ]
            []
        ]



-- CHANNELS


mail : Svg msg
mail =
    stroke "0 0 16 16"
        [ rect
            [ SA.x "2"
            , SA.y "3.5"
            , SA.width "12"
            , SA.height "9"
            , SA.rx "1.5"
            ]
            []
        , p "M2.5 4.5l5.5 4 5.5-4"
        ]


telegram : Svg msg
telegram =
    fill "0 0 16 16"
        [ p "M14.4 2.3 1.7 7.2c-.9.3-.8 1.5.1 1.7l3 .8 1.2 3.7c.2.6.9.7 1.3.3l1.7-1.5 3.3 2.4c.6.4 1.4 0 1.5-.7l2-9.7c.2-.9-.7-1.5-1.4-1.2zm-2.3 3-5.3 4.7-.2 2.5-1-2.9 6.5-4.3z" ]


discord : Svg msg
discord =
    fill "0 0 16 16"
        [ p "M13 3.5a11 11 0 0 0-2.7-.8l-.2.5a9.7 9.7 0 0 0-4.2 0l-.2-.5a11 11 0 0 0-2.7.8C1 7 1 9.5 1 12c0 0 1.4 1 3.4 1.4l.5-.9a6 6 0 0 1-1.1-.5l.3-.2a8 8 0 0 0 7.8 0l.3.2a6 6 0 0 1-1.1.5l.5.9c2-.4 3.4-1.4 3.4-1.4 0-2.5 0-5-2-8.5zM6 10c-.6 0-1.1-.6-1.1-1.3 0-.7.5-1.3 1.1-1.3.7 0 1.2.6 1.1 1.3 0 .7-.5 1.3-1.1 1.3zm4 0c-.6 0-1.1-.6-1.1-1.3 0-.7.5-1.3 1.1-1.3.7 0 1.2.6 1.1 1.3 0 .7-.5 1.3-1.1 1.3z" ]



-- ACTIONS


plus : Svg msg
plus =
    stroke "0 0 16 16" [ p "M8 3v10M3 8h10" ]


arrowUp : Svg msg
arrowUp =
    stroke "0 0 16 16" [ p "M8 13V3M4 7l4-4 4 4" ]


arrowDown : Svg msg
arrowDown =
    stroke "0 0 16 16" [ p "M8 3v10M4 9l4 4 4-4" ]


trash : Svg msg
trash =
    stroke "0 0 16 16"
        [ p "M2.5 4.5h11M5.5 4.5V3.2a1 1 0 0 1 1-1h3a1 1 0 0 1 1 1v1.3M4 4.5l.6 8.2a1 1 0 0 0 1 .9h4.8a1 1 0 0 0 1-.9l.6-8.2" ]


pencil : Svg msg
pencil =
    stroke "0 0 16 16" [ p "M11 2.5l2.5 2.5-8 8L2.5 14l1-3 7.5-8.5z" ]


close : Svg msg
close =
    stroke "0 0 16 16" [ p "M3.5 3.5l9 9M12.5 3.5l-9 9" ]


chevronRight : Svg msg
chevronRight =
    stroke "0 0 16 16" [ p "M6 3.5l5 4.5-5 4.5" ]


chevronDown : Svg msg
chevronDown =
    stroke "0 0 16 16" [ p "M3.5 6l4.5 5 4.5-5" ]


play : Svg msg
play =
    fill "0 0 16 16" [ p "M4 2.8v10.4l9-5.2z" ]


pause : Svg msg
pause =
    stroke "0 0 16 16" [ p "M5.5 3v10M10.5 3v10" ]


external : Svg msg
external =
    stroke "0 0 16 16" [ p "M6.5 3.5H3.5v9h9v-3M9 3.5h4v4M13 3.5L7.5 9" ]


bolt : Svg msg
bolt =
    fill "0 0 16 16" [ p "M9 1 3 9h4l-1 6 6-8H8z" ]


check : Svg msg
check =
    stroke "0 0 16 16" [ p "M3.5 8.5l3 3 6-7" ]


alert : Svg msg
alert =
    stroke "0 0 16 16" [ p "M8 2.5l6 11H2zM8 6.5v3.5M8 12h.01" ]


info : Svg msg
info =
    stroke "0 0 16 16"
        [ p "M8 14.5A6.5 6.5 0 1 0 8 1.5a6.5 6.5 0 0 0 0 13z"
        , p "M8 7.5v4M8 4.8h.01"
        ]


clock : Svg msg
clock =
    stroke "0 0 16 16"
        [ p "M8 14.5A6.5 6.5 0 1 0 8 1.5a6.5 6.5 0 0 0 0 13z"
        , p "M8 4.5V8l2.5 1.5"
        ]


activity : Svg msg
activity =
    stroke "0 0 16 16" [ p "M1.5 8h3l2-5 3 10 2-5h3" ]


inbox : Svg msg
inbox =
    stroke "0 0 16 16"
        [ p "M2 9.5l1.8-6h8.4L14 9.5V13a1 1 0 0 1-1 1H3a1 1 0 0 1-1-1z"
        , p "M2 9.5h3.2l.8 1.6h4l.8-1.6H14"
        ]


refresh : Svg msg
refresh =
    stroke "0 0 16 16"
        [ p "M13 8a5 5 0 1 1-1.5-3.6M13 2v3h-3" ]


send : Svg msg
send =
    stroke "0 0 16 16" [ p "M14 8a6 6 0 1 1-1.8-4.2M14 2v3.5h-3.5" ]


sun : Svg msg
sun =
    stroke "0 0 16 16"
        [ p "M8 11a3 3 0 1 0 0-6 3 3 0 0 0 0 6z"
        , p "M8 1v1.5M8 13.5V15M15 8h-1.5M2.5 8H1M12.9 3.1l-1 1M4.1 11.9l-1 1M12.9 12.9l-1-1M4.1 4.1l-1-1"
        ]


moon : Svg msg
moon =
    stroke "0 0 16 16" [ p "M13.5 9.5A5.5 5.5 0 0 1 6.5 2.5 5.5 5.5 0 1 0 13.5 9.5z" ]


monitor : Svg msg
monitor =
    stroke "0 0 16 16"
        [ rect
            [ SA.x "2"
            , SA.y "3"
            , SA.width "12"
            , SA.height "8"
            , SA.rx "1.2"
            ]
            []
        , p "M6 14h4"
        ]


search : Svg msg
search =
    stroke "0 0 16 16"
        [ p "M7 12a5 5 0 1 0 0-10 5 5 0 0 0 0 10zM13.5 13.5l-3-3" ]


shield : Svg msg
shield =
    stroke "0 0 16 16" [ p "M8 1.5l5 1.5v4c0 3-2 6-5 7.5C5 13 3 10 3 7V3z" ]


zap : Svg msg
zap =
    stroke "0 0 16 16" [ p "M8.5 1.5L3 9h4l-1.5 5.5L12 7H8z" ]


lock : Svg msg
lock =
    stroke "0 0 16 16"
        [ rect [ SA.x "3.5", SA.y "7", SA.width "9", SA.height "6.5", SA.rx "1.2" ] []
        , p "M5.5 7V5a2.5 2.5 0 0 1 5 0v2"
        ]
