module Route exposing (Route(..), fromUrl, href, toPath)

{-| URL parsing. The list here must stay in step with the Worker's routing in
`src/lib.rs`: the Worker serves the SPA shell only for paths it recognises, so
a route added here without a matching arm there 404s on a hard refresh.
-}

import Html
import Html.Attributes
import Url exposing (Url)
import Url.Parser as Parser exposing ((</>), Parser, oneOf, s)


type Route
    = Manage
    | Pending
    | Inbox
    | InboxMessage String
    | NotFound


parser : Parser (Route -> a) a
parser =
    oneOf
        [ Parser.map Pending (s "manage" </> s "pending")
        , Parser.map Manage (s "manage")
        , Parser.map InboxMessage (s "inbox" </> Parser.string)
        , Parser.map Inbox (s "inbox")
        ]


fromUrl : Url -> Route
fromUrl url =
    Parser.parse parser url |> Maybe.withDefault NotFound


toPath : Route -> String
toPath route =
    case route of
        Manage ->
            "/manage"

        Pending ->
            "/manage/pending"

        Inbox ->
            "/inbox"

        InboxMessage id ->
            "/inbox/" ++ id

        NotFound ->
            "/manage"


href : Route -> Html.Attribute msg
href route =
    Html.Attributes.href (toPath route)
