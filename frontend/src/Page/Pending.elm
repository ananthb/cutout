module Page.Pending exposing (Model, Msg, init, subscriptions, update, view)

{-| Pending dispatches: failed sends still in the retry queue, and rows that
exhausted their retries and were dead-lettered. Retry kicks a row back into the
queue; discard cuts it loose and deletes its stored body.
-}

import Api
import Browser.Events
import Html exposing (Html, a, button, div, h1, p, section, span, table, tbody, td, text, th, thead, tr)
import Html.Attributes exposing (class, classList, href, title, type_)
import Html.Events exposing (onClick)
import Icons
import Json.Decode as D
import RemoteData exposing (RemoteData(..))
import Time
import Ui exposing (ButtonVariant(..))


type alias Model =
    { rows : Api.Data (List Api.PendingRow)
    , notice : Maybe ( Ui.Banner, String )
    , busyId : Maybe String
    , allBusy : Bool
    , refreshing : Bool
    , filter : String
    , confirmingDiscard : Maybe Api.PendingRow
    , confirmingDiscardAll : Bool
    }


init : ( Model, Cmd Msg )
init =
    ( { rows = Loading
      , notice = Nothing
      , busyId = Nothing
      , allBusy = False
      , refreshing = False
      , filter = "all"
      , confirmingDiscard = Nothing
      , confirmingDiscardAll = False
      }
    , Api.getPending GotRows
    )


subscriptions : Model -> Sub Msg
subscriptions model =
    Sub.batch
        [ Time.every 15000 (\_ -> Poll)
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
                        if model.confirmingDiscard /= Nothing then
                            Just CancelDiscard

                        else if model.confirmingDiscardAll then
                            Just CancelDiscardAll

                        else
                            Nothing

                    "r" ->
                        if model.confirmingDiscard == Nothing && not model.confirmingDiscardAll then
                            Just Refresh

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


type Msg
    = Poll
    | Refresh
    | GotRows (Api.Data (List Api.PendingRow))
    | SetFilter String
    | Retry String
    | AskDiscard Api.PendingRow
    | CancelDiscard
    | ConfirmDiscard
    | RetryAll
    | AskDiscardAll
    | CancelDiscardAll
    | DiscardAll
    | RetryDone (Api.Data Bool)
    | DiscardDone (Api.Data Bool)
    | RetryAllDone (Api.Data Int)
    | DiscardAllDone (Api.Data Int)
    | Dismiss


update : Msg -> Model -> ( Model, Cmd Msg )
update msg model =
    case msg of
        Poll ->
            ( model, Api.getPending GotRows )

        Refresh ->
            ( { model | refreshing = True }
            , Api.getPending GotRows
            )

        GotRows result ->
            ( { model | rows = result, refreshing = False }, Cmd.none )

        SetFilter filter ->
            ( { model | filter = filter }, Cmd.none )

        Retry id ->
            ( { model | busyId = Just id, notice = Nothing }
            , Api.retryPending id RetryDone
            )

        AskDiscard entry ->
            ( { model | confirmingDiscard = Just entry }, Cmd.none )

        CancelDiscard ->
            ( { model | confirmingDiscard = Nothing }, Cmd.none )

        ConfirmDiscard ->
            case model.confirmingDiscard of
                Just entry ->
                    ( { model | busyId = Just entry.id, confirmingDiscard = Nothing, notice = Nothing }
                    , Api.discardPending entry.id DiscardDone
                    )

                Nothing ->
                    ( model, Cmd.none )

        RetryAll ->
            ( { model | allBusy = True, notice = Nothing }
            , Api.retryAllPending RetryAllDone
            )

        AskDiscardAll ->
            ( { model | confirmingDiscardAll = True }, Cmd.none )

        CancelDiscardAll ->
            ( { model | confirmingDiscardAll = False }, Cmd.none )

        DiscardAll ->
            ( { model | allBusy = True, confirmingDiscardAll = False, notice = Nothing }
            , Api.discardAllPending DiscardAllDone
            )

        RetryDone result ->
            case result of
                Success _ ->
                    ( { model | busyId = Nothing, notice = Just ( Ui.Success, "Queued." ) }
                    , Api.getPending GotRows
                    )

                Failure err ->
                    ( { model | busyId = Nothing, notice = Just ( Ui.Error, Api.errorMessage err ) }
                    , Cmd.none
                    )

                _ ->
                    ( model, Cmd.none )

        DiscardDone result ->
            case result of
                Success _ ->
                    ( { model | busyId = Nothing, notice = Just ( Ui.Success, "Discarded." ) }
                    , Api.getPending GotRows
                    )

                Failure err ->
                    ( { model | busyId = Nothing, notice = Just ( Ui.Error, Api.errorMessage err ) }
                    , Cmd.none
                    )

                _ ->
                    ( model, Cmd.none )

        RetryAllDone result ->
            case result of
                Success n ->
                    ( { model | allBusy = False, notice = Just ( Ui.Success, "Queued " ++ String.fromInt n ++ "." ) }
                    , Api.getPending GotRows
                    )

                Failure err ->
                    ( { model | allBusy = False, notice = Just ( Ui.Error, Api.errorMessage err ) }
                    , Cmd.none
                    )

                _ ->
                    ( model, Cmd.none )

        DiscardAllDone result ->
            case result of
                Success n ->
                    ( { model | allBusy = False, notice = Just ( Ui.Success, "Deleted " ++ String.fromInt n ++ "." ) }
                    , Api.getPending GotRows
                    )

                Failure err ->
                    ( { model | allBusy = False, notice = Just ( Ui.Error, Api.errorMessage err ) }
                    , Cmd.none
                    )

                _ ->
                    ( model, Cmd.none )

        Dismiss ->
            ( { model | notice = Nothing }, Cmd.none )


view : Model -> Html Msg
view model =
    let
        hasRows =
            case model.rows of
                Success rows ->
                    not (List.isEmpty rows)

                _ ->
                    False
    in
    section [ class "pending-page" ]
        [ div [ class "pending-head" ]
            [ div []
                [ h1 [] [ text "Pending dispatches" ]
                , p [ class "pending-lede" ]
                    [ text
                        "Rows here are inbound emails that hit a dispatch failure. The retry queue re-runs them on a backoff; dead-lettered rows have exhausted their retries and the original sender was notified."
                    ]
                ]
            , div [ class "pending-actions" ]
                [ Ui.iconButton
                    { icon = Icons.refresh
                    , label = "Refresh (r)"
                    , onClick = Refresh
                    , variant = Ghost
                    , disabled = model.refreshing || model.rows == Loading
                    , small = False
                    }
                , Ui.button
                    { label = "Retry all"
                    , onClick = RetryAll
                    , variant = Secondary
                    , busy = model.allBusy
                    , disabled = not hasRows || model.allBusy
                    }
                , Ui.button
                    { label = "Delete all"
                    , onClick = AskDiscardAll
                    , variant = Danger
                    , busy = False
                    , disabled = not hasRows || model.allBusy
                    }
                ]
            ]
        , case model.notice of
            Just ( kind, message ) ->
                div [ class "notice-bar", onClick Dismiss, title "Click to dismiss" ]
                    [ Ui.banner kind message ]

            Nothing ->
                text ""
        , case model.rows of
            Loading ->
                Ui.spinner

            NotAsked ->
                Ui.spinner

            Failure err ->
                Ui.banner Ui.Error (Api.errorMessage err)

            Success rows ->
                if List.isEmpty rows then
                    Ui.emptyState
                        { icon = Icons.check
                        , title = "Nothing stuck"
                        , body = "Failed dispatches land here. Right now the queue is clear."
                        , action = Nothing
                        }

                else
                    let
                        visible =
                            visibleRows model.filter rows
                    in
                    div [ class "pending-content" ]
                        [ pendingToolbar model.filter rows
                        , if List.isEmpty visible then
                            div [ class "card empty-inline" ]
                                [ text "No pending dispatches match this filter." ]

                          else
                            table [ class "pending-table" ]
                                [ thead []
                                    [ tr []
                                        [ th [] [ text "Status" ]
                                        , th [] [ text "From → To" ]
                                        , th [] [ text "Attempts" ]
                                        , th [] [ text "Last error" ]
                                        , th [] [ text "Action" ]
                                        ]
                                    ]
                                , tbody [] (List.map (row model.busyId) visible)
                                ]
                        ]
        , discardModal model
        , if model.confirmingDiscardAll then
            Ui.modal
                { title = "Delete all pending"
                , onClose = CancelDiscardAll
                , body =
                    [ p []
                        [ text
                            "Delete every pending dispatch and its stored message body? This can't be undone."
                        ]
                    ]
                , footer =
                    [ Ui.button { label = "Cancel", onClick = CancelDiscardAll, variant = Ghost, busy = False, disabled = False }
                    , Ui.button { label = "Delete all", onClick = DiscardAll, variant = Danger, busy = model.allBusy, disabled = False }
                    ]
                }

          else
            text ""
        ]


visibleRows : String -> List Api.PendingRow -> List Api.PendingRow
visibleRows filter rows =
    case filter of
        "queued" ->
            List.filter (\r -> not r.deadLettered) rows

        "dead" ->
            List.filter .deadLettered rows

        _ ->
            rows


pendingToolbar : String -> List Api.PendingRow -> Html Msg
pendingToolbar currentFilter rows =
    let
        totalCount =
            List.length rows

        queuedCount =
            List.length (List.filter (\r -> not r.deadLettered) rows)

        deadCount =
            List.length (List.filter .deadLettered rows)
    in
    div [ class "pending-toolbar" ]
        [ div [ class "pending-filter-chips" ]
            [ filterChip ("All (" ++ String.fromInt totalCount ++ ")") (currentFilter == "all") (SetFilter "all")
            , filterChip ("Queued (" ++ String.fromInt queuedCount ++ ")") (currentFilter == "queued") (SetFilter "queued")
            , filterChip ("Dead-lettered (" ++ String.fromInt deadCount ++ ")") (currentFilter == "dead") (SetFilter "dead")
            ]
        ]


filterChip : String -> Bool -> Msg -> Html Msg
filterChip label isActive onSelect =
    button
        [ classList
            [ ( "pending-filter-chip", True )
            , ( "is-active", isActive )
            ]
        , type_ "button"
        , onClick onSelect
        ]
        [ text label ]


discardModal : Model -> Html Msg
discardModal model =
    case model.confirmingDiscard of
        Just entry ->
            let
                busy =
                    model.busyId == Just entry.id
            in
            Ui.modal
                { title = "Discard dispatch"
                , onClose = CancelDiscard
                , body =
                    [ p []
                        [ text "Discard this dispatch and permanently delete its stored message body? This can't be undone." ]
                    , div [ class "pending-confirm-meta" ]
                        [ span [ class "pr-from" ] [ text entry.sender ]
                        , span [ class "pr-arrow" ] [ text "→" ]
                        , span [ class "pr-to" ] [ text entry.recipient ]
                        ]
                    ]
                , footer =
                    [ Ui.button { label = "Cancel", onClick = CancelDiscard, variant = Ghost, busy = False, disabled = False }
                    , Ui.button { label = "Discard", onClick = ConfirmDiscard, variant = Danger, busy = busy, disabled = busy }
                    ]
                }

        Nothing ->
            text ""


row : Maybe String -> Api.PendingRow -> Html Msg
row busyId entry =
    let
        busy =
            busyId == Just entry.id
    in
    tr [ classList [ ( "pending-row", True ), ( "is-dead", entry.deadLettered ) ] ]
        [ td []
            [ span
                [ classList
                    [ ( "pstatus", True )
                    , ( if entry.deadLettered then
                            "k-dead"

                        else
                            "k-queued"
                      , True
                      )
                    ]
                ]
                [ text
                    (if entry.deadLettered then
                        "dead-lettered"

                     else
                        "queued"
                    )
                ]
            ]
        , td [ class "mono" ]
            [ span [ class "pr-from" ] [ text entry.sender ]
            , span [ class "pr-arrow" ] [ text "→" ]
            , span [ class "pr-to" ] [ text entry.recipient ]
            ]
        , td [ class "mono" ] [ text (String.fromInt entry.attempts ++ "×") ]
        , td [ class "errcell", title (Maybe.withDefault "" entry.lastError) ]
            [ text (Maybe.withDefault "—" (Maybe.map (String.left 200) entry.lastError)) ]
        , td [ class "pr-actions" ]
            [ a [ class "btn btn-ghost btn-sm", href ("/inbox/" ++ entry.id) ]
                [ text "View" ]
            , Ui.button
                { label = "Retry"
                , onClick = Retry entry.id
                , variant = Secondary
                , busy = busy
                , disabled = busy
                }
            , Ui.button
                { label = "Discard"
                , onClick = AskDiscard entry
                , variant = Danger
                , busy = busy
                , disabled = busy
                }
            ]
        ]
