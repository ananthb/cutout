module Page.Manage.Editor exposing
    ( Editor
    , Msg(..)
    , OutMsg(..)
    , Submission
    , empty
    , fromRule
    , setChats
    , setError
    , setSubmitting
    , update
    , view
    )

{-| Inline rule editor rendered inside the workbench inspector pane.

Supports fluid keyboard entry (`@` jumps from local to domain pattern, `Enter`
or `,` commits a destination or inbox reader, clicking a Telegram chat
suggestion adds it in one click) and auto-commits any pending destination or
reader input when the operator submits the form directly.

-}

import Api
import Browser.Dom as Dom
import Html exposing (Html, button, div, h1, header, input, label, p, section, span, text)
import Html.Attributes as A exposing (attribute, class, classList, id, placeholder, title, type_, value)
import Html.Events as E exposing (onClick, onInput)
import Icons
import Json.Decode as D
import Task
import Ui exposing (ButtonVariant(..))



-- MODEL


type alias Submission =
    { id : Maybe String
    , label : String
    , localPattern : String
    , domainPattern : String
    , action : Api.Action
    , inboxReaders : List String
    }


type OutMsg
    = NoOut
    | Cancelled
    | Submit Submission


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
    , destError : String
    , readers : List String
    , readerValue : String
    , readerError : String
    , chats : List Api.TelegramChat
    , submitting : Bool
    , formError : Maybe String
    }


empty : Editor
empty =
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
    , destProxy = True
    , destError = ""
    , readers = []
    , readerValue = ""
    , readerError = ""
    , chats = []
    , submitting = False
    , formError = Nothing
    }


fromRule : Api.Rule -> Editor
fromRule rule =
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
    { empty
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


setChats : List Api.TelegramChat -> Editor -> Editor
setChats chats editor =
    { editor | chats = chats }


setSubmitting : Bool -> Editor -> Editor
setSubmitting submitting editor =
    { editor | submitting = submitting }


setError : Maybe String -> Editor -> Editor
setError err editor =
    { editor | formError = err }



-- UPDATE


type Msg
    = SetLabel String
    | SetLocal String
    | SetDomain String
    | FocusDomain
    | FocusResult (Result Dom.Error ())
    | SetActionType String
    | SetPersist Bool
    | SetStoreForward Bool
    | SetDestKind String
    | SetDestValue String
    | SetDestProxy Bool
    | ToggleDestProxy Int
    | AddDest
    | RemoveDest Int
    | SetReaderValue String
    | AddReader
    | RemoveReader Int
    | DismissError
    | Cancel
    | SubmitForm


update : Api.Enabled -> Msg -> Editor -> ( Editor, Cmd Msg, OutMsg )
update enabled msg editor =
    case msg of
        SetLabel v ->
            ( { editor | label = v, formError = Nothing }, Cmd.none, NoOut )

        SetLocal v ->
            ( { editor | local = v, formError = Nothing }, Cmd.none, NoOut )

        SetDomain v ->
            ( { editor | domain = v, formError = Nothing }, Cmd.none, NoOut )

        FocusDomain ->
            ( editor, Dom.focus "rule-domain" |> Task.attempt FocusResult, NoOut )

        FocusResult _ ->
            ( editor, Cmd.none, NoOut )

        SetActionType v ->
            ( { editor | actionType = v, formError = Nothing }, Cmd.none, NoOut )

        SetPersist v ->
            ( { editor | persist = v, formError = Nothing }, Cmd.none, NoOut )

        SetStoreForward v ->
            ( { editor | storeForward = v, formError = Nothing }, Cmd.none, NoOut )

        SetDestKind v ->
            ( { editor | destKind = v, destValue = "", destError = "", formError = Nothing }, Cmd.none, NoOut )

        SetDestValue v ->
            if editor.destKind == "telegram" && enabled.telegram && List.any (\c -> c.chatId == v) editor.chats then
                let
                    dest =
                        Api.Telegram { chatId = v }

                    nextDests =
                        if List.member dest editor.destinations then
                            editor.destinations

                        else
                            editor.destinations ++ [ dest ]
                in
                ( { editor
                    | destinations = nextDests
                    , destValue = ""
                    , destError = ""
                    , formError = Nothing
                  }
                , Cmd.none
                , NoOut
                )

            else
                ( { editor | destValue = v, destError = "" }, Cmd.none, NoOut )

        SetDestProxy v ->
            ( { editor | destProxy = v }, Cmd.none, NoOut )

        ToggleDestProxy index ->
            ( { editor | destinations = toggleProxyAt index editor.destinations }, Cmd.none, NoOut )

        AddDest ->
            case parseDestination enabled editor of
                Ok dest ->
                    let
                        nextDests =
                            if List.member dest editor.destinations then
                                editor.destinations

                            else
                                editor.destinations ++ [ dest ]
                    in
                    ( { editor
                        | destinations = nextDests
                        , destValue = ""
                        , destError = ""
                        , formError = Nothing
                      }
                    , Cmd.none
                    , NoOut
                    )

                Err err ->
                    ( { editor | destError = err }, Cmd.none, NoOut )

        RemoveDest index ->
            ( { editor | destinations = removeAt index editor.destinations }, Cmd.none, NoOut )

        SetReaderValue v ->
            ( { editor | readerValue = v, readerError = "" }, Cmd.none, NoOut )

        AddReader ->
            let
                implicit =
                    Api.emailRecipients (editorAction editor)

                value =
                    String.toLower (String.trim editor.readerValue)
            in
            if not (validEmail value) then
                ( { editor | readerError = "Enter an email address like you@example.com" }, Cmd.none, NoOut )

            else if List.member value implicit || List.member value editor.readers then
                ( { editor | readerError = "Already an inbox reader." }, Cmd.none, NoOut )

            else
                ( { editor
                    | readers = editor.readers ++ [ value ]
                    , readerValue = ""
                    , readerError = ""
                  }
                , Cmd.none
                , NoOut
                )

        RemoveReader index ->
            ( { editor | readers = removeAt index editor.readers }, Cmd.none, NoOut )

        DismissError ->
            ( { editor | formError = Nothing }, Cmd.none, NoOut )

        Cancel ->
            ( editor, Cmd.none, Cancelled )

        SubmitForm ->
            submitForm enabled editor


submitForm : Api.Enabled -> Editor -> ( Editor, Cmd Msg, OutMsg )
submitForm enabled editor =
    let
        withPendingDest =
            if editor.actionType == "forward" && String.trim editor.destValue /= "" then
                case parseDestination enabled editor of
                    Ok dest ->
                        let
                            nextDests =
                                if List.member dest editor.destinations then
                                    editor.destinations

                                else
                                    editor.destinations ++ [ dest ]
                        in
                        Ok { editor | destinations = nextDests, destValue = "", destError = "" }

                    Err err ->
                        Err { editor | destError = err }

            else
                Ok editor

        withPendingReader ed =
            if ed.actionType /= "drop" && String.trim ed.readerValue /= "" then
                let
                    value =
                        String.toLower (String.trim ed.readerValue)

                    implicit =
                        Api.emailRecipients (editorAction ed)
                in
                if not (validEmail value) then
                    Err { ed | readerError = "Enter an email address like you@example.com" }

                else
                    let
                        nextReaders =
                            if List.member value implicit || List.member value ed.readers then
                                ed.readers

                            else
                                ed.readers ++ [ value ]
                    in
                    Ok { ed | readers = nextReaders, readerValue = "", readerError = "" }

            else
                Ok ed
    in
    case withPendingDest |> Result.andThen withPendingReader of
        Err invalidEditor ->
            ( invalidEditor, Cmd.none, NoOut )

        Ok editorWithPending ->
            let
                localTrimmed =
                    String.trim editorWithPending.local

                domainTrimmed =
                    String.trim editorWithPending.domain
            in
            if localTrimmed == "" || domainTrimmed == "" then
                ( { editorWithPending | formError = Just "Local and domain patterns cannot be empty." }
                , Cmd.none
                , NoOut
                )

            else if editorWithPending.actionType == "forward" && List.isEmpty editorWithPending.destinations then
                ( { editorWithPending | formError = Just "Add at least one destination for a forward rule." }
                , Cmd.none
                , NoOut
                )

            else
                let
                    submission =
                        { id = editorWithPending.id
                        , label = String.trim editorWithPending.label
                        , localPattern = localTrimmed
                        , domainPattern = domainTrimmed
                        , action = editorAction editorWithPending
                        , inboxReaders = editorWithPending.readers
                        }
                in
                ( { editorWithPending | submitting = True, formError = Nothing }
                , Cmd.none
                , Submit submission
                )



-- HELPERS


removeAt : Int -> List a -> List a
removeAt index items =
    List.indexedMap Tuple.pair items
        |> List.filter (\( i, _ ) -> i /= index)
        |> List.map Tuple.second


toggleProxyAt : Int -> List Api.Destination -> List Api.Destination
toggleProxyAt index destinations =
    List.indexedMap
        (\i d ->
            if i == index then
                case d of
                    Api.Email email ->
                        Api.Email { email | proxy = not email.proxy }

                    other ->
                        other

            else
                d
        )
        destinations


editorAction : Editor -> Api.Action
editorAction editor =
    case editor.actionType of
        "store" ->
            Api.Store { persist = editor.persist }

        "drop" ->
            Api.Drop

        _ ->
            if editor.storeForward && hasEmailDestination editor then
                Api.ForwardStore editor.destinations

            else
                Api.Forward editor.destinations


hasEmailDestination : Editor -> Bool
hasEmailDestination editor =
    List.any
        (\d ->
            case d of
                Api.Email _ ->
                    True

                _ ->
                    False
        )
        editor.destinations


hasChatDestination : Editor -> Bool
hasChatDestination editor =
    List.any
        (\d ->
            case d of
                Api.Telegram _ ->
                    True

                Api.Discord _ ->
                    True

                Api.Email _ ->
                    False
        )
        editor.destinations


ruleStoresMessages : Editor -> Bool
ruleStoresMessages editor =
    editor.actionType
        == "store"
        || (editor.actionType == "forward" && (editor.storeForward || hasChatDestination editor))


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
                Ok (Api.Telegram { chatId = raw })

            else
                Err "A Telegram chat id is an integer, like -1001234567890"

        "discord" ->
            if not enabled.discord then
                Err "Discord isn't configured on this deployment"

            else if isInteger raw then
                Ok (Api.Discord { channelId = raw })

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



-- VIEW


view : Api.Enabled -> Editor -> Html Msg
view enabled editor =
    let
        idBadge =
            case editor.id of
                Nothing ->
                    "new rule"

                Just id ->
                    "edit · " ++ String.left 8 id

        titleText =
            if editor.id == Nothing then
                "New routing rule"

            else
                "Edit rule"

        localPreview =
            if String.trim editor.local == "" then
                "*"

            else
                editor.local

        domainPreview =
            if String.trim editor.domain == "" then
                "*"

            else
                editor.domain
    in
    section [ class "inspector-pane editor-pane" ]
        [ header [ class "inspector-head" ]
            [ button [ class "inspector-back", type_ "button", onClick Cancel ]
                [ Icons.chevronRight, text "Back" ]
            , div [ class "ih-meta" ]
                [ span [ class "ih-id" ] [ text idBadge ]
                , h1 [ class "ih-title" ] [ text titleText ]
                , span [ class "ih-pattern" ] (patternPieces localPreview domainPreview)
                ]
            ]
        , div [ class "inspector-body editor-body" ]
            [ case editor.formError of
                Just err ->
                    Ui.banner Ui.Error err

                Nothing ->
                    text ""
            , patternField editor
            , Ui.field
                { id = "rule-label"
                , label = "Label (optional)"
                , value = editor.label
                , hint = "Leave blank to name it after the pattern and action."
                , placeholder = editorAutoLabel editor
                , type_ = "text"
                , required = False
                , onInput = SetLabel
                }
            , Ui.segmented
                { name = "rule-action"
                , options = [ ( "forward", "Forward" ), ( "store", "Store" ), ( "drop", "Drop" ) ]
                , selected = editor.actionType
                , onSelect = SetActionType
                , disabled = []
                }
            , if editor.actionType == "forward" then
                div [ class "forward-fields" ]
                    [ destinationsField editor enabled
                    , if hasEmailDestination editor then
                        Ui.toggle
                            { id = "rule-store-forward"
                            , label = "Keep a copy in Cutout inbox (Store & Forward)"
                            , checked = editor.storeForward
                            , onCheck = SetStoreForward
                            }

                      else
                        text ""
                    ]

              else if editor.actionType == "store" then
                Ui.toggle
                    { id = "rule-persist"
                    , label = "Persist the message body to the database"
                    , checked = editor.persist
                    , onCheck = SetPersist
                    }

              else
                p [ class "hint" ] [ text "Matching mail is silently discarded. No notification, no bounce." ]
            , if ruleStoresMessages editor then
                readersField editor

              else
                text ""
            ]
        , div [ class "editor-footer" ]
            [ Ui.button
                { label = "Cancel"
                , onClick = Cancel
                , variant = Ghost
                , busy = False
                , disabled = editor.submitting
                }
            , Ui.button
                { label =
                    if editor.id == Nothing then
                        "Create rule"

                    else
                        "Save changes"
                , onClick = SubmitForm
                , variant = Primary
                , busy = editor.submitting
                , disabled = False
                }
            ]
        ]


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


patternField : Editor -> Html Msg
patternField editor =
    div [ class "field" ]
        [ label [ A.for "rule-local" ] [ text "Pattern" ]
        , div [ class "pat-input" ]
            [ input
                [ id "rule-local"
                , type_ "text"
                , value editor.local
                , placeholder "*"
                , onInput SetLocal
                , E.preventDefaultOn "keydown"
                    (D.field "key" D.string
                        |> D.andThen
                            (\key ->
                                if key == "@" then
                                    D.succeed ( FocusDomain, True )

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
                [ id "rule-domain"
                , type_ "text"
                , value editor.domain
                , placeholder "*"
                , onInput SetDomain
                , A.autocomplete False
                , A.spellcheck False
                ]
                []
            ]
        , p [ class "hint" ]
            [ text "* matches anything, ? matches one character. Local part before @, domain after." ]
        ]


onAddKeyDown : Bool -> Msg -> Html.Attribute Msg
onAddKeyDown allowComma msg =
    E.preventDefaultOn "keydown"
        (D.field "key" D.string
            |> D.andThen
                (\key ->
                    if key == "Enter" || (allowComma && key == ",") then
                        D.succeed ( msg, True )

                    else
                        D.fail "not add key"
                )
        )


destinationsField : Editor -> Api.Enabled -> Html Msg
destinationsField editor enabled =
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
                , onSelect = SetDestKind
                , disabled = []
                }
            , div [ class "dest-add-row" ]
                [ input
                    [ class "dest-input"
                    , type_ "text"
                    , value editor.destValue
                    , placeholder (destPlaceholder editor.destKind)
                    , onInput SetDestValue
                    , onAddKeyDown (editor.destKind == "email") AddDest
                    , A.autocomplete False
                    , A.spellcheck False
                    ]
                    []
                , Ui.button
                    { label = "Add"
                    , onClick = AddDest
                    , variant = Secondary
                    , busy = False
                    , disabled = False
                    }
                ]
            , if editor.destKind == "email" then
                Ui.toggle
                    { id = "dest-proxy"
                    , label = "Proxy replies through the worker (rewrite Reply-To)"
                    , checked = editor.destProxy
                    , onCheck = SetDestProxy
                    }

              else
                text ""
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


chatSuggestion : Api.TelegramChat -> Html Msg
chatSuggestion chat =
    button
        [ class "dest-suggest-item"
        , type_ "button"
        , onClick (SetDestValue chat.chatId)
        ]
        [ span [] [ text chat.label ]
        , span [ class "dest-suggest-id" ] [ text chat.chatId ]
        ]


destChip : Int -> Api.Destination -> Html Msg
destChip index destination =
    let
        ( kind, labelText, badges ) =
            case destination of
                Api.Email { address, proxy } ->
                    ( "email"
                    , address
                    , [ proxyToggle index proxy ]
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
    in
    span [ classList [ ( "dest-chip", True ), ( kind, True ) ] ]
        [ span [ class "dest-chip-kind" ] [ text kind ]
        , span [ class "dest-chip-value" ] [ text labelText ]
        , span [ class "dest-chip-mods" ] badges
        , button
            [ class "dest-chip-remove"
            , type_ "button"
            , attribute "aria-label" ("Remove " ++ labelText)
            , title "Remove"
            , onClick (RemoveDest index)
            ]
            [ Icons.close ]
        ]


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
        , title
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


proxyToggle : Int -> Bool -> Html Msg
proxyToggle index on =
    button
        [ classList
            [ ( "dest-chip-mod", True )
            , ( "is-proxy", True )
            , ( "is-toggle", True )
            , ( if on then
                    "is-on"

                else
                    "is-off"
              , True
              )
            ]
        , type_ "button"
        , title
            (if on then
                "Proxying replies. Click to stop."

             else
                "Proxy replies through the worker"
            )
        , onClick (ToggleDestProxy index)
        ]
        [ if on then
            Icons.check

          else
            text ""
        , text "proxy"
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


readersField : Editor -> Html Msg
readersField editor =
    let
        implicit =
            Api.emailRecipients (editorAction editor)
    in
    div [ class "readers-field" ]
        [ label [] [ text "Inbox readers" ]
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
                , onInput SetReaderValue
                , onAddKeyDown True AddReader
                , A.autocomplete False
                , A.spellcheck False
                ]
                []
            , Ui.button
                { label = "Add"
                , onClick = AddReader
                , variant = Secondary
                , busy = False
                , disabled = False
                }
            ]
        , if String.isEmpty editor.readerError then
            text ""

          else
            p [ class "dest-error" ] [ text editor.readerError ]
        , p [ class "hint" ]
            [ text "Email destinations always read this rule. Add more." ]
        ]


implicitReaderChip : String -> Html Msg
implicitReaderChip address =
    span [ class "reader-chip is-locked" ]
        [ span [ class "reader-chip-kind" ] [ text "recipient" ]
        , span [ class "reader-chip-value" ] [ text address ]
        , span
            [ class "reader-chip-lock"
            , title "Always a reader"
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
            , title "Remove"
            , onClick (RemoveReader index)
            ]
            [ Icons.close ]
        ]


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
                    if editor.storeForward && hasEmailDestination editor then
                        "store & forward"

                    else if List.isEmpty editor.destinations then
                        "forward (no destinations)"

                    else
                        "forward"
    in
    prefix ++ " → " ++ suffix
