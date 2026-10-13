module Main exposing (main)

{-| Application entry point: one bootstrap request, then routing.

`/api/bootstrap` answers with everything the first frame needs — the operator,
which chat channels are configured, the rules and their version, 7-day stats and
the pending counts — so the first real frame is the workbench, not a skeleton.

A 401 from any request means the Cloudflare Access session is gone. The app says
so and offers a reload rather than showing a raw error, because "signed out" is
a state, not a failure.

A 401 from `/api/bootstrap` specifically means the signed-in person has no
operator access: the app drops the Rules and Pending links and sends every route
to the inbox, which is the only surface they can use.

-}

import Api
import Browser
import Browser.Navigation as Nav
import Html exposing (Html, a, button, div, main_, span, text)
import Html.Attributes exposing (class, classList, href)
import Html.Events exposing (onClick)
import Icons
import Page.Inbox as Inbox
import Page.Manage as Manage
import Page.Pending as Pending
import Ports exposing (Theme(..))
import RemoteData exposing (RemoteData(..))
import Route exposing (Route)
import Ui exposing (ButtonVariant(..))
import Url exposing (Url)


main : Program () Model Msg
main =
    Browser.application
        { init = init
        , view = view
        , update = update
        , subscriptions = subscriptions
        , onUrlChange = UrlChanged
        , onUrlRequest = LinkClicked
        }


type alias Model =
    { key : Nav.Key
    , url : Url
    , route : Route
    , boot : Api.Data Api.Snapshot
    , manage : Maybe Manage.Model
    , pending : Maybe Pending.Model
    , inbox : Maybe Inbox.Model
    , theme : Theme
    }


init : () -> Url -> Nav.Key -> ( Model, Cmd Msg )
init _ url key =
    ( { key = key
      , url = url
      , route = Route.fromUrl url
      , boot = Loading
      , manage = Nothing
      , pending = Nothing
      , inbox = Nothing
      , theme = System
      }
    , Api.getBootstrap GotBootstrap
    )



-- UPDATE


type Msg
    = UrlChanged Url
    | LinkClicked Browser.UrlRequest
    | GotBootstrap (Api.Data Api.Snapshot)
    | ManageMsg Manage.Msg
    | PendingMsg Pending.Msg
    | InboxMsg Inbox.Msg
    | SetTheme Theme
    | GotTheme String
    | Retry


update : Msg -> Model -> ( Model, Cmd Msg )
update msg model =
    case msg of
        LinkClicked request ->
            case request of
                Browser.Internal url ->
                    ( model, Nav.pushUrl model.key (Url.toString url) )

                Browser.External url ->
                    ( model, Nav.load url )

        UrlChanged url ->
            ensurePage { model | url = url, route = Route.fromUrl url }

        GotBootstrap result ->
            ensurePage { model | boot = result }

        ManageMsg sub ->
            case model.manage of
                Just manage ->
                    let
                        ( updated, cmd ) =
                            Manage.update sub manage
                    in
                    ( { model | manage = Just updated }, Cmd.map ManageMsg cmd )

                Nothing ->
                    ( model, Cmd.none )

        PendingMsg sub ->
            case model.pending of
                Just pending ->
                    let
                        ( updated, cmd ) =
                            Pending.update sub pending
                    in
                    ( { model | pending = Just updated }, Cmd.map PendingMsg cmd )

                Nothing ->
                    ( model, Cmd.none )

        InboxMsg sub ->
            case model.inbox of
                Just inbox ->
                    let
                        ( updated, cmd ) =
                            Inbox.update sub inbox
                    in
                    ( { model | inbox = Just updated }, Cmd.map InboxMsg cmd )

                Nothing ->
                    ( model, Cmd.none )

        SetTheme theme ->
            ( { model | theme = theme }, Ports.setTheme theme )

        GotTheme stored ->
            ( { model | theme = themeFromString stored }, Cmd.none )

        Retry ->
            ( { model | boot = Loading }, Api.getBootstrap GotBootstrap )


{-| Make sure the page for the current route exists, initialising it from the
bootstrap payload the first time. Once a page model exists it is kept, so
navigating away and back does not drop the workbench's loaded state.
-}
ensurePage : Model -> ( Model, Cmd Msg )
ensurePage model =
    -- No operator access means the inbox is the whole app: any other route
    -- lands there.
    if manageDenied model && not (isInboxRoute model.route) then
        ( model, Nav.replaceUrl model.key (Route.toPath Route.Inbox) )

    else
        case model.route of
            Route.Manage ->
                case model.manage of
                    Just _ ->
                        ( model, Cmd.none )

                    Nothing ->
                        case model.boot of
                            Success snap ->
                                let
                                    ( manage, cmd ) =
                                        Manage.init model.key snap
                                in
                                ( { model | manage = Just manage }, Cmd.map ManageMsg cmd )

                            _ ->
                                ( model, Cmd.none )

            Route.Pending ->
                case model.pending of
                    Just _ ->
                        ( model, Cmd.none )

                    Nothing ->
                        let
                            ( pending, cmd ) =
                                Pending.init
                        in
                        ( { model | pending = Just pending }, Cmd.map PendingMsg cmd )

            Route.Inbox ->
                inboxPage model Nothing

            Route.InboxMessage id ->
                inboxPage model (Just id)

            Route.NotFound ->
                ( model, Cmd.none )


inboxPage : Model -> Maybe String -> ( Model, Cmd Msg )
inboxPage model target =
    case model.inbox of
        Just inbox ->
            let
                ( updated, cmd ) =
                    Inbox.select target inbox
            in
            ( { model | inbox = Just updated }, Cmd.map InboxMsg cmd )

        Nothing ->
            let
                ( inbox, cmd ) =
                    Inbox.init target
            in
            ( { model | inbox = Just inbox }, Cmd.map InboxMsg cmd )


isInboxRoute : Route -> Bool
isInboxRoute route =
    case route of
        Route.Inbox ->
            True

        Route.InboxMessage _ ->
            True

        _ ->
            False


{-| True once the bootstrap has answered 401: the person is signed in to
Cloudflare Access but has no operator access. A different failure (a 500, a
network error) is a real error, not a permissions decision.
-}
manageDenied : Model -> Bool
manageDenied model =
    case model.boot of
        Failure Api.Unauthenticated ->
            True

        _ ->
            False


themeFromString : String -> Theme
themeFromString s =
    case s of
        "light" ->
            Light

        "dark" ->
            Dark

        _ ->
            System


cycleTheme : Theme -> Theme
cycleTheme theme =
    case theme of
        System ->
            Light

        Light ->
            Dark

        Dark ->
            System


themeIcon : Theme -> Html Msg
themeIcon theme =
    case theme of
        System ->
            Icons.monitor

        Light ->
            Icons.sun

        Dark ->
            Icons.moon


themeLabel : Theme -> String
themeLabel theme =
    case theme of
        System ->
            "Theme: system"

        Light ->
            "Theme: light"

        Dark ->
            "Theme: dark"



-- SUBSCRIPTIONS


subscriptions : Model -> Sub Msg
subscriptions model =
    Sub.batch
        [ Ports.theme GotTheme
        , case model.manage of
            Just manage ->
                Sub.map ManageMsg (Manage.subscriptions manage)

            Nothing ->
                Sub.none
        , case model.pending of
            Just pending ->
                Sub.map PendingMsg (Pending.subscriptions pending)

            Nothing ->
                Sub.none
        , case model.inbox of
            Just inbox ->
                Sub.map InboxMsg (Inbox.subscriptions inbox)

            Nothing ->
                Sub.none
        ]



-- VIEW


view : Model -> Browser.Document Msg
view model =
    { title = title model.route
    , body =
        [ header model
        , main_ [ class "app-main" ] [ content model ]
        , footer
        ]
    }


footer : Html Msg
footer =
    Html.footer [ class "site-footer" ]
        [ span [] [ text "Cutout · AGPL-3.0" ]
        , Html.nav [ class "site-footer-links" ]
            [ a
                [ href "https://github.com/ananthb/cutout"
                , Html.Attributes.target "_blank"
                , Html.Attributes.rel "noopener"
                ]
                [ text "Source" ]
            , a
                [ href "https://www.gnu.org/licenses/agpl-3.0.html"
                , Html.Attributes.target "_blank"
                , Html.Attributes.rel "noopener"
                ]
                [ text "License" ]
            ]
        ]


title : Route -> String
title route =
    case route of
        Route.Manage ->
            "Cutout — routing pipeline"

        Route.Pending ->
            "Pending dispatches — Cutout"

        Route.Inbox ->
            "Inbox — Cutout"

        Route.InboxMessage _ ->
            "Inbox — Cutout"

        Route.NotFound ->
            "Not found — Cutout"


operator : Model -> Maybe String
operator model =
    RemoteData.toMaybe model.boot |> Maybe.map .operator


header : Model -> Html Msg
header model =
    Html.header [ class "site-header" ]
        [ a
            [ class "brand"
            , href
                (if manageDenied model then
                    Route.toPath Route.Inbox

                 else
                    Route.toPath Route.Manage
                )
            ]
            [ span [ class "brand-mark" ] [ Icons.logo ]
            , span [ class "brand-name" ] [ text "Cutout" ]
            , span [ class "brand-sub" ] [ text "routing pipeline" ]
            ]
        , Html.nav [ class "site-nav" ]
            (if manageDenied model then
                [ navLink model.route Route.Inbox "Inbox" ]

             else
                [ navLink model.route Route.Manage "Rules"
                , navLink model.route Route.Pending "Pending"
                , navLink model.route Route.Inbox "Inbox"
                ]
            )
        , div [ class "site-header-right" ]
            [ button
                [ class "theme-toggle"
                , Html.Attributes.title (themeLabel model.theme)
                , onClick (SetTheme (cycleTheme model.theme))
                ]
                [ themeIcon model.theme ]
            , case operator model of
                Just email ->
                    span [ class "operator" ] [ text email ]

                Nothing ->
                    text ""
            , a [ class "btn btn-ghost btn-sm", href "/cdn-cgi/access/logout" ]
                [ text "Log out" ]
            ]
        ]


navLink : Route -> Route -> String -> Html Msg
navLink current target label_ =
    a
        [ classList [ ( "nav-link", True ), ( "is-active", routeIs current target ) ]
        , href (Route.toPath target)
        ]
        [ text label_ ]


{-| The Inbox tab stays lit while a message is open.
-}
routeIs : Route -> Route -> Bool
routeIs current target =
    case ( current, target ) of
        ( Route.InboxMessage _, Route.Inbox ) ->
            True

        _ ->
            current == target


content : Model -> Html Msg
content model =
    case model.route of
        -- The inbox is the reader-facing surface: it renders from its own
        -- request rather than the operator bootstrap, so a reader whose
        -- Access policy doesn't cover `/api/bootstrap` still gets their mail.
        Route.Inbox ->
            inboxContent model

        Route.InboxMessage _ ->
            inboxContent model

        _ ->
            -- A reader is being redirected to the inbox; hold the loader so the
            -- operator pages never flash.
            if manageDenied model then
                fullLoader

            else
                contentForBoot model


inboxContent : Model -> Html Msg
inboxContent model =
    case model.inbox of
        Just inbox ->
            Html.map InboxMsg (Inbox.view inbox)

        Nothing ->
            fullLoader


contentForBoot : Model -> Html Msg
contentForBoot model =
    case model.boot of
        Loading ->
            fullLoader

        NotAsked ->
            fullLoader

        Failure Api.Unauthenticated ->
            accessScreen

        Failure err ->
            div [ class "centered-screen" ]
                [ Ui.banner Ui.Error (Api.errorMessage err)
                , Ui.button
                    { label = "Retry"
                    , onClick = Retry
                    , variant = Primary
                    , busy = False
                    , disabled = False
                    }
                ]

        Success _ ->
            case model.route of
                Route.Manage ->
                    case model.manage of
                        Just manage ->
                            Html.map ManageMsg (Manage.view manage)

                        Nothing ->
                            fullLoader

                Route.Pending ->
                    case model.pending of
                        Just pending ->
                            Html.map PendingMsg (Pending.view pending)

                        Nothing ->
                            fullLoader

                Route.Inbox ->
                    fullLoader

                Route.InboxMessage _ ->
                    fullLoader

                Route.NotFound ->
                    div [ class "centered-screen" ]
                        [ Ui.emptyState
                            { icon = Icons.search
                            , title = "Nothing here"
                            , body = "That route doesn't exist. Head back to the pipeline."
                            , action = Just (a [ class "btn btn-primary", href "/manage" ] [ text "Back to rules" ])
                            }
                        ]


fullLoader : Html Msg
fullLoader =
    div [ class "full-loader" ]
        [ span [ class "full-loader-mark" ] [ Icons.logo ]
        , Ui.spinner
        ]


accessScreen : Html Msg
accessScreen =
    div [ class "centered-screen" ]
        [ Ui.emptyState
            { icon = Icons.shield
            , title = "Cloudflare Access required"
            , body = "Your session has expired or you aren't signed in. Reload to authenticate."
            , action = Just (a [ class "btn btn-primary", href "/manage" ] [ text "Reload" ])
            }
        ]
