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
import Browser.Events
import Browser.Navigation as Nav
import Format
import Html exposing (Html, a, button, div, h1, input, section, span, text)
import Html.Attributes as A exposing (autocomplete, class, classList, href, placeholder, spellcheck, title, type_, value)
import Html.Events exposing (onClick, onInput)
import Icons
import Json.Decode as D
import Page.Inbox.Reader as Reader
import RemoteData exposing (RemoteData(..))
import Ui exposing (ButtonVariant(..))


type alias Model =
    { key : Nav.Key
    , data : Api.Data Api.InboxPayload
    , selected : Maybe String
    , viewing : Api.Data Api.RenderedMessage
    , loadingMore : Bool
    , refreshing : Bool
    , search : String
    , ruleFilter : Maybe String
    }


init : Nav.Key -> Maybe String -> ( Model, Cmd Msg )
init key selected =
    ( { key = key
      , data = Loading
      , selected = selected
      , viewing =
            case selected of
                Just _ ->
                    Loading

                Nothing ->
                    NotAsked
      , loadingMore = False
      , refreshing = False
      , search = ""
      , ruleFilter = Nothing
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
    Browser.Events.onKeyDown keyDecoder


keyDecoder : D.Decoder Msg
keyDecoder =
    D.map5
        (\key tag ctrl meta alt ->
            if ctrl || meta || alt || isInputTag tag then
                Nothing

            else
                case key of
                    "ArrowDown" ->
                        Just (StepMessage 1)

                    "j" ->
                        Just (StepMessage 1)

                    "ArrowUp" ->
                        Just (StepMessage -1)

                    "k" ->
                        Just (StepMessage -1)

                    "Escape" ->
                        Just CloseMessage

                    "r" ->
                        Just Refresh

                    _ ->
                        Nothing
        )
        (D.field "key" D.string)
        (D.oneOf [ D.at [ "target", "tagName" ] D.string, D.succeed "" ])
        (D.oneOf [ D.field "ctrlKey" D.bool, D.succeed False ])
        (D.oneOf [ D.field "metaKey" D.bool, D.succeed False ])
        (D.oneOf [ D.field "altKey" D.bool, D.succeed False ])
        |> D.andThen
            (\maybeMsg ->
                case maybeMsg of
                    Just msg ->
                        D.succeed msg

                    Nothing ->
                        D.fail "ignored"
            )


isInputTag : String -> Bool
isInputTag tag =
    let
        upper =
            String.toUpper tag
    in
    upper == "INPUT" || upper == "TEXTAREA" || upper == "SELECT"


type Msg
    = GotInbox (Api.Data Api.InboxPayload)
    | GotMessage (Api.Data Api.RenderedMessage)
    | LoadMore
    | GotMore (Api.Data Api.InboxPayload)
    | Refresh
    | SetSearch String
    | ClearSearch
    | SetRuleFilter (Maybe String)
    | CloseMessage
    | StepMessage Int


update : Msg -> Model -> ( Model, Cmd Msg )
update msg model =
    case msg of
        GotInbox result ->
            ( { model | data = result, refreshing = False }, Cmd.none )

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

        Refresh ->
            ( { model | refreshing = True }
            , Cmd.batch
                [ Api.getInbox { before = Nothing } GotInbox
                , case model.selected of
                    Just id ->
                        Api.getInboxMessage id GotMessage

                    Nothing ->
                        Cmd.none
                ]
            )

        SetSearch query ->
            ( { model | search = query }, Cmd.none )

        ClearSearch ->
            ( { model | search = "" }, Cmd.none )

        SetRuleFilter target ->
            let
                nextFilter =
                    if model.ruleFilter == target then
                        Nothing

                    else
                        target
            in
            ( { model | ruleFilter = nextFilter }, Cmd.none )

        CloseMessage ->
            ( model, Nav.pushUrl model.key "/inbox" )

        StepMessage delta ->
            case model.data of
                Success payload ->
                    let
                        ids =
                            visibleMessages model payload
                                |> List.map .id
                    in
                    case stepId delta model.selected ids of
                        Just nextId ->
                            ( model, Nav.pushUrl model.key ("/inbox/" ++ nextId) )

                        Nothing ->
                            ( model, Cmd.none )

                _ ->
                    ( model, Cmd.none )


stepId : Int -> Maybe String -> List String -> Maybe String
stepId delta selected ids =
    case selected of
        Nothing ->
            if delta >= 0 then
                List.head ids

            else
                List.reverse ids |> List.head

        Just current ->
            let
                indexed =
                    List.indexedMap Tuple.pair ids

                currentIdx =
                    indexed
                        |> List.filter (\( _, id ) -> id == current)
                        |> List.head
                        |> Maybe.map Tuple.first
            in
            case currentIdx of
                Just idx ->
                    let
                        targetIdx =
                            clamp 0 (List.length ids - 1) (idx + delta)
                    in
                    if targetIdx == idx then
                        Nothing

                    else
                        indexed
                            |> List.filter (\( i, _ ) -> i == targetIdx)
                            |> List.head
                            |> Maybe.map Tuple.second

                Nothing ->
                    List.head ids


visibleMessages : Model -> Api.InboxPayload -> List Api.InboxItem
visibleMessages model payload =
    let
        needle =
            String.toLower (String.trim model.search)

        matchesRule item =
            case model.ruleFilter of
                Nothing ->
                    True

                Just label ->
                    item.ruleLabel == Just label

        matchesSearch item =
            if String.isEmpty needle then
                True

            else
                let
                    ruleText =
                        Maybe.withDefault "" item.ruleLabel
                in
                String.contains needle (String.toLower item.sender)
                    || String.contains needle (String.toLower item.subject)
                    || String.contains needle (String.toLower ruleText)
    in
    payload.messages
        |> List.filter (\item -> matchesRule item && matchesSearch item)


isFiltered : Model -> Bool
isFiltered model =
    model.ruleFilter /= Nothing || not (String.isEmpty (String.trim model.search))


distinctRuleLabels : List Api.InboxItem -> List String
distinctRuleLabels items =
    List.foldl
        (\item acc ->
            case item.ruleLabel of
                Just label ->
                    if List.member label acc then
                        acc

                    else
                        acc ++ [ label ]

                Nothing ->
                    acc
        )
        []
        items


view : Model -> Html Msg
view model =
    section
        [ classList
            [ ( "inbox-page", True )
            , ( "has-selection", model.selected /= Nothing )
            , ( "no-selection", model.selected == Nothing )
            ]
        ]
        [ inboxListPane model
        , inboxReadPane model
        ]


inboxListPane : Model -> Html Msg
inboxListPane model =
    div [ class "inbox-list-pane" ]
        (div [ class "inbox-list-head" ]
            [ div [ class "inbox-head-left" ]
                [ h1 [] [ text "Inbox" ]
                , case model.data of
                    Success payload ->
                        span [ class "inbox-count" ] [ text (headerCountLabel model payload) ]

                    _ ->
                        text ""
                ]
            , div [ class "inbox-head-actions" ]
                [ Ui.iconButton
                    { icon = Icons.refresh
                    , label = "Refresh inbox (r)"
                    , onClick = Refresh
                    , variant = Ghost
                    , disabled = model.refreshing || model.data == Loading
                    , small = True
                    }
                ]
            ]
            :: (case model.data of
                    Loading ->
                        [ Ui.spinner ]

                    NotAsked ->
                        [ Ui.spinner ]

                    Failure Api.Unauthenticated ->
                        [ accessState ]

                    Failure err ->
                        [ Ui.banner Ui.Error (Api.errorMessage err) ]

                    Success payload ->
                        inboxList model payload
               )
        )


headerCountLabel : Model -> Api.InboxPayload -> String
headerCountLabel model payload =
    if isFiltered model then
        let
            visibleCount =
                List.length (visibleMessages model payload)
        in
        String.fromInt visibleCount ++ " / " ++ countLabel payload

    else
        countLabel payload


countLabel : Api.InboxPayload -> String
countLabel payload =
    String.fromInt (List.length payload.messages)
        ++ (if payload.hasMore then
                "+"

            else
                ""
           )


inboxList : Model -> Api.InboxPayload -> List (Html Msg)
inboxList model payload =
    if List.isEmpty payload.messages then
        [ Ui.emptyState
            { icon = Icons.mail
            , title = "Nothing here yet"
            , body = "Stored email you can read will appear here."
            , action = Nothing
            }
        ]

    else
        let
            visible =
                visibleMessages model payload
        in
        [ inboxToolbar model payload
        , div [ class "inbox-list" ]
            ((if List.isEmpty visible then
                [ div [ class "empty-inline" ] [ text "No messages match your filter." ] ]

              else
                List.map (messageRow model) visible
             )
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
        ]


inboxToolbar : Model -> Api.InboxPayload -> Html Msg
inboxToolbar model payload =
    let
        ruleLabels =
            distinctRuleLabels payload.messages
    in
    div [ class "inbox-toolbar" ]
        [ div [ class "inbox-search-wrap" ]
            [ span [ class "inbox-search-icon" ] [ Icons.search ]
            , input
                [ class "inbox-search"
                , type_ "text"
                , placeholder "Search sender, subject, or rule…"
                , value model.search
                , onInput SetSearch
                , autocomplete False
                , spellcheck False
                ]
                []
            , if not (String.isEmpty model.search) then
                button
                    [ class "inbox-search-clear"
                    , type_ "button"
                    , onClick ClearSearch
                    , title "Clear search"
                    ]
                    [ Icons.close ]

              else
                text ""
            ]
        , if List.length ruleLabels >= 2 then
            div [ class "inbox-rule-filters" ]
                (filterChip "All" (model.ruleFilter == Nothing) (SetRuleFilter Nothing)
                    :: List.map
                        (\label ->
                            filterChip label (model.ruleFilter == Just label) (SetRuleFilter (Just label))
                        )
                        ruleLabels
                )

          else
            text ""
        ]


filterChip : String -> Bool -> Msg -> Html Msg
filterChip label isActive onSelect =
    button
        [ classList
            [ ( "inbox-filter-chip", True )
            , ( "is-active", isActive )
            ]
        , type_ "button"
        , onClick onSelect
        ]
        [ text label ]


messageRow : Model -> Api.InboxItem -> Html Msg
messageRow model item =
    a
        [ classList [ ( "inbox-row", True ), ( "is-active", model.selected == Just item.id ) ]
        , href ("/inbox/" ++ item.id)
        ]
        [ span [ class "inbox-from" ] [ text item.sender ]
        , span [ class "inbox-ts" ]
            [ text (item.createdAt |> Maybe.map Format.dateShort |> Maybe.withDefault "—") ]
        , span
            [ classList
                [ ( "inbox-subj", True )
                , ( "is-empty", String.trim item.subject == "" )
                ]
            ]
            [ text (Reader.displaySubject item.subject) ]
        , case item.ruleLabel of
            Just label ->
                span [ class "inbox-rule" ] [ text label ]

            Nothing ->
                text ""
        ]


inboxReadPane : Model -> Html Msg
inboxReadPane model =
    let
        ( selectedItem, hasPrev, hasNext ) =
            case ( model.selected, model.data ) of
                ( Just currentId, Success payload ) ->
                    let
                        visible =
                            visibleMessages model payload

                        ids =
                            List.map .id visible

                        found =
                            payload.messages
                                |> List.filter (\item -> item.id == currentId)
                                |> List.head
                    in
                    ( found
                    , stepId -1 (Just currentId) ids /= Nothing
                    , stepId 1 (Just currentId) ids /= Nothing
                    )

                _ ->
                    ( Nothing, False, False )
    in
    Reader.view
        { selected = model.selected
        , viewing = model.viewing
        , selectedItem = selectedItem
        , hasPrev = hasPrev
        , hasNext = hasNext
        , onClose = CloseMessage
        , onPrev = StepMessage -1
        , onNext = StepMessage 1
        }


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
