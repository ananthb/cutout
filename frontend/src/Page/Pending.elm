module Page.Pending exposing (Model, Msg, init, subscriptions, update, view)

{-| Pending dispatches: failed sends still in the retry queue, and rows that
exhausted their retries and were dead-lettered. Retry kicks a row back into the
queue; discard cuts it loose and deletes its stored body.
-}

import Api
import Html exposing (Html, a, div, h1, p, section, span, table, tbody, td, text, th, thead, tr)
import Html.Attributes as A exposing (class, classList, href, title)
import Html.Events exposing (onClick)
import Icons
import RemoteData exposing (RemoteData(..))
import Route
import Time
import Ui exposing (ButtonVariant(..))


type alias Model =
    { rows : Api.Data (List Api.PendingRow)
    , notice : Maybe ( Ui.Banner, String )
    , busyId : Maybe String
    , allBusy : Bool
    , confirmingDiscardAll : Bool
    }


init : ( Model, Cmd Msg )
init =
    ( { rows = Loading
      , notice = Nothing
      , busyId = Nothing
      , allBusy = False
      , confirmingDiscardAll = False
      }
    , Api.getPending GotRows
    )


subscriptions : Model -> Sub Msg
subscriptions _ =
    Time.every 15000 (\_ -> Poll)


type Msg
    = Poll
    | GotRows (Api.Data (List Api.PendingRow))
    | Retry String
    | Discard String
    | RetryAll
    | AskDiscardAll
    | CancelDiscardAll
    | DiscardAll
    | ActionDone (Api.Data Bool)
    | RetryAllDone (Api.Data Int)
    | DiscardAllDone (Api.Data Int)
    | Dismiss


update : Msg -> Model -> ( Model, Cmd Msg )
update msg model =
    case msg of
        Poll ->
            ( model, Api.getPending GotRows )

        GotRows result ->
            ( { model | rows = result }, Cmd.none )

        Retry id ->
            ( { model | busyId = Just id, notice = Nothing }
            , Api.retryPending id ActionDone
            )

        Discard id ->
            ( { model | busyId = Just id, notice = Nothing }
            , Api.discardPending id ActionDone
            )

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

        ActionDone result ->
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
                [ Ui.button
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
                , a [ class "btn btn-secondary", href (Route.toPath Route.Manage) ]
                    [ Icons.chevronRight, text "Back to rules" ]
                ]
            ]
        , case model.notice of
            Just ( kind, message ) ->
                Ui.banner kind message

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
                        , tbody [] (List.map (row model.busyId) rows)
                        ]
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
            [ Ui.button
                { label = "Retry"
                , onClick = Retry entry.id
                , variant = Secondary
                , busy = busy
                , disabled = busy
                }
            , Ui.button
                { label = "Discard"
                , onClick = Discard entry.id
                , variant = Danger
                , busy = busy
                , disabled = busy
                }
            ]
        ]
