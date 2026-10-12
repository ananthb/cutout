module Api exposing
    ( Action(..)
    , ActionKind(..)
    , ApiError(..)
    , Body(..)
    , Data
    , Destination(..)
    , Enabled
    , Event
    , EventsPayload
    , Issue
    , MessageItem
    , MessagesPayload
    , PendingCounts
    , PendingRow
    , RenderedMessage
    , Rule
    , Snapshot
    , TelegramChat
    , TopSender
    , actionKind
    , actionSummary
    , createRule
    , deleteRule
    , destinationsOf
    , discardAllPending
    , discardPending
    , errorMessage
    , getBootstrap
    , getEvents
    , getMessage
    , getPending
    , getPendingCount
    , getRuleMessages
    , getTelegramChats
    , isForwardStore
    , isUnauthenticated
    , reorderRule
    , retryAllPending
    , retryPending
    , updateRule
    )

{-| The single place that knows the shape of `/api/*`.

Two conventions the whole module rests on:

**Cloudflare Access is the credential.** The SPA and the worker are on the
same origin, and the Access session cookie travels on every request
automatically. There is no token to store or attach. A 401 from anywhere means
the session is gone; the app says so rather than showing a raw error.

**Every mutation carries `X-Cutout-Request`.** The cookie is `SameSite=Lax`,
which already stops a cross-site form post, and a custom header can't be set
cross-origin without a preflight the worker never grants.

Errors decode the worker's `{"error":{"code","message"}}` envelope, so a 400
reaches the user as "Rule 2: …" instead of `BadStatus 400`.

-}

import Http
import Json.Decode as D exposing (Decoder)
import Json.Encode as E
import RemoteData exposing (RemoteData)



-- TYPES


type Action
    = Forward (List Destination)
    | ForwardStore (List Destination)
    | Store { persist : Bool }
    | Drop


type Destination
    = Email { address : String, proxy : Bool }
    | Telegram { chatId : String, linkAuth : String }
    | Discord { channelId : String, linkAuth : String }


type alias Enabled =
    { telegram : Bool
    , discord : Bool
    }


type alias Issue =
    { level : String
    , message : String
    }


type alias Rule =
    { id : String
    , label : String
    , displayLabel : String
    , localPattern : String
    , domainPattern : String
    , isCatchAll : Bool
    , action : Action
    , issues : List Issue
    , matches7d : Maybe Int
    , lastMatchS : Maybe Int
    }


type alias TopSender =
    { address : String
    , n : Int
    }


type alias Stats =
    { forwardedTotal : Int
    , droppedTotal : Int
    , storedTotal : Int
    , topSenders : List TopSender
    , generatedAt : Int
    }


type alias PendingCounts =
    { queued : Int
    , deadLettered : Int
    }


type alias Snapshot =
    { operator : String
    , enabled : Enabled
    , rulesVersion : Int
    , rules : List Rule
    , stats : Maybe Stats
    , pending : PendingCounts
    }


type alias Event =
    { ts : Int
    , kind : String
    , from : String
    , to : String
    , ruleId : Maybe String
    , channels : List String
    , sizeBytes : Int
    , error : Maybe String
    }


type alias EventsPayload =
    { events : List Event
    , now : Int
    }


type alias PendingRow =
    { id : String
    , sender : String
    , recipient : String
    , ruleId : Maybe String
    , attempts : Int
    , lastError : Maybe String
    , deadLettered : Bool
    }


type alias MessageItem =
    { id : String
    , sender : String
    , subject : String
    , createdAt : Maybe String
    }


type alias MessagesPayload =
    { messages : List MessageItem
    , hasMore : Bool
    , nextBefore : Maybe String
    }


type alias RenderedMessage =
    { subject : String
    , from : String
    , to : String
    , date : Maybe String
    , body : Body
    }


type Body
    = HtmlBody String
    | TextBody String
    | NoBody


type alias TelegramChat =
    { chatId : String
    , label : String
    }



-- ERRORS


type ApiError
    = Fault { code : String, message : String }
    | Unauthenticated
    | Transport Http.Error


type alias Data a =
    RemoteData ApiError a


errorMessage : ApiError -> String
errorMessage err =
    case err of
        Fault { message } ->
            message

        Unauthenticated ->
            "Your Cloudflare Access session expired. Reload to sign in again."

        Transport (Http.BadStatus code) ->
            "The server returned an unexpected error (" ++ String.fromInt code ++ ")."

        Transport Http.NetworkError ->
            "Couldn't reach the server. Check your connection and try again."

        Transport Http.Timeout ->
            "That took too long. Try again."

        Transport (Http.BadBody detail) ->
            "The server sent something we couldn't read. " ++ detail

        Transport (Http.BadUrl url) ->
            "Tried to call an invalid URL: " ++ url


isUnauthenticated : ApiError -> Bool
isUnauthenticated err =
    err == Unauthenticated



-- REQUESTS


expectJson : (Data a -> msg) -> Decoder a -> Http.Expect msg
expectJson toMsg decoder =
    Http.expectStringResponse (RemoteData.fromResult >> toMsg) <|
        \response ->
            case response of
                Http.BadUrl_ url ->
                    Err (Transport (Http.BadUrl url))

                Http.Timeout_ ->
                    Err (Transport Http.Timeout)

                Http.NetworkError_ ->
                    Err (Transport Http.NetworkError)

                Http.BadStatus_ metadata body ->
                    if metadata.statusCode == 401 then
                        Err Unauthenticated

                    else
                        case D.decodeString faultDecoder body of
                            Ok fault ->
                                Err (Fault fault)

                            Err _ ->
                                Err (Transport (Http.BadStatus metadata.statusCode))

                Http.GoodStatus_ _ body ->
                    let
                        payload =
                            if String.trim body == "" then
                                "{}"

                            else
                                body
                    in
                    case D.decodeString decoder payload of
                        Ok value ->
                            Ok value

                        Err err ->
                            Err (Transport (Http.BadBody (D.errorToString err)))


faultDecoder : Decoder { code : String, message : String }
faultDecoder =
    D.field "error"
        (D.map2 (\c m -> { code = c, message = m })
            (D.field "code" D.string)
            (D.field "message" D.string)
        )


get : String -> Decoder a -> (Data a -> msg) -> Cmd msg
get path decoder toMsg =
    Http.request
        { method = "GET"
        , headers = []
        , url = path
        , body = Http.emptyBody
        , expect = expectJson toMsg decoder
        , timeout = Nothing
        , tracker = Nothing
        }


mutate : String -> String -> E.Value -> Decoder a -> (Data a -> msg) -> Cmd msg
mutate method path body decoder toMsg =
    Http.request
        { method = method
        , headers = [ Http.header "X-Cutout-Request" "1" ]
        , url = path
        , body = Http.jsonBody body
        , expect = expectJson toMsg decoder
        , timeout = Nothing
        , tracker = Nothing
        }


post : String -> E.Value -> Decoder a -> (Data a -> msg) -> Cmd msg
post path body decoder toMsg =
    mutate "POST" path body decoder toMsg


put : String -> E.Value -> Decoder a -> (Data a -> msg) -> Cmd msg
put path body decoder toMsg =
    mutate "PUT" path body decoder toMsg


delete : String -> E.Value -> Decoder a -> (Data a -> msg) -> Cmd msg
delete path body decoder toMsg =
    mutate "DELETE" path body decoder toMsg



-- ENDPOINTS


getBootstrap : (Data Snapshot -> msg) -> Cmd msg
getBootstrap =
    get "/api/bootstrap" snapshotDecoder


type alias NewRule =
    { label : String
    , localPattern : String
    , domainPattern : String
    , action : Action
    , rulesVersion : Int
    }


createRule : NewRule -> (Data Snapshot -> msg) -> Cmd msg
createRule r =
    post "/api/rules" (ruleBody r) snapshotDecoder


updateRule : String -> NewRule -> (Data Snapshot -> msg) -> Cmd msg
updateRule id r =
    put ("/api/rules/" ++ id) (ruleBody r) snapshotDecoder


ruleBody : NewRule -> E.Value
ruleBody r =
    E.object
        [ ( "label", E.string r.label )
        , ( "local_pattern", E.string r.localPattern )
        , ( "domain_pattern", E.string r.domainPattern )
        , ( "action", encodeAction r.action )
        , ( "rules_version", E.int r.rulesVersion )
        ]


deleteRule : String -> Int -> (Data Snapshot -> msg) -> Cmd msg
deleteRule id version =
    delete ("/api/rules/" ++ id) (E.object [ ( "rules_version", E.int version ) ]) snapshotDecoder


reorderRule : { id : String, direction : String, rulesVersion : Int } -> (Data Snapshot -> msg) -> Cmd msg
reorderRule args =
    post "/api/rules/reorder"
        (E.object
            [ ( "id", E.string args.id )
            , ( "direction", E.string args.direction )
            , ( "rules_version", E.int args.rulesVersion )
            ]
        )
        snapshotDecoder


getEvents : Maybe Int -> (Data EventsPayload -> msg) -> Cmd msg
getEvents since =
    let
        path =
            case since of
                Just ms ->
                    "/api/events?since=" ++ String.fromInt ms

                Nothing ->
                    "/api/events"
    in
    get path eventsDecoder


getPendingCount : (Data PendingCounts -> msg) -> Cmd msg
getPendingCount =
    get "/api/pending/count" pendingCountsDecoder


getPending : (Data (List PendingRow) -> msg) -> Cmd msg
getPending =
    get "/api/pending" (D.field "rows" (D.list pendingRowDecoder))


retryPending : String -> (Data Bool -> msg) -> Cmd msg
retryPending id =
    post ("/api/pending/" ++ id ++ "/retry") (E.object []) (D.field "ok" D.bool)


discardPending : String -> (Data Bool -> msg) -> Cmd msg
discardPending id =
    post ("/api/pending/" ++ id ++ "/discard") (E.object []) (D.field "ok" D.bool)


retryAllPending : (Data Int -> msg) -> Cmd msg
retryAllPending =
    post "/api/pending/retry-all" (E.object []) (D.field "count" D.int)


discardAllPending : (Data Int -> msg) -> Cmd msg
discardAllPending =
    post "/api/pending/discard-all" (E.object []) (D.field "count" D.int)


getRuleMessages : { ruleId : String, before : Maybe String } -> (Data MessagesPayload -> msg) -> Cmd msg
getRuleMessages args =
    let
        base =
            "/api/rules/" ++ args.ruleId ++ "/messages"

        path =
            case args.before of
                Just before ->
                    base ++ "?before=" ++ urlEncode before

                Nothing ->
                    base
    in
    get path messagesDecoder


getMessage : String -> (Data RenderedMessage -> msg) -> Cmd msg
getMessage id =
    get ("/api/messages/" ++ id) renderedDecoder


getTelegramChats : (Data (List TelegramChat) -> msg) -> Cmd msg
getTelegramChats =
    get "/api/telegram-chats" (D.field "chats" (D.list telegramChatDecoder))


urlEncode : String -> String
urlEncode s =
    s
        |> String.toList
        |> List.map encodeChar
        |> String.concat


encodeChar : Char -> String
encodeChar c =
    if Char.isAlphaNum c || List.member c [ '-', '_', '.', '~' ] then
        String.fromChar c

    else
        let
            code =
                Char.toCode c
        in
        "%" ++ hexDigit (code // 16) ++ hexDigit (modBy 16 code)


hexDigit : Int -> String
hexDigit n =
    if n < 10 then
        String.fromInt n

    else
        String.fromChar (Char.fromCode (87 + n))



-- DECODERS


snapshotDecoder : Decoder Snapshot
snapshotDecoder =
    D.map6 Snapshot
        (D.field "operator" D.string)
        (D.field "enabled" enabledDecoder)
        (D.field "rules_version" D.int)
        (D.field "rules" (D.list ruleDecoder))
        (D.field "stats" (D.nullable statsDecoder))
        (D.field "pending" pendingCountsDecoder)


enabledDecoder : Decoder Enabled
enabledDecoder =
    D.map2 Enabled
        (D.field "telegram" D.bool)
        (D.field "discord" D.bool)


ruleDecoder : Decoder Rule
ruleDecoder =
    D.map8
        (\id label displayLabel localPattern domainPattern isCatchAll action issues ->
            \matches7d lastMatchS ->
                { id = id
                , label = label
                , displayLabel = displayLabel
                , localPattern = localPattern
                , domainPattern = domainPattern
                , isCatchAll = isCatchAll
                , action = action
                , issues = issues
                , matches7d = matches7d
                , lastMatchS = lastMatchS
                }
        )
        (D.field "id" D.string)
        (D.field "label" D.string)
        (D.field "display_label" D.string)
        (D.field "local_pattern" D.string)
        (D.field "domain_pattern" D.string)
        (D.field "is_catch_all" D.bool)
        (D.field "action" actionDecoder)
        (D.field "issues" (D.list issueDecoder))
        |> D.andThen
            (\build ->
                D.map2 build
                    (D.field "matches_7d" (D.nullable D.int))
                    (D.field "last_match_s" (D.nullable D.int))
            )


issueDecoder : Decoder Issue
issueDecoder =
    D.map2 Issue
        (D.field "level" D.string)
        (D.field "message" D.string)


actionDecoder : Decoder Action
actionDecoder =
    D.field "type" D.string
        |> D.andThen
            (\t ->
                case t of
                    "forward" ->
                        D.map Forward (D.field "destinations" (D.list destinationDecoder))

                    "forward_store" ->
                        D.map ForwardStore (D.field "destinations" (D.list destinationDecoder))

                    "store" ->
                        D.map (\p -> Store { persist = p })
                            (D.field "persist" (D.oneOf [ D.bool, D.succeed False ]))

                    "drop" ->
                        D.succeed Drop

                    other ->
                        D.fail ("unknown action type: " ++ other)
            )


destinationDecoder : Decoder Destination
destinationDecoder =
    D.field "kind" D.string
        |> D.andThen
            (\k ->
                case k of
                    "email" ->
                        D.map2 (\address proxy -> Email { address = address, proxy = proxy })
                            (D.field "address" D.string)
                            (D.field "proxy" (D.oneOf [ D.bool, D.succeed False ]))

                    "telegram" ->
                        D.map2 (\chatId auth -> Telegram { chatId = chatId, linkAuth = auth })
                            (D.field "chat_id" D.string)
                            (D.field "link_auth" linkAuthDecoder)

                    "discord" ->
                        D.map2 (\channelId auth -> Discord { channelId = channelId, linkAuth = auth })
                            (D.field "channel_id" D.string)
                            (D.field "link_auth" linkAuthDecoder)

                    other ->
                        D.fail ("unknown destination kind: " ++ other)
            )


linkAuthDecoder : Decoder String
linkAuthDecoder =
    D.oneOf [ D.string, D.succeed "access" ]


statsDecoder : Decoder Stats
statsDecoder =
    D.map5 Stats
        (D.field "forwarded_total" D.int)
        (D.field "dropped_total" D.int)
        (D.field "stored_total" D.int)
        (D.field "top_senders" (D.list topSenderDecoder))
        (D.field "generated_at" D.int)


topSenderDecoder : Decoder TopSender
topSenderDecoder =
    D.map2 TopSender
        (D.field "address" D.string)
        (D.field "n" D.int)


pendingCountsDecoder : Decoder PendingCounts
pendingCountsDecoder =
    D.map2 PendingCounts
        (D.field "queued" D.int)
        (D.field "dead_lettered" D.int)


eventDecoder : Decoder Event
eventDecoder =
    D.map8 Event
        (D.field "ts" D.int)
        (D.field "kind" D.string)
        (D.field "from" D.string)
        (D.field "to" D.string)
        (D.field "rule_id" (D.nullable D.string))
        (D.field "channels" (D.list D.string))
        (D.field "size_bytes" D.int)
        (D.field "error" (D.nullable D.string))


eventsDecoder : Decoder EventsPayload
eventsDecoder =
    D.map2 EventsPayload
        (D.field "events" (D.list eventDecoder))
        (D.field "now" D.int)


pendingRowDecoder : Decoder PendingRow
pendingRowDecoder =
    D.map7 PendingRow
        (D.field "id" D.string)
        (D.field "sender" D.string)
        (D.field "recipient" D.string)
        (D.field "rule_id" (D.nullable D.string))
        (D.field "attempts" D.int)
        (D.field "last_error" (D.nullable D.string))
        (D.field "dead_lettered" D.bool)


messageItemDecoder : Decoder MessageItem
messageItemDecoder =
    D.map4 MessageItem
        (D.field "id" D.string)
        (D.field "sender" D.string)
        (D.field "subject" D.string)
        (D.field "created_at" (D.nullable D.string))


messagesDecoder : Decoder MessagesPayload
messagesDecoder =
    D.map3 MessagesPayload
        (D.field "messages" (D.list messageItemDecoder))
        (D.field "has_more" D.bool)
        (D.field "next_before" (D.nullable D.string))


bodyDecoder : Decoder Body
bodyDecoder =
    D.field "kind" D.string
        |> D.andThen
            (\k ->
                case k of
                    "html" ->
                        D.map HtmlBody (D.field "html" D.string)

                    "text" ->
                        D.map TextBody (D.field "text" D.string)

                    _ ->
                        D.succeed NoBody
            )


renderedDecoder : Decoder RenderedMessage
renderedDecoder =
    D.map5 RenderedMessage
        (D.field "subject" D.string)
        (D.field "from" D.string)
        (D.field "to" D.string)
        (D.field "date" (D.nullable D.string))
        (D.field "body" bodyDecoder)


telegramChatDecoder : Decoder TelegramChat
telegramChatDecoder =
    D.map2 TelegramChat
        (D.field "chat_id" D.string)
        (D.field "label" D.string)



-- ENCODERS


encodeAction : Action -> E.Value
encodeAction action =
    case action of
        Forward destinations ->
            E.object
                [ ( "type", E.string "forward" )
                , ( "destinations", E.list encodeDestination destinations )
                ]

        ForwardStore destinations ->
            E.object
                [ ( "type", E.string "forward_store" )
                , ( "destinations", E.list encodeDestination destinations )
                ]

        Store { persist } ->
            E.object
                [ ( "type", E.string "store" )
                , ( "persist", E.bool persist )
                ]

        Drop ->
            E.object [ ( "type", E.string "drop" ) ]


encodeDestination : Destination -> E.Value
encodeDestination destination =
    case destination of
        Email { address, proxy } ->
            E.object
                [ ( "kind", E.string "email" )
                , ( "address", E.string address )
                , ( "proxy", E.bool proxy )
                ]

        Telegram { chatId, linkAuth } ->
            E.object
                [ ( "kind", E.string "telegram" )
                , ( "chat_id", E.string chatId )
                , ( "link_auth", E.string linkAuth )
                ]

        Discord { channelId, linkAuth } ->
            E.object
                [ ( "kind", E.string "discord" )
                , ( "channel_id", E.string channelId )
                , ( "link_auth", E.string linkAuth )
                ]



-- PRESENTATION HELPERS


{-| One-line description of what a rule does, for the pipeline card.
-}
actionSummary : Action -> String
actionSummary action =
    case action of
        Forward destinations ->
            "forward · " ++ channels destinations

        ForwardStore destinations ->
            "store & forward · " ++ channels destinations

        Store _ ->
            "store"

        Drop ->
            "drop"


{-| Coarse action class. `Forward` and `ForwardStore` are the same class; use
[`isForwardStore`] to tell them apart. Elm has no or-patterns, so this is how
shared handling stays in one place.
-}
type ActionKind
    = ForwardKind
    | StoreKind
    | DropKind


actionKind : Action -> ActionKind
actionKind action =
    case action of
        Forward _ ->
            ForwardKind

        ForwardStore _ ->
            ForwardKind

        Store _ ->
            StoreKind

        Drop ->
            DropKind


destinationsOf : Action -> List Destination
destinationsOf action =
    case action of
        Forward destinations ->
            destinations

        ForwardStore destinations ->
            destinations

        _ ->
            []


isForwardStore : Action -> Bool
isForwardStore action =
    case action of
        ForwardStore _ ->
            True

        _ ->
            False


channels : List Destination -> String
channels destinations =
    if List.isEmpty destinations then
        "no destinations"

    else
        let
            ( emails, telegrams, discords ) =
                counts destinations
        in
        String.join " + "
            (List.filterMap identity
                [ plural emails "email"
                , plural telegrams "telegram"
                , plural discords "discord"
                ]
            )


counts : List Destination -> ( Int, Int, Int )
counts destinations =
    List.foldl
        (\d ( e, t, dc ) ->
            case d of
                Email _ ->
                    ( e + 1, t, dc )

                Telegram _ ->
                    ( e, t + 1, dc )

                Discord _ ->
                    ( e, t, dc + 1 )
        )
        ( 0, 0, 0 )
        destinations


plural : Int -> String -> Maybe String
plural n name =
    if n == 0 then
        Nothing

    else if n == 1 then
        Just name

    else
        Just (name ++ " ×" ++ String.fromInt n)
