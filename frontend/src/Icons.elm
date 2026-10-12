module Icons exposing (..)

{-| Inline SVG icons.

Built with `Html.node "svg"` rather than `elm/svg` so the app keeps its
dependency list small; the few attributes that matter (`viewBox`, `stroke`,
`stroke-width`) are set with `attribute`. Icons inherit `currentColor`, so a
hover or a state class recolours them for free.

-}

import Html exposing (Html)
import Html.Attributes exposing (attribute, class)


stroke : String -> List (Html msg) -> Html msg
stroke viewBox children =
    Html.node "svg"
        [ attribute "viewBox" viewBox
        , attribute "width" "1em"
        , attribute "height" "1em"
        , attribute "fill" "none"
        , attribute "stroke" "currentColor"
        , attribute "stroke-width" "1.8"
        , attribute "stroke-linecap" "round"
        , attribute "stroke-linejoin" "round"
        , attribute "aria-hidden" "true"
        , class "icon"
        ]
        children


fill : String -> List (Html msg) -> Html msg
fill viewBox children =
    Html.node "svg"
        [ attribute "viewBox" viewBox
        , attribute "width" "1em"
        , attribute "height" "1em"
        , attribute "fill" "currentColor"
        , attribute "aria-hidden" "true"
        , class "icon"
        ]
        children


path : String -> Html msg
path d =
    Html.node "path" [ attribute "d" d ] []



-- BRAND


{-| The Cutout mark: an outlined square overlapping a filled one, the overlap
"cut out". Inherits colour so it works on any surface.
-}
logo : Html msg
logo =
    Html.node "svg"
        [ attribute "viewBox" "0 0 22 22"
        , attribute "width" "1em"
        , attribute "height" "1em"
        , attribute "aria-hidden" "true"
        , class "icon icon-logo"
        ]
        [ Html.node "rect"
            [ attribute "x" "1.5"
            , attribute "y" "1.5"
            , attribute "width" "13"
            , attribute "height" "13"
            , attribute "rx" "2.5"
            , attribute "fill" "none"
            , attribute "stroke" "currentColor"
            , attribute "stroke-width" "1.7"
            ]
            []
        , Html.node "rect"
            [ attribute "x" "7.5"
            , attribute "y" "7.5"
            , attribute "width" "13"
            , attribute "height" "13"
            , attribute "rx" "2.5"
            , attribute "fill" "currentColor"
            ]
            []
        , Html.node "rect"
            [ attribute "x" "7.5"
            , attribute "y" "7.5"
            , attribute "width" "7"
            , attribute "height" "7"
            , attribute "fill" "var(--bg-1)"
            , attribute "stroke" "currentColor"
            , attribute "stroke-width" "1.7"
            ]
            []
        ]



-- CHANNELS


mail : Html msg
mail =
    stroke "0 0 16 16"
        [ Html.node "rect"
            [ attribute "x" "2"
            , attribute "y" "3.5"
            , attribute "width" "12"
            , attribute "height" "9"
            , attribute "rx" "1.5"
            ]
            []
        , path "M2.5 4.5l5.5 4 5.5-4"
        ]


telegram : Html msg
telegram =
    fill "0 0 16 16"
        [ path "M14.4 2.3 1.7 7.2c-.9.3-.8 1.5.1 1.7l3 .8 1.2 3.7c.2.6.9.7 1.3.3l1.7-1.5 3.3 2.4c.6.4 1.4 0 1.5-.7l2-9.7c.2-.9-.7-1.5-1.4-1.2zm-2.3 3-5.3 4.7-.2 2.5-1-2.9 6.5-4.3z" ]


discord : Html msg
discord =
    fill "0 0 16 16"
        [ path "M13 3.5a11 11 0 0 0-2.7-.8l-.2.5a9.7 9.7 0 0 0-4.2 0l-.2-.5a11 11 0 0 0-2.7.8C1 7 1 9.5 1 12c0 0 1.4 1 3.4 1.4l.5-.9a6 6 0 0 1-1.1-.5l.3-.2a8 8 0 0 0 7.8 0l.3.2a6 6 0 0 1-1.1.5l.5.9c2-.4 3.4-1.4 3.4-1.4 0-2.5 0-5-2-8.5zM6 10c-.6 0-1.1-.6-1.1-1.3 0-.7.5-1.3 1.1-1.3.7 0 1.2.6 1.1 1.3 0 .7-.5 1.3-1.1 1.3zm4 0c-.6 0-1.1-.6-1.1-1.3 0-.7.5-1.3 1.1-1.3.7 0 1.2.6 1.1 1.3 0 .7-.5 1.3-1.1 1.3z" ]



-- ACTIONS


plus : Html msg
plus =
    stroke "0 0 16 16" [ path "M8 3v10M3 8h10" ]


arrowUp : Html msg
arrowUp =
    stroke "0 0 16 16" [ path "M8 13V3M4 7l4-4 4 4" ]


arrowDown : Html msg
arrowDown =
    stroke "0 0 16 16" [ path "M8 3v10M4 9l4 4 4-4" ]


trash : Html msg
trash =
    stroke "0 0 16 16"
        [ path "M2.5 4.5h11M5.5 4.5V3.2a1 1 0 0 1 1-1h3a1 1 0 0 1 1 1v1.3M4 4.5l.6 8.2a1 1 0 0 0 1 .9h4.8a1 1 0 0 0 1-.9l.6-8.2" ]


pencil : Html msg
pencil =
    stroke "0 0 16 16" [ path "M11 2.5l2.5 2.5-8 8L2.5 14l1-3 7.5-8.5z" ]


close : Html msg
close =
    stroke "0 0 16 16" [ path "M3.5 3.5l9 9M12.5 3.5l-9 9" ]


chevronRight : Html msg
chevronRight =
    stroke "0 0 16 16" [ path "M6 3.5l5 4.5-5 4.5" ]


chevronDown : Html msg
chevronDown =
    stroke "0 0 16 16" [ path "M3.5 6l4.5 5 4.5-5" ]


play : Html msg
play =
    fill "0 0 16 16" [ path "M4 2.8v10.4l9-5.2z" ]


pause : Html msg
pause =
    stroke "0 0 16 16" [ path "M5.5 3v10M10.5 3v10" ]


external : Html msg
external =
    stroke "0 0 16 16" [ path "M6.5 3.5H3.5v9h9v-3M9 3.5h4v4M13 3.5L7.5 9" ]


bolt : Html msg
bolt =
    fill "0 0 16 16" [ path "M9 1 3 9h4l-1 6 6-8H8z" ]


check : Html msg
check =
    stroke "0 0 16 16" [ path "M3.5 8.5l3 3 6-7" ]


alert : Html msg
alert =
    stroke "0 0 16 16" [ path "M8 2.5l6 11H2zM8 6.5v3.5M8 12h.01" ]


info : Html msg
info =
    stroke "0 0 16 16"
        [ path "M8 14.5A6.5 6.5 0 1 0 8 1.5a6.5 6.5 0 0 0 0 13z"
        , path "M8 7.5v4M8 4.8h.01"
        ]


clock : Html msg
clock =
    stroke "0 0 16 16"
        [ path "M8 14.5A6.5 6.5 0 1 0 8 1.5a6.5 6.5 0 0 0 0 13z"
        , path "M8 4.5V8l2.5 1.5"
        ]


activity : Html msg
activity =
    stroke "0 0 16 16" [ path "M1.5 8h3l2-5 3 10 2-5h3" ]


inbox : Html msg
inbox =
    stroke "0 0 16 16"
        [ path "M2 9.5l1.8-6h8.4L14 9.5V13a1 1 0 0 1-1 1H3a1 1 0 0 1-1-1z"
        , path "M2 9.5h3.2l.8 1.6h4l.8-1.6H14"
        ]


refresh : Html msg
refresh =
    stroke "0 0 16 16"
        [ path "M13 8a5 5 0 1 1-1.5-3.6M13 2v3h-3" ]


send : Html msg
send =
    stroke "0 0 16 16" [ path "M14 8a6 6 0 1 1-1.8-4.2M14 2v3.5h-3.5" ]


sun : Html msg
sun =
    stroke "0 0 16 16"
        [ path "M8 11a3 3 0 1 0 0-6 3 3 0 0 0 0 6z"
        , path "M8 1v1.5M8 13.5V15M15 8h-1.5M2.5 8H1M12.9 3.1l-1 1M4.1 11.9l-1 1M12.9 12.9l-1-1M4.1 4.1l-1-1"
        ]


moon : Html msg
moon =
    stroke "0 0 16 16" [ path "M13.5 9.5A5.5 5.5 0 0 1 6.5 2.5 5.5 5.5 0 1 0 13.5 9.5z" ]


monitor : Html msg
monitor =
    stroke "0 0 16 16"
        [ Html.node "rect"
            [ attribute "x" "2"
            , attribute "y" "3"
            , attribute "width" "12"
            , attribute "height" "8"
            , attribute "rx" "1.2"
            ]
            []
        , path "M6 14h4"
        ]


search : Html msg
search =
    stroke "0 0 16 16"
        [ path "M7 12a5 5 0 1 0 0-10 5 5 0 0 0 0 10zM13.5 13.5l-3-3" ]


shield : Html msg
shield =
    stroke "0 0 16 16" [ path "M8 1.5l5 1.5v4c0 3-2 6-5 7.5C5 13 3 10 3 7V3z" ]


zap : Html msg
zap =
    stroke "0 0 16 16" [ path "M8.5 1.5L3 9h4l-1.5 5.5L12 7H8z" ]
