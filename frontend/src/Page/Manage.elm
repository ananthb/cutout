module Page.Manage exposing (Model, Msg, init, subscriptions, update, view)

{-| The workbench: the routing pipeline, the inspector, the live feed and the
rule editor.

The page owns its copy of the server state (`data`). Every rule mutation returns
a fresh snapshot, so there is exactly one way the rules list changes: a whole
snapshot replaces the old one. Nothing here patches a list in place.

The tester runs the routing engine's glob matcher in Elm on every keystroke and
glows the rule that would fire, so you can see the pipeline decide without a
request.

-}

import Api
import Browser.Dom as Dom
import Browser.Navigation as Nav
import Format
import Glob
import Html exposing (Html, a, aside, button, div, h1, h3, header, iframe, input, label, p, pre, section, span, text)
import Html.Attributes as A exposing (attribute, class, classList, href, placeholder, title, type_, value)
import Html.Events as E exposing (onClick, onInput)
import Icons
import Json.Decode as D
import Ports
import RemoteData exposing (RemoteData(..))
import Route
import Task
import Time
import Ui exposing (ButtonVariant(..))



-- MODEL


type alias Model =
    { data : Api.Data Api.Snapshot
    , selected : Maybe String
    , tester : String
    , editor : Maybe Editor
    , live : Live
    , messages : Messages
    , expanded : Maybe String
    , viewing : Api.Data Api.RenderedMessage
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


type alias Editor =
    { id : Maybe String
    , label : String
    , local : String
    , domain : String
    , actionType : String
    , destinations : List Api.Destination
    , persist : Bool
    , storeForward : Bool
    , destKind : String
    , destValue : String
    , destProxy : Bool
    , destAuth : String
    , destError : String
    , readers : List String
    , readerValue : String
    , readerError : String
    , chats : List Api.TelegramChat
    , submitting : Bool
    }


emptyMessages : Messages
emptyMessages =
    { items = [], cursor = Nothing, hasMore = False, loading = False, error = Nothing }


emptyEditor : Editor
emptyEditor =
    { id = Nothing
    , label = ""
    , local = "*"
    , domain = "*"
    , actionType = "forward"
    , destinations = []
    , persist = False
    , storeForward = False
    , destKind = "email"
    , destValue = ""
    , destProxy = False
    , destAuth = "access"
    , destError = ""
    , readers = []
    , readerValue = ""
    , readerError = ""
    , chats = []
    , submitting = False
    }


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
      , expanded = Nothing
      , viewing = NotAsked
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
subscriptions _ =
    Sub.batch
        [ Time.every 1000 Tick
        , Time.every 2000 (\_ -> PollEvents)
        , Time.every 10000 (\_ -> PollPending)
        , Ports.zone Zone
        ]



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
    | Reorder String String
    | OpenNewRule
    | OpenEditRule String
    | CloseEditor
    | EditorLabel String
    | EditorLocal String
    | EditorDomain String
    | EditorAt
    | FocusResult (Result Dom.Error ())
    | EditorActionType String
    | EditorPersist Bool
    | EditorStore Bool
    | EditorDestKind String
    | EditorDestValue String
    | EditorDestProxy Bool
    | EditorDestAuth String
    | EditorAddDest
    | EditorRemoveDest Int
    | EditorReaderValue String
    | EditorAddReader
    | EditorRemoveReader Int
    | EditorSubmit
    | GotSaved (Api.Data Api.Snapshot)
    | GotRefreshed (Api.Data Api.Snapshot)
    | AskDelete String
    | CancelDelete
    | ConfirmDelete
    | ToggleMessage String
    | GotMessage (Api.Data Api.RenderedMessage)
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
            , Api.getEvents
                (if model.live.loaded then
                    Just model.live.lastTs

                 else
                    Nothing
                )
                GotEvents
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
            ( { model | live = model.live |> (\l -> { l | collapsed = not l.collapsed }) }, Cmd.none )

        SetLiveFilter f ->
            ( { model | live = model.live |> (\l -> { l | filter = f }) }, Cmd.none )

        SetTester v ->
            ( { model | tester = v }, Cmd.none )

        SelectRule target ->
            if target == model.selected then
                ( { model | selected = Nothing, messages = emptyMessages, expanded = Nothing, viewing = NotAsked }, Cmd.none )

            else
                ( { model
                    | selected = target
                    , messages = { emptyMessages | loading = target /= Nothing }
                    , expanded = Nothing
                    , viewing = NotAsked
                    , notice = Nothing
                  }
                , case target of
                    Just id ->
                        Api.getRuleMessages { ruleId = id, before = Nothing } GotMessages

                    Nothing ->
                        Cmd.none
                )

        Reorder id direction ->
            case model.data of
                Success snap ->
                    ( { model | busy = True }
                    , Api.reorderRule { id = id, direction = direction, rulesVersion = snap.rulesVersion } GotSaved
                    )

                _ ->
                    ( model, Cmd.none )

        OpenNewRule ->
            ( { model | editor = Just emptyEditor, notice = Nothing }
            , Api.getTelegramChats GotChats
            )

        OpenEditRule id ->
            case ruleAt id model of
                Just rule ->
                    ( { model | editor = Just (editorFrom rule), notice = Nothing }
                    , Api.getTelegramChats GotChats
                    )

                Nothing ->
                    ( model, Cmd.none )

        CloseEditor ->
            ( { model | editor = Nothing }, Cmd.none )

        EditorLabel v ->
            ( mapEditor (\e -> { e | label = v }) model, Cmd.none )

        EditorLocal v ->
            ( mapEditor (\e -> { e | local = v }) model, Cmd.none )

        EditorDomain v ->
            ( mapEditor (\e -> { e | domain = v }) model, Cmd.none )

        EditorAt ->
            -- Typing `@` in the local part jumps to the domain field.
            ( model, Dom.focus "rule-domain" |> Task.attempt FocusResult )

        FocusResult _ ->
            ( model, Cmd.none )

        EditorActionType v ->
            ( mapEditor (\e -> { e | actionType = v }) model, Cmd.none )

        EditorPersist v ->
            ( mapEditor (\e -> { e | persist = v }) model, Cmd.none )

        EditorStore v ->
            ( mapEditor (\e -> { e | storeForward = v }) model, Cmd.none )

        EditorDestKind v ->
            ( mapEditor (\e -> { e | destKind = v, destValue = "", destError = "" }) model, Cmd.none )

        EditorDestValue v ->
            ( mapEditor (\e -> { e | destValue = v, destError = "" }) model, Cmd.none )

        EditorDestProxy v ->
            ( mapEditor (\e -> { e | destProxy = v }) model, Cmd.none )

        EditorDestAuth v ->
            ( mapEditor (\e -> { e | destAuth = v }) model, Cmd.none )

        EditorAddDest ->
            case model.editor of
                Just editor ->
                    case enabledChannels model of
                        Just enabled ->
                            case parseDestination enabled editor of
                                Ok dest ->
                                    ( mapEditor
                                        (\e ->
                                            { e
                                                | destinations = e.destinations ++ [ dest ]
                                                , destValue = ""
                                                , destError = ""
                                            }
                                        )
                                        model
                                    , Cmd.none
                                    )

                                Err err ->
                                    ( mapEditor (\e -> { e | destError = err }) model, Cmd.none )

                        Nothing ->
                            ( model, Cmd.none )

                Nothing ->
                    ( model, Cmd.none )

        EditorRemoveDest index ->
            ( mapEditor (\e -> { e | destinations = removeAt index e.destinations }) model, Cmd.none )

        EditorReaderValue v ->
            ( mapEditor (\e -> { e | readerValue = v, readerError = "" }) model, Cmd.none )

        EditorAddReader ->
            case model.editor of
                Just editor ->
                    let
                        implicit =
                            Api.emailRecipients (editorAction editor)

                        value =
                            String.toLower (String.trim editor.readerValue)
                    in
                    if not (validEmail value) then
                        ( mapEditor (\e -> { e | readerError = "Enter an email address like you@example.com" }) model, Cmd.none )

                    else if List.member value implicit || List.member value editor.readers then
                        ( mapEditor (\e -> { e | readerError = "Already an inbox reader." }) model, Cmd.none )

                    else
                        ( mapEditor
                            (\e ->
                                { e
                                    | readers = e.readers ++ [ value ]
                                    , readerValue = ""
                                    , readerError = ""
                                }
                            )
                            model
                        , Cmd.none
                        )

                Nothing ->
                    ( model, Cmd.none )

        EditorRemoveReader index ->
            ( mapEditor (\e -> { e | readers = removeAt index e.readers }) model, Cmd.none )

        EditorSubmit ->
            case ( model.editor, model.data ) of
                ( Just editor, Success snap ) ->
                    if editor.actionType == "forward" && List.isEmpty editor.destinations then
                        ( { model | notice = Just ( Ui.Error, "A forward rule needs at least one destination." ) }, Cmd.none )

                    else
                        let
                            action =
                                editorAction editor

                            newRule =
                                { label = editor.label
                                , localPattern = editor.local
                                , domainPattern = editor.domain
                                , action = action
                                , inboxReaders = editor.readers
                                , rulesVersion = snap.rulesVersion
                                }
                        in
                        ( { model | editor = Just { editor | submitting = True }, notice = Nothing }
                        , case editor.id of
                            Just id ->
                                Api.updateRule id newRule GotSaved

                            Nothing ->
                                Api.createRule newRule GotSaved
                        )

                _ ->
                    ( model, Cmd.none )

        GotSaved result ->
            case result of
                Success snap ->
                    ( { model
                        | data = Success snap
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
                        ( { model
                            | busy = False
                            , editor = Maybe.map (\e -> { e | submitting = False }) model.editor
                            , notice = Just ( Ui.Error, Api.errorMessage err )
                          }
                        , Cmd.none
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

        ToggleMessage id ->
            if model.expanded == Just id then
                ( { model | expanded = Nothing, viewing = NotAsked }, Cmd.none )

            else
                ( { model | expanded = Just id, viewing = Loading }
                , Api.getMessage id GotMessage
                )

        GotMessage result ->
            ( { model | viewing = result }, Cmd.none )

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
                    ( mapEditor (\e -> { e | chats = chats }) model, Cmd.none )

                _ ->
                    ( model, Cmd.none )

        DismissNotice ->
            ( { model | notice = Nothing }, Cmd.none )



-- UPDATE HELPERS


mapEditor : (Editor -> Editor) -> Model -> Model
mapEditor f model =
    { model | editor = Maybe.map f model.editor }


removeAt : Int -> List a -> List a
removeAt index items =
    List.indexedMap Tuple.pair items
        |> List.filter (\( i, _ ) -> i /= index)
        |> List.map Tuple.second


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


editorAction : Editor -> Api.Action
editorAction editor =
    case editor.actionType of
        "store" ->
            Api.Store { persist = editor.persist }

        "drop" ->
            Api.Drop

        _ ->
            if editor.storeForward then
                Api.ForwardStore editor.destinations

            else
                Api.Forward editor.destinations


parseDestination : Api.Enabled -> Editor -> Result String Api.Destination
parseDestination enabled editor =
    let
        raw =
            String.trim editor.destValue
    in
    case editor.destKind of
        "email" ->
            let
                v =
                    String.toLower raw
            in
            if validEmail v then
                Ok (Api.Email { address = v, proxy = editor.destProxy })

            else
                Err "Enter an email address like you@example.com"

        "telegram" ->
            if not enabled.telegram then
                Err "Telegram isn't configured on this deployment"

            else if isInteger raw then
                Ok (Api.Telegram { chatId = raw, linkAuth = editor.destAuth })

            else
                Err "A Telegram chat id is an integer, like -1001234567890"

        "discord" ->
            if not enabled.discord then
                Err "Discord isn't configured on this deployment"

            else if isInteger raw then
                Ok (Api.Discord { channelId = raw, linkAuth = editor.destAuth })

            else
                Err "A Discord channel id is a positive integer"

        _ ->
            Err "Unknown destination kind"


isInteger : String -> Bool
isInteger s =
    let
        body =
            if String.startsWith "-" s then
                String.dropLeft 1 s

            else
                s
    in
    not (String.isEmpty body) && String.all Char.isDigit body


validEmail : String -> Bool
validEmail addr =
    case String.split "@" addr of
        [ local, domain ] ->
            not (String.isEmpty local)
                && String.all isLocalChar local
                && not (String.isEmpty domain)
                && String.contains "." domain
                && not (String.startsWith "-" domain)
                && not (String.endsWith "-" domain)
                && not (String.startsWith "." domain)
                && not (String.endsWith "." domain)
                && not (String.contains ".." domain)
                && String.all isDomainChar domain
                && (case List.reverse (String.split "." domain) of
                        tld :: _ ->
                            String.length tld >= 2 && String.all Char.isAlpha tld

                        [] ->
                            False
                   )

        _ ->
            False


isLocalChar : Char -> Bool
isLocalChar c =
    Char.isAlphaNum c || String.contains (String.fromChar c) "!#$%&'*+-/=?^_`{|}~."


isDomainChar : Char -> Bool
isDomainChar c =
    Char.isAlphaNum c || c == '-' || c == '.'


editorFrom : Api.Rule -> Editor
editorFrom rule =
    let
        fields =
            case rule.action of
                Api.Forward dests ->
                    { actionType = "forward", destinations = dests, persist = False, storeForward = False }

                Api.ForwardStore dests ->
                    { actionType = "forward", destinations = dests, persist = False, storeForward = True }

                Api.Store store ->
                    { actionType = "store", destinations = [], persist = store.persist, storeForward = False }

                Api.Drop ->
                    { actionType = "drop", destinations = [], persist = False, storeForward = False }
    in
    { emptyEditor
        | id = Just rule.id
        , label = rule.label
        , local = rule.localPattern
        , domain = rule.domainPattern
        , actionType = fields.actionType
        , destinations = fields.destinations
        , persist = fields.persist
        , storeForward = fields.storeForward
        , readers = rule.inboxReaders
    }


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
        [ microbar model
        , div
            [ classList
                [ ( "workbench", True )
                , ( if model.selected == Nothing then
                        "no-selection"

                    else
                        "has-selection"
                  , True
                  )
                ]
            ]
            [ pipelinePane model
            , inspectorPane model
            ]
        , liveFeed model
        , deleteModal model
        , editorModal model
        ]


snapshotOf : Model -> Maybe Api.Snapshot
snapshotOf model =
    RemoteData.toMaybe model.data


microbar : Model -> Html Msg
microbar model =
    let
        stats =
            snapshotOf model |> Maybe.andThen .stats
    in
    div [ class "microbar" ]
        [ div [ class "microstats" ]
            (case stats of
                Just s ->
                    [ microstat "fwd" (Format.count s.forwardedTotal) "forwarded · 7d"
                    , microstat "str" (Format.count s.storedTotal) "stored · 7d"
                    , microstat "drp" (Format.count s.droppedTotal) "dropped · 7d"
                    ]

                Nothing ->
                    [ microstat "muted" "—" "stats unavailable" ]
            )
        , case stats of
            Just s ->
                ticker s.topSenders

            Nothing ->
                text ""
        , div [ class "microbar-right" ]
            [ span [ class "microbar-zone" ]
                [ text
                    (if model.zone == 0 then
                        "UTC"

                     else
                        "local"
                    )
                ]
            ]
        ]


microstat : String -> String -> String -> Html Msg
microstat tone value_ label =
    div [ class "microstat" ]
        [ span [ classList [ ( "ms-value", True ), ( tone, True ) ] ] [ text value_ ]
        , span [ class "ms-key" ] [ text label ]
        ]


ticker : List Api.TopSender -> Html Msg
ticker senders =
    if List.isEmpty senders then
        div [ class "ticker ticker-empty" ] [ text "no senders yet · 7d" ]

    else
        let
            item s =
                span [ class "ticker-item" ]
                    [ span [ class "ticker-n" ] [ text (Format.count s.n) ]
                    , span [ class "ticker-addr" ] [ text s.address ]
                    ]
        in
        div [ class "ticker", attribute "title" "Top senders · 7d (forwarded)" ]
            [ span [ class "ticker-label" ] [ text "top senders · 7d" ]
            , div [ class "ticker-window" ]
                [ div [ class "ticker-track" ] (List.map item senders ++ List.map item senders) ]
            ]


pipelinePane : Model -> Html Msg
pipelinePane model =
    let
        rules =
            snapshotOf model |> Maybe.map .rules |> Maybe.withDefault []
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
            ([ div [ class "pipeline-node" ] [ span [ class "node-dot" ] [], text "INBOUND · email routing" ] ]
                ++ List.concatMap
                    (\card ->
                        [ div [ class "pipeline-connector" ] []
                        , card
                        ]
                    )
                    (List.indexedMap (pipelineCard model) rules)
                ++ [ div [ class "pipeline-connector" ] []
                   , div [ class "pipeline-node" ] [ span [ class "node-dot node-dot-end" ] [], text "END · all rules evaluated" ]
                   ]
            )
        ]


ruleWord : Int -> String
ruleWord n =
    if n == 1 then
        " rule"

    else
        " rules"


pipelineCard : Model -> Int -> Api.Rule -> Html Msg
pipelineCard model index rule =
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
                    , disabled = model.busy
                    , small = True
                    }
                , Ui.iconButton
                    { icon = Icons.arrowDown
                    , label = "Move down"
                    , onClick = Reorder rule.id "down"
                    , variant = Ghost
                    , disabled = model.busy
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
    in
    div [ class "tester-card" ]
        [ div [ class "tester-head" ]
            [ span [ class "tester-title" ] [ Icons.send, text "Tester" ] ]
        , div [ class "tester-input-wrap" ]
            [ span [ class "tester-prefix" ] [ Icons.search ]
            , input
                [ class "tester-input"
                , type_ "text"
                , placeholder "shop@yourdomain.example"
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
                [ case selectedRule_ of
                    Just _ ->
                        div [ class "tester-row" ]
                            [ span [ class "tester-label" ] [ text "This rule" ]
                            , if thisMatches then
                                Ui.chip { label = "matches", variant = "ok", dot = True }

                              else
                                Ui.chip { label = "no match", variant = "muted", dot = False }
                            ]

                    Nothing ->
                        text ""
                , div [ class "tester-row" ]
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
                            if String.contains "@" model.tester then
                                Ui.chip { label = "no rule matches · bounced", variant = "bad", dot = True }

                            else
                                Ui.chip { label = "waiting for a full address…", variant = "muted", dot = False }
                    ]
                , if thisMatches && firing /= selectedRule_ then
                    div [ class "tester-note" ] [ text "This rule matches, but a higher rule catches first." ]

                  else
                    text ""
                ]
        ]


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
    case Maybe.andThen (\id -> ruleAt id model) model.selected of
        Nothing ->
            section [ class "inspector-pane" ]
                [ div [ class "inspector-empty" ]
                    [ span [ class "inspector-empty-arrow" ] [ Icons.chevronRight ]
                    , p []
                        [ text "Pick a rule from the pipeline to see its destinations, recent traffic and the mail it has stored." ]
                    ]
                ]

        Just rule ->
            section [ class "inspector-pane" ]
                [ inspectorHead model rule
                , div [ class "inspector-body" ]
                    [ statStrip model rule
                    , issuesCard rule
                    , destinationsCard rule
                    , actionCard rule
                    , messagesCard model rule
                    ]
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
                    div [ class "dest-grid" ] (List.map destinationCard destinations)
                ]

        _ ->
            text ""


destinationCard : Api.Destination -> Html Msg
destinationCard destination =
    let
        ( kind, value_, modifier ) =
            case destination of
                Api.Email { address, proxy } ->
                    ( "email"
                    , address
                    , if proxy then
                        Just "proxy"

                      else
                        Nothing
                    )

                Api.Telegram { chatId, linkAuth } ->
                    ( "telegram"
                    , chatId
                    , if linkAuth == "token" then
                        Just "token"

                      else
                        Nothing
                    )

                Api.Discord { channelId, linkAuth } ->
                    ( "discord"
                    , channelId
                    , if linkAuth == "token" then
                        Just "token"

                      else
                        Nothing
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
        , case modifier of
            Just m ->
                span [ class "dest-mod" ] [ text m ]

            Nothing ->
                text ""
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
messagesCard model rule =
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
            div [ class "message-list" ] (List.map (messageRow model) messages.items)
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


messageRow : Model -> Api.MessageItem -> Html Msg
messageRow model item =
    let
        isOpen =
            model.expanded == Just item.id
    in
    div [ classList [ ( "message", True ), ( "is-open", isOpen ) ] ]
        [ button
            [ class "message-row"
            , type_ "button"
            , onClick (ToggleMessage item.id)
            ]
            [ span [ class "message-ts" ]
                [ text (item.createdAt |> Maybe.map Format.dateShort |> Maybe.withDefault "—") ]
            , span [ class "message-from" ] [ text item.sender ]
            , span [ class "message-subj" ] [ text item.subject ]
            , span [ classList [ ( "message-caret", True ), ( "is-open", isOpen ) ] ] [ Icons.chevronRight ]
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



-- MODALS


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


editorModal : Model -> Html Msg
editorModal model =
    case ( model.editor, enabledChannels model ) of
        ( Just editor, Just enabled ) ->
            Ui.modal
                { title =
                    if editor.actionType == "forward" && editor.storeForward then
                        "Store & Forward"

                    else if editor.id == Nothing then
                        "New rule"

                    else
                        "Edit rule"
                , onClose = CloseEditor
                , body =
                    [ Ui.field
                        { id = "rule-label"
                        , label = "Label (optional)"
                        , value = editor.label
                        , hint = "Leave blank to name it after the pattern and action."
                        , placeholder = editorAutoLabel editor
                        , type_ = "text"
                        , required = False
                        , onInput = EditorLabel
                        }
                    , patternField editor
                    , Ui.segmented
                        { name = "rule-action"
                        , options = [ ( "forward", "Forward" ), ( "store", "Store" ), ( "drop", "Drop" ) ]
                        , selected = editor.actionType
                        , onSelect = EditorActionType
                        , disabled = []
                        }
                    , if editor.actionType == "forward" then
                        div [ class "forward-fields" ]
                            [ destinationsField model editor enabled
                            , Ui.toggle
                                { id = "rule-store-forward"
                                , label = "Store & Forward (keep a copy of the message)"
                                , checked = editor.storeForward
                                , onCheck = EditorStore
                                }
                            ]

                      else if editor.actionType == "store" then
                        Ui.toggle
                            { id = "rule-persist"
                            , label = "Persist the message body to the database"
                            , checked = editor.persist
                            , onCheck = EditorPersist
                            }

                      else
                        p [ class "hint" ] [ text "Matching mail is silently discarded. No notification, no bounce." ]
                    , if editor.actionType == "drop" then
                        text ""

                      else
                        readersField editor
                    ]
                , footer =
                    [ Ui.button { label = "Cancel", onClick = CloseEditor, variant = Ghost, busy = False, disabled = False }
                    , Ui.button
                        { label =
                            if editor.id == Nothing then
                                "Create rule"

                            else
                                "Save"
                        , onClick = EditorSubmit
                        , variant = Primary
                        , busy = editor.submitting
                        , disabled = False
                        }
                    ]
                }

        _ ->
            text ""


patternField : Editor -> Html Msg
patternField editor =
    div [ class "field" ]
        [ Html.label [] [ text "Pattern" ]
        , div [ class "pat-input" ]
            [ input
                [ A.id "rule-local"
                , type_ "text"
                , value editor.local
                , placeholder "*"
                , onInput EditorLocal
                , E.preventDefaultOn "keydown"
                    (D.field "key" D.string
                        |> D.andThen
                            (\key ->
                                if key == "@" then
                                    D.succeed ( EditorAt, True )

                                else
                                    D.fail "not the at key"
                            )
                    )
                , A.autocomplete False
                , A.spellcheck False
                ]
                []
            , span [ class "pat-at" ] [ text "@" ]
            , input
                [ A.id "rule-domain"
                , type_ "text"
                , value editor.domain
                , placeholder "*"
                , onInput EditorDomain
                , A.autocomplete False
                , A.spellcheck False
                ]
                []
            ]
        , p [ class "hint" ]
            [ text "* matches anything, ? matches one character. Local part before @, domain after." ]
        ]


{-| Inbox readers. The rule's email destinations are shown as locked chips:
every destination is automatically a reader. Extra addresses the operator adds
are removable. The server unions the two into the rule's effective readers.
-}
readersField : Editor -> Html Msg
readersField editor =
    let
        implicit =
            Api.emailRecipients (editorAction editor)
    in
    div [ class "readers-field" ]
        [ Html.label [] [ text "Inbox readers" ]
        , div [ class "readers-chips" ]
            (List.map implicitReaderChip implicit
                ++ List.indexedMap explicitReaderChip editor.readers
            )
        , div [ class "dest-add-row" ]
            [ input
                [ class "dest-input"
                , type_ "text"
                , value editor.readerValue
                , placeholder "teammate@example.com"
                , onInput EditorReaderValue
                , A.autocomplete False
                , A.spellcheck False
                ]
                []
            , Ui.button { label = "Add", onClick = EditorAddReader, variant = Secondary, busy = False, disabled = False }
            ]
        , if String.isEmpty editor.readerError then
            text ""

          else
            p [ class "dest-error" ] [ text editor.readerError ]
        , p [ class "hint" ]
            [ text "Email destinations always read this rule's stored mail. Add anyone else who should see it in their inbox." ]
        ]


implicitReaderChip : String -> Html Msg
implicitReaderChip address =
    span [ class "reader-chip is-locked" ]
        [ span [ class "reader-chip-kind" ] [ text "recipient" ]
        , span [ class "reader-chip-value" ] [ text address ]
        , span
            [ class "reader-chip-lock"
            , A.title "Every email destination is always a reader"
            ]
            [ Icons.lock ]
        ]


explicitReaderChip : Int -> String -> Html Msg
explicitReaderChip index address =
    span [ class "reader-chip" ]
        [ span [ class "reader-chip-value" ] [ text address ]
        , button
            [ class "reader-chip-remove"
            , type_ "button"
            , attribute "aria-label" ("Remove " ++ address)
            , A.title "Remove"
            , onClick (EditorRemoveReader index)
            ]
            [ Icons.close ]
        ]


destinationsField : Model -> Editor -> Api.Enabled -> Html Msg
destinationsField _ editor enabled =
    let
        kindOptions =
            List.filterMap identity
                [ Just ( "email", "Email" )
                , if enabled.telegram then
                    Just ( "telegram", "Telegram" )

                  else
                    Nothing
                , if enabled.discord then
                    Just ( "discord", "Discord" )

                  else
                    Nothing
                ]
    in
    div [ class "dest-builder" ]
        [ if List.isEmpty editor.destinations then
            text ""

          else
            div [ class "dest-chips" ] (List.indexedMap destChip editor.destinations)
        , div [ class "dest-add" ]
            [ Ui.segmented
                { name = "dest-kind"
                , options = kindOptions
                , selected = editor.destKind
                , onSelect = EditorDestKind
                , disabled = []
                }
            , div [ class "dest-add-row" ]
                [ input
                    [ class "dest-input"
                    , type_ "text"
                    , value editor.destValue
                    , placeholder (destPlaceholder editor.destKind)
                    , onInput EditorDestValue
                    , A.autocomplete False
                    , A.spellcheck False
                    ]
                    []
                , Ui.button { label = "Add", onClick = EditorAddDest, variant = Secondary, busy = False, disabled = False }
                ]
            , modifierToggles editor
            , if String.isEmpty editor.destError then
                text ""

              else
                p [ class "dest-error" ] [ text editor.destError ]
            , if editor.destKind == "telegram" && not (List.isEmpty editor.chats) then
                div [ class "dest-suggest" ] (List.map chatSuggestion editor.chats)

              else
                text ""
            ]
        ]


modifierToggles : Editor -> Html Msg
modifierToggles editor =
    case editor.destKind of
        "email" ->
            Ui.toggle
                { id = "dest-proxy"
                , label = "Proxy mode (rewrite Reply-To through the worker)"
                , checked = editor.destProxy
                , onCheck = EditorDestProxy
                }

        "telegram" ->
            authToggle editor

        "discord" ->
            authToggle editor

        _ ->
            text ""


authToggle : Editor -> Html Msg
authToggle editor =
    div [ class "dest-auth" ]
        [ span [ class "dest-auth-label" ] [ text "Viewer link" ]
        , Ui.segmented
            { name = "dest-auth"
            , options = [ ( "access", "Access" ), ( "token", "Token" ) ]
            , selected = editor.destAuth
            , onSelect = EditorDestAuth
            , disabled = []
            }
        ]


chatSuggestion : Api.TelegramChat -> Html Msg
chatSuggestion chat =
    button
        [ class "dest-suggest-item"
        , type_ "button"
        , onClick (EditorDestValue chat.chatId)
        ]
        [ span [] [ text chat.label ]
        , span [ class "dest-suggest-id" ] [ text chat.chatId ]
        ]


destChip : Int -> Api.Destination -> Html Msg
destChip index destination =
    let
        ( kind, labelText, modText ) =
            case destination of
                Api.Email { address, proxy } ->
                    ( "email"
                    , address
                    , if proxy then
                        [ "proxy" ]

                      else
                        []
                    )

                Api.Telegram { chatId, linkAuth } ->
                    ( "telegram"
                    , chatId
                    , if linkAuth == "token" then
                        [ "token" ]

                      else
                        []
                    )

                Api.Discord { channelId, linkAuth } ->
                    ( "discord"
                    , channelId
                    , if linkAuth == "token" then
                        [ "token" ]

                      else
                        []
                    )
    in
    span [ classList [ ( "dest-chip", True ), ( kind, True ) ] ]
        [ span [ class "dest-chip-kind" ] [ text kind ]
        , span [ class "dest-chip-value" ] [ text labelText ]
        , span [ class "dest-chip-mods" ] (List.map (\m -> span [ class "dest-chip-mod" ] [ text m ]) modText)
        , button
            [ class "dest-chip-remove"
            , type_ "button"
            , attribute "aria-label" ("Remove " ++ labelText)
            , A.title "Remove"
            , onClick (EditorRemoveDest index)
            ]
            [ Icons.close ]
        ]


destPlaceholder : String -> String
destPlaceholder kind =
    case kind of
        "telegram" ->
            "-1001234567890"

        "discord" ->
            "987654321098765432"

        _ ->
            "you@example.com"


editorAutoLabel : Editor -> String
editorAutoLabel editor =
    let
        isCatch =
            editor.local == "*" && editor.domain == "*"

        prefix =
            if isCatch then
                "Catch-all"

            else
                editor.local ++ "@" ++ editor.domain

        suffix =
            case editor.actionType of
                "drop" ->
                    "drop"

                "store" ->
                    "store"

                _ ->
                    if editor.storeForward then
                        "store & forward"

                    else if List.isEmpty editor.destinations then
                        "forward (no destinations)"

                    else
                        "forward"
    in
    prefix ++ " → " ++ suffix
