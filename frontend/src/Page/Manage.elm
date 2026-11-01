module Page.Manage exposing (Model, Msg, init, subscriptions, update, view)

{-| The workbench: the routing pipeline, the inspector/overview, the inline rule
editor and the live feed.

The page owns its copy of the server state (`data`). Every rule mutation returns
a fresh snapshot, so the server snapshot remains the source of truth (with
optimistic reordering for immediate feedback).

The tester runs the routing engine's glob matcher in Elm on every keystroke and
highlights the rule that would fire, so you can see the pipeline decide without
a request.

-}

import Api
import Browser.Events
import Browser.Navigation as Nav
import Format
import Glob
import Html exposing (Html, a, aside, button, div, h1, h3, header, input, p, section, span, text)
import Html.Attributes as A exposing (attribute, class, classList, href, placeholder, title, type_, value)
import Html.Events exposing (onClick, onInput)
import Icons
import Json.Decode as D
import Page.Manage.Editor as Editor
import Ports
import RemoteData exposing (RemoteData(..))
import Route
import Time
import Ui exposing (ButtonVariant(..))



-- MODEL


type alias Model =
    { data : Api.Data Api.Snapshot
    , selected : Maybe String
    , tester : String
    , editor : Maybe Editor.Editor
    , live : Live
    , messages : Messages
    , notice : Maybe ( Ui.Banner, String )
    , busy : Bool
    , zone : Int
    , now : Int
    , deleteTarget : Maybe String
    }


type alias Live =
    { events : List Api.Event
    , collapsed : Bool
    , filter : String
    , lastTs : Int
    , pending : Api.PendingCounts
    , loaded : Bool
    }


type alias Messages =
    { items : List Api.MessageItem
    , cursor : Maybe String
    , hasMore : Bool
    , loading : Bool
    , error : Maybe String
    }


emptyMessages : Messages
emptyMessages =
    { items = [], cursor = Nothing, hasMore = False, loading = False, error = Nothing }


init : Nav.Key -> Api.Snapshot -> ( Model, Cmd Msg )
init _ snapshot =
    ( { data = Success snapshot
      , selected = Nothing
      , tester = ""
      , editor = Nothing
      , live =
            { events = []
            , collapsed = True
            , filter = "all"
            , lastTs = 0
            , pending = snapshot.pending
            , loaded = False
            }
      , messages = emptyMessages
      , notice = Nothing
      , busy = False
      , zone = 0
      , now = 0
      , deleteTarget = Nothing
      }
    , Cmd.batch
        [ Api.getEvents Nothing GotEvents
        , Api.getPendingCount GotPending
        ]
    )


subscriptions : Model -> Sub Msg
subscriptions model =
    Sub.batch
        [ if not model.live.collapsed || model.selected /= Nothing then
            Time.every 1000 Tick

          else
            Sub.none
        , if not model.live.collapsed then
            Time.every 2000 (\_ -> PollEvents)

          else
            Sub.none
        , Time.every 10000 (\_ -> PollPending)
        , Ports.zone Zone
        , Browser.Events.onKeyDown (keyDecoder model)
        ]


keyDecoder : Model -> D.Decoder Msg
keyDecoder model =
    D.map5
        (\key tag ctrl meta alt ->
            if ctrl || meta || alt || isInputTag tag then
                Nothing

            else
                case key of
                    "Escape" ->
                        if model.deleteTarget /= Nothing then
                            Just CancelDelete

                        else if model.editor /= Nothing then
                            Just CloseEditor

                        else if model.selected /= Nothing then
                            Just (SelectRule Nothing)

                        else
                            Nothing

                    "ArrowDown" ->
                        if model.editor == Nothing && model.deleteTarget == Nothing then
                            Just (StepRule 1)

                        else
                            Nothing

                    "j" ->
                        if model.editor == Nothing && model.deleteTarget == Nothing then
                            Just (StepRule 1)

                        else
                            Nothing

                    "ArrowUp" ->
                        if model.editor == Nothing && model.deleteTarget == Nothing then
                            Just (StepRule -1)

                        else
                            Nothing

                    "k" ->
                        if model.editor == Nothing && model.deleteTarget == Nothing then
                            Just (StepRule -1)

                        else
                            Nothing

                    "e" ->
                        if model.editor == Nothing && model.deleteTarget == Nothing then
                            Maybe.map OpenEditRule model.selected

                        else
                            Nothing

                    "n" ->
                        if model.editor == Nothing && model.deleteTarget == Nothing then
                            Just OpenNewRule

                        else
                            Nothing

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



-- UPDATE


type Msg
    = Tick Time.Posix
    | Zone Int
    | PollEvents
    | GotEvents (Api.Data Api.EventsPayload)
    | PollPending
    | GotPending (Api.Data Api.PendingCounts)
    | ToggleLive
    | SetLiveFilter String
    | SetTester String
    | SelectRule (Maybe String)
    | StepRule Int
    | Reorder String String
    | OpenNewRule
    | OpenEditRule String
    | CloseEditor
    | EditorMsg Editor.Msg
    | GotSaved (Api.Data Api.Snapshot)
    | GotRefreshed (Api.Data Api.Snapshot)
    | AskDelete String
    | CancelDelete
    | ConfirmDelete
    | LoadMoreMessages
    | GotMoreMessages (Api.Data Api.MessagesPayload)
    | GotMessages (Api.Data Api.MessagesPayload)
    | GotChats (Api.Data (List Api.TelegramChat))
    | DismissNotice


update : Msg -> Model -> ( Model, Cmd Msg )
update msg model =
    case msg of
        Tick posix ->
            ( { model | now = Time.posixToMillis posix }, Cmd.none )

        Zone offset ->
            ( { model | zone = offset }, Cmd.none )

        PollEvents ->
            ( model
            , fetchEvents model.live
            )

        GotEvents result ->
            case result of
                Success payload ->
                    let
                        live =
                            model.live

                        merged =
                            if live.loaded then
                                payload.events ++ live.events

                            else
                                payload.events
                    in
                    ( { model
                        | live =
                            { live
                                | events = List.take 120 merged
                                , lastTs = max live.lastTs payload.now
                                , loaded = True
                            }
                      }
                    , Cmd.none
                    )

                Failure _ ->
                    -- Polling failures are swallowed; the next tick retries.
                    ( model, Cmd.none )

                _ ->
                    ( model, Cmd.none )

        PollPending ->
            ( model, Api.getPendingCount GotPending )

        GotPending result ->
            case result of
                Success counts ->
                    ( { model
                        | live = model.live |> (\l -> { l | pending = counts })
                        , data = RemoteData.map (\s -> { s | pending = counts }) model.data
                      }
                    , Cmd.none
                    )

                _ ->
                    ( model, Cmd.none )

        ToggleLive ->
            let
                nextCollapsed =
                    not model.live.collapsed

                live =
                    model.live
            in
            ( { model | live = { live | collapsed = nextCollapsed } }
            , if nextCollapsed then
                Cmd.none

              else
                fetchEvents live
            )

        SetLiveFilter f ->
            ( { model | live = model.live |> (\l -> { l | filter = f }) }, Cmd.none )

        SetTester v ->
            ( { model | tester = v }, Cmd.none )

        SelectRule target ->
            if target == model.selected && model.editor == Nothing then
                ( { model | selected = Nothing, messages = emptyMessages }, Cmd.none )

            else
                ( { model
                    | selected = target
                    , editor = Nothing
                    , messages = { emptyMessages | loading = target /= Nothing }
                    , notice = Nothing
                  }
                , case target of
                    Just id ->
                        Api.getRuleMessages { ruleId = id, before = Nothing } GotMessages

                    Nothing ->
                        Cmd.none
                )

        StepRule delta ->
            let
                ids =
                    List.map .id (rulesOf model)
            in
            case stepRuleId delta model.selected ids of
                Just nextId ->
                    update (SelectRule (Just nextId)) model

                Nothing ->
                    ( model, Cmd.none )

        Reorder id direction ->
            case model.data of
                Success snap ->
                    let
                        optimisticSnap =
                            { snap | rules = swapRule id direction snap.rules }
                    in
                    ( { model | busy = True, data = Success optimisticSnap }
                    , Api.reorderRule { id = id, direction = direction, rulesVersion = snap.rulesVersion } GotSaved
                    )

                _ ->
                    ( model, Cmd.none )

        OpenNewRule ->
            ( { model | editor = Just Editor.empty, notice = Nothing }
            , Api.getTelegramChats GotChats
            )

        OpenEditRule id ->
            case ruleAt id model of
                Just rule ->
                    ( { model | editor = Just (Editor.fromRule rule), notice = Nothing }
                    , Api.getTelegramChats GotChats
                    )

                Nothing ->
                    ( model, Cmd.none )

        CloseEditor ->
            ( { model | editor = Nothing }, Cmd.none )

        EditorMsg subMsg ->
            case ( model.editor, enabledChannels model ) of
                ( Just editor, Just enabled ) ->
                    let
                        ( nextEditor, editorCmd, outMsg ) =
                            Editor.update enabled subMsg editor
                    in
                    case outMsg of
                        Editor.NoOut ->
                            ( { model | editor = Just nextEditor }, Cmd.map EditorMsg editorCmd )

                        Editor.Cancelled ->
                            ( { model | editor = Nothing }, Cmd.map EditorMsg editorCmd )

                        Editor.Submit sub ->
                            case model.data of
                                Success snap ->
                                    let
                                        newRule =
                                            { label = sub.label
                                            , localPattern = sub.localPattern
                                            , domainPattern = sub.domainPattern
                                            , action = sub.action
                                            , inboxReaders = sub.inboxReaders
                                            , rulesVersion = snap.rulesVersion
                                            }

                                        saveCmd =
                                            case sub.id of
                                                Just id ->
                                                    Api.updateRule id newRule GotSaved

                                                Nothing ->
                                                    Api.createRule newRule GotSaved
                                    in
                                    ( { model | editor = Just nextEditor, notice = Nothing }
                                    , Cmd.batch [ Cmd.map EditorMsg editorCmd, saveCmd ]
                                    )

                                _ ->
                                    ( { model | editor = Just nextEditor }, Cmd.map EditorMsg editorCmd )

                _ ->
                    ( model, Cmd.none )

        GotSaved result ->
            case result of
                Success snap ->
                    let
                        stillSelected =
                            case model.selected of
                                Just selId ->
                                    if List.any (\r -> r.id == selId) snap.rules then
                                        Just selId

                                    else
                                        Nothing

                                Nothing ->
                                    Nothing
                    in
                    ( { model
                        | data = Success snap
                        , selected = stillSelected
                        , editor = Nothing
                        , busy = False
                        , notice = Just ( Ui.Success, "Saved." )
                      }
                    , Cmd.none
                    )

                Failure err ->
                    if isConflict err then
                        ( { model
                            | busy = False
                            , editor = Nothing
                            , notice =
                                Just
                                    ( Ui.Warn
                                    , "Another operator saved first. Loading their version — try your change again."
                                    )
                          }
                        , Api.getBootstrap GotRefreshed
                        )

                    else
                        case model.editor of
                            Just editor ->
                                ( { model
                                    | busy = False
                                    , editor =
                                        Just
                                            (editor
                                                |> Editor.setSubmitting False
                                                |> Editor.setError (Just (Api.errorMessage err))
                                            )
                                  }
                                , Cmd.none
                                )

                            Nothing ->
                                ( { model
                                    | busy = False
                                    , notice = Just ( Ui.Error, Api.errorMessage err )
                                  }
                                , Api.getBootstrap GotRefreshed
                                )

                _ ->
                    ( model, Cmd.none )

        GotRefreshed result ->
            case result of
                Success snap ->
                    ( { model | data = Success snap }, Cmd.none )

                Failure err ->
                    ( { model | data = Failure err }, Cmd.none )

                _ ->
                    ( model, Cmd.none )

        AskDelete id ->
            ( { model | deleteTarget = Just id }, Cmd.none )

        CancelDelete ->
            ( { model | deleteTarget = Nothing }, Cmd.none )

        ConfirmDelete ->
            case ( model.deleteTarget, model.data ) of
                ( Just id, Success snap ) ->
                    ( { model | deleteTarget = Nothing, busy = True }
                    , Api.deleteRule id snap.rulesVersion GotSaved
                    )

                _ ->
                    ( { model | deleteTarget = Nothing }, Cmd.none )

        LoadMoreMessages ->
            case ( model.selected, model.messages ) of
                ( Just ruleId, messages ) ->
                    ( { model | messages = { messages | loading = True } }
                    , Api.getRuleMessages { ruleId = ruleId, before = messages.cursor } GotMoreMessages
                    )

                _ ->
                    ( model, Cmd.none )

        GotMessages result ->
            case result of
                Success payload ->
                    ( { model
                        | messages =
                            { items = payload.messages
                            , cursor = payload.nextBefore
                            , hasMore = payload.hasMore
                            , loading = False
                            , error = Nothing
                            }
                      }
                    , Cmd.none
                    )

                Failure err ->
                    ( { model | messages = { emptyMessages | error = Just (Api.errorMessage err) } }, Cmd.none )

                _ ->
                    ( model, Cmd.none )

        GotMoreMessages result ->
            case result of
                Success payload ->
                    ( { model
                        | messages =
                            { items = model.messages.items ++ payload.messages
                            , cursor = payload.nextBefore
                            , hasMore = payload.hasMore
                            , loading = False
                            , error = Nothing
                            }
                      }
                    , Cmd.none
                    )

                Failure err ->
                    let
                        current =
                            model.messages
                    in
                    ( { model | messages = { current | loading = False, error = Just (Api.errorMessage err) } }, Cmd.none )

                _ ->
                    ( model, Cmd.none )

        GotChats result ->
            case result of
                Success chats ->
                    ( { model | editor = Maybe.map (Editor.setChats chats) model.editor }, Cmd.none )

                _ ->
                    ( model, Cmd.none )

        DismissNotice ->
            ( { model | notice = Nothing }, Cmd.none )



-- UPDATE HELPERS


fetchEvents : Live -> Cmd Msg
fetchEvents live =
    Api.getEvents
        (if live.loaded then
            Just live.lastTs

         else
            Nothing
        )
        GotEvents


stepRuleId : Int -> Maybe String -> List String -> Maybe String
stepRuleId delta selected ids =
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


swapRule : String -> String -> List Api.Rule -> List Api.Rule
swapRule id direction rules =
    let
        movable =
            List.filter (\r -> not r.isCatchAll) rules

        catchAlls =
            List.filter .isCatchAll rules

        swappedMovable =
            case direction of
                "up" ->
                    swapUp id movable

                "down" ->
                    swapDown id movable

                _ ->
                    movable
    in
    swappedMovable ++ catchAlls


swapUp : String -> List Api.Rule -> List Api.Rule
swapUp id rules =
    case rules of
        a :: b :: rest ->
            if b.id == id then
                b :: a :: rest

            else
                a :: swapUp id (b :: rest)

        _ ->
            rules


swapDown : String -> List Api.Rule -> List Api.Rule
swapDown id rules =
    case rules of
        a :: b :: rest ->
            if a.id == id then
                b :: a :: rest

            else
                a :: swapDown id (b :: rest)

        _ ->
            rules


isConflict : Api.ApiError -> Bool
isConflict err =
    case err of
        Api.Fault { code } ->
            code == "conflict"

        _ ->
            False


enabledChannels : Model -> Maybe Api.Enabled
enabledChannels model =
    model.data |> RemoteData.toMaybe |> Maybe.map .enabled


ruleAt : String -> Model -> Maybe Api.Rule
ruleAt id model =
    model.data
        |> RemoteData.toMaybe
        |> Maybe.map .rules
        |> Maybe.withDefault []
        |> List.filter (\r -> r.id == id)
        |> List.head



-- VIEW


view : Model -> Html Msg
view model =
    div [ class "workbench-shell" ]
        [ div
            [ classList
                [ ( "workbench", True )
                , ( if model.selected == Nothing && model.editor == Nothing then
                        "no-selection"

                    else
                        "has-selection"
                  , True
                  )
                , ( "is-editing", model.editor /= Nothing )
                ]
            ]
            [ pipelinePane model
            , inspectorPane model
            ]
        , liveFeed model
        , deleteModal model
        ]


snapshotOf : Model -> Maybe Api.Snapshot
snapshotOf model =
    RemoteData.toMaybe model.data


noticeBanner : Model -> Html Msg
noticeBanner model =
    case model.notice of
        Just ( kind, message ) ->
            div [ onClick DismissNotice, title "Click to dismiss" ]
                [ Ui.banner kind message ]

        Nothing ->
            text ""


pipelinePane : Model -> Html Msg
pipelinePane model =
    let
        rules =
            snapshotOf model |> Maybe.map .rules |> Maybe.withDefault []

        movableCount =
            List.length (List.filter (\r -> not r.isCatchAll) rules)
    in
    aside [ class "pipeline-pane" ]
        [ header [ class "pane-head" ]
            [ div []
                [ h3 [ class "pane-title" ] [ text "Routing pipeline" ]
                , span [ class "pane-sub" ]
                    [ text (String.fromInt (List.length rules) ++ ruleWord (List.length rules) ++ " · evaluated top → bottom") ]
                ]
            , div [ class "pane-actions" ]
                [ Ui.button
                    { label = "+ Rule"
                    , onClick = OpenNewRule
                    , variant = Primary
                    , busy = False
                    , disabled = False
                    }
                ]
            ]
        , div [ class "pipeline-tester" ] [ testerCard model rules ]
        , div [ class "pipeline-list" ]
            (List.indexedMap (pipelineCard model movableCount) rules)
        ]


ruleWord : Int -> String
ruleWord n =
    if n == 1 then
        " rule"

    else
        " rules"


pipelineCard : Model -> Int -> Int -> Api.Rule -> Html Msg
pipelineCard model movableCount index rule =
    let
        selected =
            model.selected == Just rule.id

        firing =
            String.contains "@" model.tester
                && (firstMatch model (rulesOf model) |> Maybe.map .id)
                == Just rule.id

        dim =
            String.contains "@" model.tester
                && (firstMatch model (rulesOf model) |> Maybe.map .id)
                /= Just rule.id

        orderClass =
            if rule.isCatchAll then
                "rc-order rc-catch"

            else
                case Api.actionKind rule.action of
                    Api.ForwardKind ->
                        "rc-order rc-fwd"

                    Api.StoreKind ->
                        "rc-order rc-store"

                    Api.DropKind ->
                        "rc-order rc-drop"
    in
    div
        [ classList
            [ ( "rule-card", True )
            , ( "is-selected", selected )
            , ( "is-firing", firing )
            , ( "is-dim", dim )
            ]
        ]
        [ button
            [ class "rc-main"
            , type_ "button"
            , onClick (SelectRule (Just rule.id))
            ]
            [ span [ class orderClass ] [ text (pad2 (index + 1)) ]
            , span [ class "rc-body" ]
                [ span [ class "rc-row1" ]
                    [ span [ class "rc-label" ] [ text rule.displayLabel ]
                    ]
                , span [ class "rc-row2" ]
                    [ span [ class "rc-pattern" ] (patternPieces rule.localPattern rule.domainPattern)
                    , span [ class "rc-action" ] [ text (Api.actionSummary rule.action) ]
                    ]
                ]
            ]
        , if rule.isCatchAll then
            text ""

          else
            div [ class "rc-tools" ]
                [ Ui.iconButton
                    { icon = Icons.arrowUp
                    , label = "Move up"
                    , onClick = Reorder rule.id "up"
                    , variant = Ghost
                    , disabled = model.busy || index == 0
                    , small = True
                    }
                , Ui.iconButton
                    { icon = Icons.arrowDown
                    , label = "Move down"
                    , onClick = Reorder rule.id "down"
                    , variant = Ghost
                    , disabled = model.busy || index >= movableCount - 1
                    , small = True
                    }
                ]
        ]


pad2 : Int -> String
pad2 n =
    if n < 10 then
        "0" ++ String.fromInt n

    else
        String.fromInt n


patternPieces : String -> String -> List (Html Msg)
patternPieces local domain =
    [ patternPiece local
    , span [ class "rc-at" ] [ text "@" ]
    , patternPiece domain
    ]


patternPiece : String -> Html Msg
patternPiece s =
    if s == "*" then
        span [ class "rc-star" ] [ text s ]

    else
        span [] [ text s ]


testerCard : Model -> List Api.Rule -> Html Msg
testerCard model rules =
    let
        firing =
            firstMatch model rules

        selectedRule_ =
            Maybe.andThen (\id -> List.filter (\r -> r.id == id) rules |> List.head) model.selected

        thisMatches =
            selectedRule_ |> Maybe.map (matchesAddress model.tester) |> Maybe.withDefault False

        -- An inbound address is tested against the routing table; anything else
        -- is treated as a destination and looked up across the rules.
        addressish =
            isEmailish model.tester

        delivering =
            rulesDeliveringTo model.tester rules
    in
    div [ class "tester-card" ]
        [ div [ class "tester-head" ]
            [ span [ class "tester-title" ] [ Icons.send, text "Tester" ] ]
        , div [ class "tester-input-wrap" ]
            [ span [ class "tester-prefix" ] [ Icons.search ]
            , input
                [ class "tester-input"
                , type_ "text"
                , placeholder "address or destination"
                , value model.tester
                , onInput SetTester
                , A.autocomplete False
                , A.spellcheck False
                ]
                []
            ]
        , if String.isEmpty model.tester then
            text ""

          else
            div [ class "tester-result" ]
                ([ if addressish then
                    div [ class "tester-row" ]
                        [ span [ class "tester-label" ] [ text "Would fire" ]
                        , case firing of
                            Just rule ->
                                if model.selected == Just rule.id then
                                    Ui.chip { label = "this rule · " ++ Api.actionSummary rule.action, variant = "ok", dot = True }

                                else
                                    button
                                        [ class "tester-link"
                                        , type_ "button"
                                        , onClick (SelectRule (Just rule.id))
                                        ]
                                        [ Icons.bolt, text rule.displayLabel, Icons.chevronRight ]

                            Nothing ->
                                Ui.chip { label = "no rule matches · bounced", variant = "bad", dot = True }
                        ]

                   else
                    text ""
                 , case ( addressish, selectedRule_ ) of
                    ( True, Just _ ) ->
                        div [ class "tester-row" ]
                            [ span [ class "tester-label" ] [ text "This rule" ]
                            , if thisMatches then
                                Ui.chip { label = "matches", variant = "ok", dot = True }

                              else
                                Ui.chip { label = "no match", variant = "muted", dot = False }
                            ]

                    _ ->
                        text ""
                 , div [ class "tester-row" ]
                    [ span [ class "tester-label" ] [ text "Used by" ]
                    , if List.isEmpty delivering then
                        Ui.chip { label = "no rule uses it", variant = "muted", dot = False }

                      else
                        div [ class "tester-links" ] (List.map deliveringLink delivering)
                    ]
                 ]
                    ++ (if addressish && thisMatches && firing /= selectedRule_ then
                            [ div [ class "tester-note" ] [ text "This rule matches, but a higher rule catches first." ] ]

                        else
                            []
                       )
                )
        ]


deliveringLink : Api.Rule -> Html Msg
deliveringLink rule =
    button
        [ class "tester-link"
        , type_ "button"
        , onClick (SelectRule (Just rule.id))
        ]
        [ Icons.bolt, text rule.displayLabel, Icons.chevronRight ]


{-| Rules that deliver to `input`, treated as a destination: an email address,
a Telegram chat id, or a Discord channel id. The `kind:value` forms the
destination builder uses also match.
-}
rulesDeliveringTo : String -> List Api.Rule -> List Api.Rule
rulesDeliveringTo input rules =
    let
        needle =
            String.toLower (String.trim input)
    in
    if String.isEmpty needle then
        []

    else
        List.filter (destinationMatches needle) rules


destinationMatches : String -> Api.Rule -> Bool
destinationMatches needle rule =
    List.any (destinationValueMatches needle) (Api.destinationsOf rule.action)


destinationValueMatches : String -> Api.Destination -> Bool
destinationValueMatches needle destination =
    case destination of
        Api.Email { address } ->
            String.toLower (String.trim address) == needle

        Api.Telegram { chatId } ->
            needle == chatId || needle == "telegram:" ++ chatId || needle == "tg:" ++ chatId

        Api.Discord { channelId } ->
            needle == channelId || needle == "discord:" ++ channelId || needle == "dc:" ++ channelId


isEmailish : String -> Bool
isEmailish address =
    case addressParts address of
        Just ( local, domain ) ->
            not (String.isEmpty local) && not (String.isEmpty domain)

        Nothing ->
            False


addressParts : String -> Maybe ( String, String )
addressParts address =
    let
        parts =
            String.split "@" address
    in
    case parts of
        local :: rest ->
            if List.isEmpty rest then
                Nothing

            else
                Just ( String.trim local, String.trim (String.join "@" rest) )

        [] ->
            Nothing


matchesAddress : String -> Api.Rule -> Bool
matchesAddress address rule =
    case addressParts address of
        Just ( local, domain ) ->
            Glob.match rule.localPattern local && Glob.match rule.domainPattern domain

        Nothing ->
            False


firstMatch : Model -> List Api.Rule -> Maybe Api.Rule
firstMatch model rules =
    if isEmailish model.tester then
        List.filter (matchesAddress model.tester) rules |> List.head

    else
        Nothing


rulesOf : Model -> List Api.Rule
rulesOf model =
    snapshotOf model |> Maybe.map .rules |> Maybe.withDefault []


inspectorPane : Model -> Html Msg
inspectorPane model =
    case ( model.editor, enabledChannels model ) of
        ( Just editor, Just enabled ) ->
            Html.map EditorMsg (Editor.view enabled editor)

        _ ->
            case Maybe.andThen (\id -> ruleAt id model) model.selected of
                Nothing ->
                    overviewPane model

                Just rule ->
                    section [ class "inspector-pane" ]
                        [ inspectorHead model rule
                        , div [ class "inspector-body" ]
                            [ noticeBanner model
                            , statStrip model rule
                            , issuesCard rule
                            , destinationsCard rule
                            , actionCard rule
                            , messagesCard model rule
                            ]
                        ]


overviewPane : Model -> Html Msg
overviewPane model =
    let
        stats =
            snapshotOf model |> Maybe.andThen .stats

        pending =
            model.live.pending

        pendingTotal =
            pending.queued + pending.deadLettered

        rules =
            rulesOf model

        rulesWithIssues =
            List.filter (\r -> not (List.isEmpty r.issues)) rules

        fwdVal =
            stats |> Maybe.map (\s -> Format.count s.forwardedTotal) |> Maybe.withDefault "—"

        strVal =
            stats |> Maybe.map (\s -> Format.count s.storedTotal) |> Maybe.withDefault "—"

        drpVal =
            stats |> Maybe.map (\s -> Format.count s.droppedTotal) |> Maybe.withDefault "—"

        senders =
            stats |> Maybe.map .topSenders |> Maybe.withDefault []
    in
    section [ class "inspector-pane" ]
        [ header [ class "inspector-head" ]
            [ div [ class "ih-meta" ]
                [ span [ class "ih-id" ] [ text "7-day routing summary" ]
                , h1 [ class "ih-title" ] [ text "Overview" ]
                ]
            , div [ class "ih-actions" ]
                [ Ui.button
                    { label = "+ New rule"
                    , onClick = OpenNewRule
                    , variant = Primary
                    , busy = False
                    , disabled = False
                    }
                ]
            ]
        , div [ class "overview-pane" ]
            [ noticeBanner model
            , div [ class "stat-strip" ]
                [ Ui.stat { key = "forwarded · 7d", value = fwdVal, tone = "fwd", sub = "delivered" }
                , Ui.stat { key = "stored · 7d", value = strVal, tone = "plain", sub = "kept in inbox" }
                , Ui.stat { key = "dropped · 7d", value = drpVal, tone = "plain", sub = "discarded" }
                , Ui.stat
                    { key = "pending / DLQ"
                    , value = String.fromInt pendingTotal
                    , tone =
                        if pending.deadLettered > 0 then
                            "bad"

                        else
                            "plain"
                    , sub = String.fromInt pending.queued ++ " queued · " ++ String.fromInt pending.deadLettered ++ " DLQ"
                    }
                ]
            , if List.isEmpty rulesWithIssues then
                text ""

              else
                Ui.card
                    [ div [ class "card-head" ]
                        [ h3 [] [ text ("Validation issues (" ++ String.fromInt (List.length rulesWithIssues) ++ ")") ] ]
                    , div [ class "issues" ]
                        (List.concatMap
                            (\rule ->
                                List.map
                                    (\issue ->
                                        button
                                            [ classList [ ( "issue", True ), ( "issue-" ++ issue.level, True ) ]
                                            , type_ "button"
                                            , onClick (SelectRule (Just rule.id))
                                            ]
                                            [ span [ class "issue-label" ] [ text rule.displayLabel ]
                                            , span [] [ text issue.message ]
                                            ]
                                    )
                                    rule.issues
                            )
                            rulesWithIssues
                        )
                    ]
            , Ui.card
                [ div [ class "card-head" ]
                    [ h3 [] [ text "Top senders · 7d" ]
                    , span [ class "card-count" ] [ text "forwarded" ]
                    ]
                , if List.isEmpty senders then
                    div [ class "empty-inline" ]
                        [ text "No forwarded senders in the last 7 days." ]

                  else
                    div [ class "sender-list" ]
                        (List.map topSenderRow senders)
                ]
            , Ui.card
                [ div [ class "card-head" ]
                    [ h3 [] [ text "Quick navigation" ] ]
                , p [ class "card-text" ]
                    [ text "Select any rule on the left to inspect its destinations and stored emails. Use ↑/↓ or j/k to step through rules, e to edit the selected rule, n to create a new rule, and Esc to return here." ]
                ]
            ]
        ]


topSenderRow : Api.TopSender -> Html Msg
topSenderRow sender =
    button
        [ class "sender-row"
        , type_ "button"
        , onClick (SetTester sender.address)
        , title "Test this address in the pipeline tester"
        ]
        [ span [ class "sender-addr" ] [ text sender.address ]
        , span [ class "sender-count" ] [ text (Format.count sender.n) ]
        ]


inspectorHead : Model -> Api.Rule -> Html Msg
inspectorHead model rule =
    let
        actionTag =
            case Api.actionKind rule.action of
                Api.ForwardKind ->
                    let
                        name =
                            if Api.isForwardStore rule.action then
                                "store & forward"

                            else
                                "forward"
                    in
                    if anyProxy rule.action then
                        Ui.chip { label = name ++ " · proxy", variant = "proxy", dot = False }

                    else
                        Ui.chip { label = name, variant = "info", dot = False }

                Api.StoreKind ->
                    Ui.chip { label = "store", variant = "info", dot = False }

                Api.DropKind ->
                    if rule.isCatchAll then
                        Ui.chip { label = "drop · catch-all", variant = "warn", dot = False }

                    else
                        Ui.chip { label = "drop", variant = "bad", dot = False }
    in
    header [ class "inspector-head" ]
        [ button [ class "inspector-back", type_ "button", onClick (SelectRule Nothing) ]
            [ Icons.chevronRight, text "Rules" ]
        , div [ class "ih-meta" ]
            [ div [ class "ih-idrow" ]
                [ span [ class "ih-id" ] [ text ("rule · " ++ String.left 8 rule.id) ]
                , actionTag
                ]
            , h1 [ class "ih-title" ] [ text rule.displayLabel ]
            , span [ class "ih-pattern" ] (patternPieces rule.localPattern rule.domainPattern)
            ]
        , div [ class "ih-actions" ]
            [ Ui.button
                { label = "Edit"
                , onClick = OpenEditRule rule.id
                , variant = Secondary
                , busy = False
                , disabled = False
                }
            , if rule.isCatchAll then
                text ""

              else
                Ui.button
                    { label = "Delete"
                    , onClick = AskDelete rule.id
                    , variant = Danger
                    , busy = False
                    , disabled = model.busy
                    }
            ]
        ]


anyProxy : Api.Action -> Bool
anyProxy action =
    List.any
        (\d ->
            case d of
                Api.Email { proxy } ->
                    proxy

                _ ->
                    False
        )
        (Api.destinationsOf action)


destinationCount : Api.Action -> Int
destinationCount action =
    List.length (Api.destinationsOf action)


statStrip : Model -> Api.Rule -> Html Msg
statStrip model rule =
    let
        matches =
            case rule.matches7d of
                Just n ->
                    Format.count n

                Nothing ->
                    "—"

        matchesSub =
            if rule.matches7d == Nothing then
                "no recent traffic"

            else
                "matches"

        last =
            case rule.lastMatchS of
                Just ts ->
                    Format.relative model.now (ts * 1000)

                Nothing ->
                    "—"

        channels =
            case Api.actionKind rule.action of
                Api.ForwardKind ->
                    let
                        ( e, t, d ) =
                            channelCounts (Api.destinationsOf rule.action)
                    in
                    String.join " · "
                        (List.filterMap identity
                            [ maybeCount e "email", maybeCount t "tg", maybeCount d "dc" ]
                        )

                _ ->
                    "—"
    in
    div [ class "stat-strip" ]
        [ Ui.stat { key = "matches · 7d", value = matches, tone = "fwd", sub = matchesSub }
        , Ui.stat { key = "last match", value = last, tone = "plain", sub = "" }
        , Ui.stat { key = "destinations", value = String.fromInt (destinationCount rule.action), tone = "plain", sub = "" }
        , Ui.stat { key = "channels", value = channels, tone = "small", sub = "" }
        ]


channelCounts : List Api.Destination -> ( Int, Int, Int )
channelCounts destinations =
    List.foldl
        (\d ( e, t, dc ) ->
            case d of
                Api.Email _ ->
                    ( e + 1, t, dc )

                Api.Telegram _ ->
                    ( e, t + 1, dc )

                Api.Discord _ ->
                    ( e, t, dc + 1 )
        )
        ( 0, 0, 0 )
        destinations


maybeCount : Int -> String -> Maybe String
maybeCount n name =
    if n > 0 then
        Just (String.fromInt n ++ " " ++ name)

    else
        Nothing


issuesCard : Api.Rule -> Html Msg
issuesCard rule =
    if List.isEmpty rule.issues then
        text ""

    else
        Ui.card
            [ div [ class "card-head" ] [ h3 [] [ text "Validation" ] ]
            , div [ class "issues" ]
                (List.map
                    (\issue ->
                        div [ classList [ ( "issue", True ), ( "issue-" ++ issue.level, True ) ] ]
                            [ span [ class "issue-label" ] [ text (String.toUpper issue.level) ]
                            , span [] [ text issue.message ]
                            ]
                    )
                    rule.issues
                )
            ]


destinationsCard : Api.Rule -> Html Msg
destinationsCard rule =
    case Api.actionKind rule.action of
        Api.ForwardKind ->
            let
                destinations =
                    Api.destinationsOf rule.action
            in
            Ui.card
                [ div [ class "card-head" ]
                    [ h3 [] [ text ("Destinations (" ++ String.fromInt (List.length destinations) ++ ")") ] ]
                , if List.isEmpty destinations then
                    div [ class "empty-inline" ] [ text "No destinations yet — this forward does nothing until you add one." ]

                  else
                    div [ class "dest-grid" ] (List.map (destinationCard (Api.isForwardStore rule.action)) destinations)
                ]

        _ ->
            text ""


destinationCard : Bool -> Api.Destination -> Html Msg
destinationCard stores destination =
    let
        ( kind, value_, badges ) =
            case destination of
                Api.Email { address, proxy } ->
                    ( "email"
                    , address
                    , (if stores then
                        [ storeBadge False ]

                       else
                        []
                      )
                        ++ (if proxy then
                                [ plainBadge "proxy" ]

                            else
                                []
                           )
                    )

                Api.Telegram { chatId } ->
                    ( "telegram"
                    , chatId
                    , [ storeBadge True ]
                    )

                Api.Discord { channelId } ->
                    ( "discord"
                    , channelId
                    , [ storeBadge True ]
                    )

        icon =
            case kind of
                "email" ->
                    Icons.mail

                "telegram" ->
                    Icons.telegram

                _ ->
                    Icons.discord
    in
    div [ classList [ ( "dest-card", True ), ( kind, True ) ] ]
        [ span [ class "dest-icon" ] [ icon ]
        , div [ class "dest-meta" ]
            [ span [ class "dest-kind" ] [ text kind ]
            , span [ class "dest-value" ] [ text value_ ]
            ]
        , span [ class "dest-mods" ] badges
        ]


actionCard : Api.Rule -> Html Msg
actionCard rule =
    case rule.action of
        Api.Forward _ ->
            text ""

        Api.ForwardStore _ ->
            text ""

        Api.Store { persist } ->
            Ui.card
                [ div [ class "card-head" ] [ h3 [] [ text "Store" ] ]
                , p [ class "card-text" ]
                    [ text
                        ("Inbound mail matching this rule is recorded as a store event. "
                            ++ (if persist then
                                    "The message body is persisted to the database."

                                else
                                    "The message body is not persisted."
                               )
                        )
                    ]
                ]

        Api.Drop ->
            Ui.card
                [ div [ class "card-head" ] [ h3 [] [ text "Drop" ] ]
                , p [ class "card-text" ]
                    [ text
                        ("Inbound mail matching this rule is silently discarded — no notification, no bounce."
                            ++ (if rule.isCatchAll then
                                    " This is the pinned catch-all, which always sits last and can't be deleted or moved."

                                else
                                    ""
                               )
                        )
                    ]
                ]


messagesCard : Model -> Api.Rule -> Html Msg
messagesCard model _ =
    let
        messages =
            model.messages
    in
    div [ class "card messages-card" ]
        [ div [ class "card-head" ]
            [ h3 [] [ text "Stored emails" ]
            , span [ class "card-count" ] [ text (countLabel messages) ]
            ]
        , if messages.loading && List.isEmpty messages.items then
            Ui.spinner

          else if List.isEmpty messages.items then
            div [ class "empty-inline" ]
                [ text
                    (Maybe.withDefault
                        "No stored emails for this rule yet. They appear here when the rule stores mail or forwards to Telegram or Discord."
                        messages.error
                    )
                ]

          else
            div [ class "message-list" ] (List.map messageRow messages.items)
        , if messages.hasMore then
            div [ class "load-more" ]
                [ Ui.button
                    { label = "Load more"
                    , onClick = LoadMoreMessages
                    , variant = Ghost
                    , busy = messages.loading
                    , disabled = False
                    }
                ]

          else
            text ""
        ]


countLabel : Messages -> String
countLabel messages =
    if messages.hasMore then
        String.fromInt (List.length messages.items) ++ "+"

    else
        String.fromInt (List.length messages.items)


{-| One stored email, as a single line. Reading happens in the inbox, so the
row is a link to `/inbox/{id}` rather than an inline viewer.
-}
messageRow : Api.MessageItem -> Html Msg
messageRow item =
    a
        [ class "message-row"
        , href ("/inbox/" ++ item.id)
        ]
        [ span [ class "message-ts" ]
            [ text (item.createdAt |> Maybe.map Format.dateShort |> Maybe.withDefault "—") ]
        , span [ class "message-from" ] [ text item.sender ]
        , span [ class "message-subj" ] [ text item.subject ]
        , span [ class "message-caret" ] [ Icons.chevronRight ]
        ]



-- LIVE FEED


liveFeed : Model -> Html Msg
liveFeed model =
    let
        live =
            model.live

        visible =
            if live.filter == "all" then
                live.events

            else
                List.filter (\e -> e.kind == live.filter) live.events

        pendingTotal =
            live.pending.queued + live.pending.deadLettered
    in
    div [ classList [ ( "live-feed", True ), ( "is-collapsed", live.collapsed ) ] ]
        [ div [ class "live-bar" ]
            [ button [ class "live-toggle", type_ "button", onClick ToggleLive ]
                [ span [ class "live-caret" ]
                    [ if live.collapsed then
                        Icons.chevronRight

                      else
                        Icons.chevronDown
                    ]
                , span [ class "live-title" ] [ text "Live feed" ]
                , if live.collapsed then
                    Icons.play

                  else
                    Icons.pause
                ]
            , span
                [ classList
                    [ ( "chip", True )
                    , ( "ok", not live.collapsed )
                    , ( "muted", live.collapsed )
                    ]
                ]
                (if live.collapsed then
                    [ text "paused" ]

                 else
                    [ span [ class "dot" ] [], text "streaming" ]
                )
            , if live.collapsed then
                text ""

              else
                div [ class "live-filters" ] (List.map (filterChip live.filter) filters)
            , div [ class "live-spacer" ] []
            , a
                [ classList [ ( "live-pending", True ), ( "is-empty", pendingTotal == 0 ) ]
                , href (Route.toPath Route.Pending)
                ]
                (pendingPieces live.pending)
            , span [ class "live-count" ] [ text (String.fromInt (List.length visible) ++ " events") ]
            ]
        , if live.collapsed then
            text ""

          else
            div [ class "live-body" ]
                (if List.isEmpty visible then
                    [ div [ class "live-empty" ] [ text "No events yet — they'll appear here as mail flows." ] ]

                 else
                    List.map (liveRow model.zone) (List.take 80 visible)
                )
        ]


pendingPieces : Api.PendingCounts -> List (Html Msg)
pendingPieces pending =
    List.filterMap identity
        [ if pending.queued > 0 then
            Just (span [] [ text (String.fromInt pending.queued ++ " queued") ])

          else
            Nothing
        , if pending.deadLettered > 0 then
            Just (span [ class "live-dlq" ] [ text (String.fromInt pending.deadLettered ++ " DLQ") ])

          else
            Nothing
        ]


filters : List String
filters =
    [ "all", "forward", "store", "drop", "reply", "reject", "error" ]


filterChip : String -> String -> Html Msg
filterChip active value_ =
    button
        [ classList [ ( "filter-chip", True ), ( "is-active", active == value_ ) ]
        , type_ "button"
        , onClick (SetLiveFilter value_)
        ]
        [ text value_ ]


liveRow : Int -> Api.Event -> Html Msg
liveRow offset event =
    div [ classList [ ( "live-row", True ), ( "k-" ++ event.kind, True ) ] ]
        [ span [ class "lr-ts" ] [ text (Format.clock offset event.ts) ]
        , span [ class "lr-kind" ] [ text (String.toUpper event.kind) ]
        , span [ class "lr-from" ] [ text event.from ]
        , span [ class "lr-arrow" ] [ text "→" ]
        , span [ class "lr-to" ] [ text event.to ]
        , span [ class "lr-ch" ] (List.map channelDot event.channels)
        , span [ class "lr-size" ] [ text (Format.bytes event.sizeBytes) ]
        , case event.error of
            Just err ->
                span [ class "lr-err", title err ] [ text err ]

            Nothing ->
                text ""
        ]


channelDot : String -> Html Msg
channelDot channel =
    span
        [ classList [ ( "lr-chdot", True ), ( "ch-" ++ channel, True ) ]
        , attribute "title" channel
        ]
        [ text (String.left 1 (String.toUpper channel)) ]



-- MODALS & BADGES


deleteModal : Model -> Html Msg
deleteModal model =
    case model.deleteTarget of
        Just id ->
            Ui.modal
                { title = "Delete rule"
                , onClose = CancelDelete
                , body =
                    [ p []
                        [ text
                            ("Delete "
                                ++ (ruleAt id model |> Maybe.map .displayLabel |> Maybe.withDefault "this rule")
                                ++ "? This can't be undone."
                            )
                        ]
                    ]
                , footer =
                    [ Ui.button { label = "Cancel", onClick = CancelDelete, variant = Ghost, busy = False, disabled = False }
                    , Ui.button { label = "Delete", onClick = ConfirmDelete, variant = Danger, busy = model.busy, disabled = False }
                    ]
                }

        Nothing ->
            text ""


storeBadge : Bool -> Html msg
storeBadge locked =
    span
        [ classList
            [ ( "dest-chip-mod", True )
            , ( "is-store", True )
            , ( if locked then
                    "is-locked"

                else
                    "is-on"
              , True
              )
            ]
        , A.title
            (if locked then
                "Always stored"

             else
                "Stored"
            )
        ]
        [ if locked then
            Icons.lock

          else
            Icons.check
        , text "store"
        ]


plainBadge : String -> Html msg
plainBadge label_ =
    span [ class "dest-chip-mod" ] [ text label_ ]
