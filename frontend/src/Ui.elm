module Ui exposing (Banner(..), ButtonVariant(..), banner, button, card, chip, emptyState, field, iconButton, link, modal, pill, remote, sectionTitle, segmented, spinner, stat, textarea, toggle)

{-| Shared view pieces.

Each is a thin wrapper over a class name that the stylesheets in `public/css/`
own. Keeping the markup here means the animation and layout rules live in CSS
and the Elm code stays about structure.

-}

import Api
import Html exposing (Html, a, button, div, h2, input, label, p, span, text)
import Html.Attributes exposing (attribute, autocomplete, class, classList, disabled, for, id, name, placeholder, rows, spellcheck, type_, value)
import Html.Events exposing (onClick, onInput)
import Icons
import RemoteData exposing (RemoteData(..))


type Banner
    = Error
    | Success
    | Info
    | Warn


bannerKind : Banner -> String
bannerKind b =
    case b of
        Error ->
            "error"

        Success ->
            "success"

        Info ->
            "info"

        Warn ->
            "warn"


bannerIcon : Banner -> Html msg
bannerIcon b =
    case b of
        Error ->
            Icons.alert

        Success ->
            Icons.check

        Info ->
            Icons.info

        Warn ->
            Icons.alert


{-| A message banner. Animates in on mount.
-}
banner : Banner -> String -> Html msg
banner kind message =
    div
        [ classList [ ( "banner", True ), ( bannerKind kind, True ) ]
        , attribute "role"
            (if kind == Error then
                "alert"

             else
                "status"
            )
        ]
        [ span [ class "banner-icon" ] [ bannerIcon kind ]
        , span [ class "banner-text" ] [ text message ]
        ]


spinner : Html msg
spinner =
    div [ class "spinner", attribute "role" "status", attribute "aria-label" "Loading" ]
        [ span [] [], span [] [], span [] [] ]


{-| Render remote data, handling all four states in one place.
-}
remote : Api.Data a -> (a -> Html msg) -> Html msg
remote data toHtml =
    case data of
        RemoteData.NotAsked ->
            spinner

        RemoteData.Loading ->
            spinner

        RemoteData.Failure err ->
            banner Error (Api.errorMessage err)

        RemoteData.Success value ->
            toHtml value


type ButtonVariant
    = Primary
    | Secondary
    | Ghost
    | Danger


variantClasses : ButtonVariant -> List ( String, Bool )
variantClasses variant =
    [ ( "btn", True )
    , ( "btn-primary", variant == Primary )
    , ( "btn-secondary", variant == Secondary )
    , ( "btn-ghost", variant == Ghost )
    , ( "btn-danger", variant == Danger )
    ]


button :
    { label : String, onClick : msg, variant : ButtonVariant, busy : Bool, disabled : Bool }
    -> Html msg
button config =
    Html.button
        [ classList (variantClasses config.variant ++ [ ( "is-busy", config.busy ) ])
        , onClick config.onClick
        , disabled (config.busy || config.disabled)
        , type_ "button"
        ]
        [ text
            (if config.busy then
                "Working…"

             else
                config.label
            )
        ]


iconButton :
    { icon : Html msg, label : String, onClick : msg, variant : ButtonVariant, disabled : Bool, small : Bool }
    -> Html msg
iconButton config =
    Html.button
        [ classList (variantClasses config.variant ++ [ ( "btn-icon", True ), ( "btn-sm", config.small ) ])
        , onClick config.onClick
        , disabled config.disabled
        , type_ "button"
        , attribute "title" config.label
        , attribute "aria-label" config.label
        ]
        [ config.icon ]


card : List (Html msg) -> Html msg
card =
    div [ class "card" ]


sectionTitle : String -> Html msg
sectionTitle title =
    h2 [ class "section-title" ] [ text title ]


{-| A labelled text input.
-}
field :
    { id : String
    , label : String
    , value : String
    , hint : String
    , placeholder : String
    , type_ : String
    , required : Bool
    , onInput : String -> msg
    }
    -> Html msg
field config =
    div [ class "field" ]
        [ label [ for config.id ]
            [ text config.label
            , if config.required then
                span [ class "required" ] [ text " *" ]

              else
                text ""
            ]
        , input
            [ id config.id
            , name config.id
            , type_ config.type_
            , value config.value
            , placeholder config.placeholder
            , autocomplete False
            , spellcheck False
            , onInput config.onInput
            ]
            []
        , hintText config.hint
        ]


hintText : String -> Html msg
hintText hint =
    if String.isEmpty hint then
        text ""

    else
        p [ class "hint" ] [ text hint ]


textarea :
    { id : String
    , label : String
    , value : String
    , hint : String
    , rows : Int
    , onInput : String -> msg
    }
    -> Html msg
textarea config =
    div [ class "field" ]
        [ label [ for config.id ] [ text config.label ]
        , Html.textarea
            [ id config.id
            , name config.id
            , rows config.rows
            , value config.value
            , onInput config.onInput
            ]
            []
        , hintText config.hint
        ]


toggle : { id : String, label : String, checked : Bool, onCheck : Bool -> msg } -> Html msg
toggle config =
    div [ class "field field-toggle" ]
        [ input
            [ id config.id
            , name config.id
            , type_ "checkbox"
            , Html.Attributes.checked config.checked
            , onInput (\_ -> config.onCheck (not config.checked))
            ]
            []
        , label [ for config.id ] [ text config.label ]
        ]


{-| A small pill / status chip.
-}
chip : { label : String, variant : String, dot : Bool } -> Html msg
chip config =
    span [ classList [ ( "chip", True ), ( config.variant, True ) ] ]
        [ if config.dot then
            span [ class "dot" ] []

          else
            text ""
        , text config.label
        ]


pill : String -> Html msg
pill label_ =
    span [ class "pill" ] [ text label_ ]


{-| A segmented control. `options` are `(value, label)` pairs.
-}
segmented :
    { name : String
    , options : List ( String, String )
    , selected : String
    , onSelect : String -> msg
    , disabled : List String
    }
    -> Html msg
segmented config =
    div [ class "segmented", attribute "role" "tablist" ]
        (List.map
            (\( val, lbl ) ->
                let
                    isDisabled =
                        List.member val config.disabled
                in
                Html.button
                    [ classList
                        [ ( "segment", True )
                        , ( "is-selected", val == config.selected )
                        , ( "is-disabled", isDisabled )
                        ]
                    , type_ "button"
                    , disabled isDisabled
                    , onClick (config.onSelect val)
                    ]
                    [ text lbl ]
            )
            config.options
        )


{-| A stat tile in the inspector strip.
-}
stat : { key : String, value : String, tone : String, sub : String } -> Html msg
stat config =
    div [ class "stat" ]
        [ span [ class "stat-key" ] [ text config.key ]
        , span [ classList [ ( "stat-value", True ), ( config.tone, True ) ] ] [ text config.value ]
        , if String.isEmpty config.sub then
            text ""

          else
            span [ class "stat-sub" ] [ text config.sub ]
        ]


emptyState : { icon : Html msg, title : String, body : String, action : Maybe (Html msg) } -> Html msg
emptyState config =
    div [ class "empty-state" ]
        [ span [ class "empty-icon" ] [ config.icon ]
        , h2 [] [ text config.title ]
        , p [] [ text config.body ]
        , case config.action of
            Just action ->
                action

            Nothing ->
                text ""
        ]


{-| A modal overlay. Closes on backdrop click and Escape (handled by the page).
-}
modal :
    { title : String
    , onClose : msg
    , body : List (Html msg)
    , footer : List (Html msg)
    }
    -> Html msg
modal config =
    div [ class "modal-overlay" ]
        [ div
            [ class "modal"
            , attribute "role" "dialog"
            , attribute "aria-modal" "true"
            ]
            [ div [ class "modal-header" ]
                [ h2 [] [ text config.title ]
                , iconButton
                    { icon = Icons.close
                    , label = "Close"
                    , onClick = config.onClose
                    , variant = Ghost
                    , disabled = False
                    , small = True
                    }
                ]
            , div [ class "modal-body" ] config.body
            , div [ class "modal-footer" ] config.footer
            ]
        ]


{-| An external link styled as a button.
-}
link : { label : String, url : String, newTab : Bool } -> Html msg
link config =
    a
        (class "btn btn-secondary"
            :: Html.Attributes.href config.url
            :: (if config.newTab then
                    [ Html.Attributes.target "_blank", Html.Attributes.rel "noopener" ]

                else
                    []
               )
        )
        [ text config.label ]
