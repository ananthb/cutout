module Glob exposing (match)

{-| The routing engine's glob matcher, ported to Elm so the tester can run the
whole ruleset on every keystroke without a request.

Mirrors `email::routing` / the old Alpine `glob()`: case-insensitive, `*`
matches any run (including empty) and `?` matches exactly one character.

-}


{-| True when `pattern` matches `value`, case-insensitively.
-}
match : String -> String -> Bool
match pattern value =
    go (String.toLower pattern) (String.toLower value)


go : String -> String -> Bool
go pattern value =
    case ( String.uncons pattern, String.uncons value ) of
        ( Nothing, Nothing ) ->
            True

        ( Just ( '*', restP ), valueRest ) ->
            -- `*` can match nothing, or consume one char and try again.
            go restP value
                || (case valueRest of
                        Just ( _, restV ) ->
                            go pattern restV

                        Nothing ->
                            False
                   )

        ( Just ( '?', restP ), Just ( _, restV ) ) ->
            go restP restV

        ( Just ( p, restP ), Just ( v, restV ) ) ->
            p == v && go restP restV

        _ ->
            False
