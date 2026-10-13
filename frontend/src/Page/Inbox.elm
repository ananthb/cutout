module Page.Inbox exposing (Model, Msg, init, select, subscriptions, update, view)

{-| The reader-facing inbox, two panes: the list of stored emails the signed-in
Cloudflare Access identity may read, and the message it is reading.

Who may read a rule's stored mail is configured on the rule in `/manage`: each
of its email destinations is an implicit reader, and the operator can add more.
The server enforces the same rule when it answers `/api/inbox`, so this page
shows nothing without a valid Access session.

The selected message lives in the URL (`/inbox/{id}`), so a link into one
message — from a chat post or the rules view — opens straight into the reading
pane.

-}

import Api
import Format
import Html exposing (Html, a, div, h1, iframe, p, pre, section, span, text)
import Html.Attributes as A exposing (attribute, class, classList, href, type_)
import Html.Events exposing (onClick)
import Icons
import RemoteData exposing (RemoteData(..))
import Ui exposing (ButtonVariant(..))


type alias Model =
    { data : Api.Data Api.InboxPayload
    , selected : Maybe String
    , viewing : Api.Data Api.RenderedMessage
    , loadingMore : Bool
    }


init : Maybe String -> ( Model, Cmd Msg )
init selected =
    ( { data = Loading
      , selected = selected
      , viewing =
            case selected of
                Just _ ->
                    Loading

                Nothing ->
                    NotAsked
      , loadingMore = False
      }
    , Cmd.batch
        [ Api.getInbox { before = Nothing } GotInbox
        , case selected of
            Just id ->
                Api.getInboxMessage id GotMessage

            Nothing ->
                Cmd.none
        ]
    )


{-| Point the reading pane at a message, driven by the route. A repeat selection
is a no-op so the message isn't refetched on every bootstrap.
-}
select : Maybe String -> Model -> ( Model, Cmd Msg )
select target model =
    if target == model.selected then
        ( model, Cmd.none )

    else
        ( { model
            | selected = target
            , viewing =
                case target of
                    Just _ ->
                        Loading

                    Nothing ->
                        NotAsked
          }
        , case target of
            Just id ->
                Api.getInboxMessage id GotMessage

            Nothing ->
                Cmd.none
        )


subscriptions : Model -> Sub Msg
subscriptions _ =
    Sub.none


type Msg
    = GotInbox (Api.Data Api.InboxPayload)
    | GotMessage (Api.Data Api.RenderedMessage)
    | LoadMore
    | GotMore (Api.Data Api.InboxPayload)


update : Msg -> Model -> ( Model, Cmd Msg )
update msg model =
    case msg of
        GotInbox result ->
            ( { model | data = result }, Cmd.none )

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
        [ inboxListPane model
        , inboxReadPane model
        ]


inboxListPane : Model -> Html Msg
inboxListPane model =
    div [ class "inbox-list-pane" ]
        [ div [ class "inbox-list-head" ]
            [ h1 [] [ text "Inbox" ]
            , case model.data of
                Success payload ->
                    span [ class "inbox-count" ] [ text (countLabel payload) ]

                _ ->
                    text ""
            ]
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


countLabel : Api.InboxPayload -> String
countLabel payload =
    String.fromInt (List.length payload.messages)
        ++ (if payload.hasMore then
                "+"

            else
                ""
           )


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
        div [ class "inbox-list" ]
            (List.map (messageRow model) payload.messages
                ++ (if payload.hasMore then
                        [ div [ class "load-more" ]
                            [ Ui.button
                                { label = "Load more"
                                , onClick = LoadMore
                                , variant = Ghost
                                , busy = model.loadingMore
                                , disabled = False
                                }
                            ]
                        ]

                    else
                        []
                   )
            )


messageRow : Model -> Api.InboxItem -> Html Msg
messageRow model item =
    a
        [ classList [ ( "inbox-row", True ), ( "is-active", model.selected == Just item.id ) ]
        , href ("/inbox/" ++ item.id)
        ]
        [ span [ class "inbox-ts" ]
            [ text (item.createdAt |> Maybe.map Format.dateShort |> Maybe.withDefault "—") ]
        , span [ class "inbox-rule" ] [ text (Maybe.withDefault "—" item.ruleLabel) ]
        , span [ class "inbox-from" ] [ text item.sender ]
        , span [ class "inbox-subj" ] [ text item.subject ]
        ]


inboxReadPane : Model -> Html Msg
inboxReadPane model =
    div [ class "inbox-read-pane" ]
        (case model.selected of
            Nothing ->
                [ div [ class "inbox-read-empty" ]
                    [ span [ class "inbox-read-mark" ] [ Icons.mail ]
                    , p [] [ text "Pick an email to read." ]
                    ]
                ]

            Just _ ->
                [ case model.viewing of
                    Loading ->
                        Ui.spinner

                    NotAsked ->
                        Ui.spinner

                    Failure err ->
                        Ui.banner Ui.Error (Api.errorMessage err)

                    Success rendered ->
                        renderedView rendered
                ]
        )


renderedView : Api.RenderedMessage -> Html Msg
renderedView rendered =
    div [ class "inbox-read-body" ]
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
        ]


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
