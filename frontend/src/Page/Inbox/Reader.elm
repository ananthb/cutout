module Page.Inbox.Reader exposing
    ( Config
    , displaySubject
    , view
    , wrapSrcdoc
    )

{-| Reading pane for the `/inbox` page.

Renders the selected email's header (back button, rule chip, date, prev/next
navigation, subject with empty-subject fallback, From/To addresses) and body
(sandboxed `srcdoc` iframe with base typography styles for HTML emails, `<pre>`
for plain text, or an inline empty notice).

-}

import Api
import Format
import Html exposing (Html, button, div, h1, header, iframe, p, pre, span, text)
import Html.Attributes exposing (attribute, class, classList, type_)
import Html.Events exposing (onClick)
import Icons
import RemoteData exposing (RemoteData(..))
import Ui exposing (ButtonVariant(..))


type alias Config msg =
    { selected : Maybe String
    , viewing : Api.Data Api.RenderedMessage
    , selectedItem : Maybe Api.InboxItem
    , hasPrev : Bool
    , hasNext : Bool
    , onClose : msg
    , onPrev : msg
    , onNext : msg
    }


displaySubject : String -> String
displaySubject subject =
    if String.isEmpty (String.trim subject) then
        "(no subject)"

    else
        subject


wrapSrcdoc : String -> String
wrapSrcdoc html =
    """<!doctype html><html><head><meta charset="utf-8"><meta name="viewport" content="width=device-width, initial-scale=1"><style>
html, body {
  margin: 0;
  padding: 0;
  background: #ffffff;
  color: #1f1f1f;
  font-family: -apple-system, BlinkMacSystemFont, "Segoe UI", Roboto, Helvetica, Arial, sans-serif;
  font-size: 14px;
  line-height: 1.55;
  overflow-wrap: break-word;
}
body {
  padding: 16px 18px;
}
img {
  max-width: 100%;
  height: auto;
}
pre, code {
  font-family: ui-monospace, "SF Mono", Menlo, Consolas, monospace;
  font-size: 0.92em;
}
pre {
  white-space: pre-wrap;
  overflow-x: auto;
}
blockquote {
  margin: 0.5em 0;
  padding-left: 12px;
  border-left: 3px solid #e0dcd5;
  color: #555;
}
a {
  color: #c86428;
}
</style></head><body>"""
        ++ html
        ++ "</body></html>"


view : Config msg -> Html msg
view config =
    div [ class "inbox-read-pane" ]
        (case config.selected of
            Nothing ->
                [ div [ class "inbox-read-empty" ]
                    [ span [ class "inbox-read-mark" ] [ Icons.mail ]
                    , p [] [ text "Pick an email to read." ]
                    , span [ class "inbox-read-empty-hint" ] [ text "↑ / ↓ to step through messages · r to refresh" ]
                    ]
                ]

            Just _ ->
                case config.viewing of
                    NotAsked ->
                        [ Ui.spinner ]

                    Loading ->
                        [ Ui.spinner ]

                    Failure err ->
                        [ header [ class "inbox-read-head" ]
                            [ div [ class "ir-toprow" ]
                                [ viewBackButton config.onClose ]
                            ]
                        , div [ class "inbox-read-body" ]
                            [ Ui.banner Ui.Error (Api.errorMessage err) ]
                        ]

                    Success rendered ->
                        [ viewHeader config rendered
                        , div [ class "inbox-read-body" ]
                            [ viewBody rendered.body ]
                        ]
        )


viewBackButton : msg -> Html msg
viewBackButton onClose =
    button
        [ class "btn btn-secondary btn-sm inbox-back"
        , type_ "button"
        , onClick onClose
        ]
        [ Icons.chevronRight, text "Inbox" ]


viewHeader : Config msg -> Api.RenderedMessage -> Html msg
viewHeader config rendered =
    header [ class "inbox-read-head" ]
        [ div [ class "ir-toprow" ]
            [ viewBackButton config.onClose
            , case config.selectedItem |> Maybe.andThen .ruleLabel of
                Just label ->
                    Ui.chip { label = label, variant = "proxy", dot = False }

                Nothing ->
                    text ""
            , case rendered.date of
                Just d ->
                    span [ class "ir-date" ] [ text (Format.dateShort d) ]

                Nothing ->
                    text ""
            , div [ class "ir-spacer" ] []
            , div [ class "ir-nav" ]
                [ Ui.iconButton
                    { icon = Icons.arrowUp
                    , label = "Previous email (k)"
                    , onClick = config.onPrev
                    , variant = Ghost
                    , disabled = not config.hasPrev
                    , small = True
                    }
                , Ui.iconButton
                    { icon = Icons.arrowDown
                    , label = "Next email (j)"
                    , onClick = config.onNext
                    , variant = Ghost
                    , disabled = not config.hasNext
                    , small = True
                    }
                ]
            ]
        , h1
            [ classList
                [ ( "ir-subject", True )
                , ( "is-empty", String.isEmpty (String.trim rendered.subject) )
                ]
            ]
            [ text (displaySubject rendered.subject) ]
        , div [ class "ir-addrs" ]
            [ div [ class "ir-addr-row" ]
                [ span [ class "ir-addr-k" ] [ text "From" ]
                , span [ class "ir-addr-v" ] [ text rendered.from ]
                ]
            , div [ class "ir-addr-row" ]
                [ span [ class "ir-addr-k" ] [ text "To" ]
                , span [ class "ir-addr-v" ] [ text rendered.to ]
                ]
            ]
        ]


viewBody : Api.Body -> Html msg
viewBody body =
    case body of
        Api.HtmlBody html ->
            iframe
                [ class "email-body"
                , attribute "sandbox" "allow-popups allow-popups-to-escape-sandbox"
                , attribute "srcdoc" (wrapSrcdoc html)
                ]
                []

        Api.TextBody txt ->
            pre [ class "email-text" ] [ text txt ]

        Api.NoBody ->
            p [ class "empty-inline" ] [ text "No renderable body." ]
