module Page.Inbox exposing (Model, Msg, init, subscriptions, update, view)

{-| The reader-facing inbox.

Every stored email the signed-in Cloudflare Access identity is allowed to read,
across every rule. Who may read a rule's stored mail is configured on the rule
in `/manage`: each of its email destinations is an implicit reader, and the
operator can add more. The server enforces the same rule when it answers
`/api/inbox`, so this page shows nothing without a valid Access session.

Unlike the workbench, this page renders from its own request rather than the
operator bootstrap, so a reader whose Access policy doesn't cover
`/api/bootstrap` still sees their mail.

-}

import Api
import Format
import Html exposing (Html, button, div, h1, iframe, p, pre, section, span, text)
import Html.Attributes as A exposing (attribute, class, classList, type_)
import Html.Events exposing (onClick)
import Icons
import RemoteData exposing (RemoteData(..))
import Ui exposing (ButtonVariant(..))


type alias Model =
    { data : Api.Data Api.InboxPayload
    , expanded : Maybe String
    , viewing : Api.Data Api.RenderedMessage
    , loadingMore : Bool
    }


init : ( Model, Cmd Msg )
init =
    ( { data = Loading
      , expanded = Nothing
      , viewing = NotAsked
      , loadingMore = False
      }
    , Api.getInbox { before = Nothing } GotInbox
    )


subscriptions : Model -> Sub Msg
subscriptions _ =
    Sub.none


type Msg
    = GotInbox (Api.Data Api.InboxPayload)
    | ToggleMessage String
    | GotMessage (Api.Data Api.RenderedMessage)
    | LoadMore
    | GotMore (Api.Data Api.InboxPayload)


update : Msg -> Model -> ( Model, Cmd Msg )
update msg model =
    case msg of
        GotInbox result ->
            ( { model | data = result }, Cmd.none )

        ToggleMessage id ->
            if model.expanded == Just id then
                ( { model | expanded = Nothing, viewing = NotAsked }, Cmd.none )

            else
                ( { model | expanded = Just id, viewing = Loading }
                , Api.getInboxMessage id GotMessage
                )

        GotMessage result ->
            ( { model | viewing = result }, Cmd.none )

        LoadMore ->
            case model.data of
                Success payload ->
                    ( { model | loadingMore = True }
                    , Api.getInbox { before = payload.nextBefore } GotMore
                    )

                _ ->
                    ( model, Cmd.none )

        GotMore result ->
            case ( result, model.data ) of
                ( Success page, Success old ) ->
                    ( { model
                        | loadingMore = False
                        , data =
                            Success
                                { old
                                    | messages = old.messages ++ page.messages
                                    , hasMore = page.hasMore
                                    , nextBefore = page.nextBefore
                                }
                      }
                    , Cmd.none
                    )

                _ ->
                    ( { model | loadingMore = False }, Cmd.none )


view : Model -> Html Msg
view model =
    section [ class "inbox-page" ]
        [ inboxHead model
        , case model.data of
            Loading ->
                Ui.spinner

            NotAsked ->
                Ui.spinner

            Failure Api.Unauthenticated ->
                accessState

            Failure err ->
                Ui.banner Ui.Error (Api.errorMessage err)

            Success payload ->
                inboxList model payload
        ]


inboxHead : Model -> Html Msg
inboxHead model =
    let
        count =
            case model.data of
                Success payload ->
                    String.fromInt (List.length payload.messages)
                        ++ (if payload.hasMore then
                                "+"

                            else
                                ""
                           )

                _ ->
                    ""
    in
    div [ class "inbox-head" ]
        [ div []
            [ h1 [] [ text "Inbox" ]
            , p [ class "inbox-lede" ]
                [ text "Every stored email you can read." ]
            ]
        , if String.isEmpty count then
            text ""

          else
            span [ class "inbox-count" ] [ text (count ++ " messages") ]
        ]


inboxList : Model -> Api.InboxPayload -> Html Msg
inboxList model payload =
    if List.isEmpty payload.messages then
        Ui.emptyState
            { icon = Icons.mail
            , title = "Nothing here yet"
            , body = "Stored email you can read will appear here."
            , action = Nothing
            }

    else
        div [ class "inbox-list-wrap" ]
            [ div [ class "inbox-list" ] (List.map (messageRow model) payload.messages)
            , if payload.hasMore then
                div [ class "load-more" ]
                    [ Ui.button
                        { label = "Load more"
                        , onClick = LoadMore
                        , variant = Ghost
                        , busy = model.loadingMore
                        , disabled = False
                        }
                    ]

              else
                text ""
            ]


messageRow : Model -> Api.InboxItem -> Html Msg
messageRow model item =
    let
        isOpen =
            model.expanded == Just item.id
    in
    div [ classList [ ( "message", True ), ( "is-open", isOpen ) ] ]
        [ button
            [ class "inbox-row"
            , type_ "button"
            , onClick (ToggleMessage item.id)
            ]
            [ span [ class "inbox-ts" ]
                [ text (item.createdAt |> Maybe.map Format.dateShort |> Maybe.withDefault "—") ]
            , span [ class "inbox-rule" ]
                [ text (Maybe.withDefault "—" item.ruleLabel) ]
            , span [ class "inbox-from" ] [ text item.sender ]
            , span [ class "inbox-subj" ] [ text item.subject ]
            , span [ classList [ ( "inbox-caret", True ), ( "is-open", isOpen ) ] ] [ Icons.chevronRight ]
            ]
        , if isOpen then
            div [ class "message-body" ] [ messageBody model item.id ]

          else
            text ""
        ]


messageBody : Model -> String -> Html Msg
messageBody model messageId =
    case model.viewing of
        Loading ->
            Ui.spinner

        Failure err ->
            Ui.banner Ui.Error (Api.errorMessage err)

        Success rendered ->
            div []
                [ div [ class "embed-header" ]
                    [ embedRow "From" rendered.from
                    , embedRow "To" rendered.to
                    , case rendered.date of
                        Just d ->
                            embedRow "Date" d

                        Nothing ->
                            text ""
                    , embedRow "Subject" rendered.subject
                    ]
                , case rendered.body of
                    Api.HtmlBody html ->
                        iframe
                            [ class "email-body"
                            , attribute "sandbox" "allow-popups allow-popups-to-escape-sandbox"
                            , attribute "srcdoc" html
                            ]
                            []

                    Api.TextBody txt ->
                        pre [ class "email-text" ] [ text txt ]

                    Api.NoBody ->
                        p [ class "empty-inline" ] [ text "No renderable body." ]
                , div [ class "embed-footer" ]
                    [ Ui.link
                        { label = "Open in new tab"
                        , url = "/manage/m/" ++ messageId
                        , newTab = True
                        }
                    ]
                ]

        _ ->
            Ui.spinner


embedRow : String -> String -> Html Msg
embedRow k v =
    div [ class "embed-row" ]
        [ span [ class "embed-k" ] [ text k ]
        , span [ class "embed-v" ] [ text v ]
        ]


accessState : Html Msg
accessState =
    div [ class "centered-screen" ]
        [ Ui.emptyState
            { icon = Icons.shield
            , title = "Access required"
            , body = "Session expired. Reload to sign in."
            , action = Just (Html.a [ class "btn btn-primary", A.href "/inbox" ] [ text "Reload" ])
            }
        ]
