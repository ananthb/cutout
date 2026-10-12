module Format exposing (bytes, clock, count, dateShort, relative)

{-| Pure formatting helpers.

These are deliberately in Elm rather than a port or a server round-trip:
`view` can't wait on a port, and the live feed re-renders on every tick, so a
request-per-render would be absurd.

-}


{-| Relative time ("just now", "4m ago") from a unix-millisecond timestamp,
anchored to `now_ms`.
-}
relative : Int -> Int -> String
relative nowMs tsMs =
    let
        delta =
            max 0 (nowMs - tsMs) // 1000
    in
    if delta < 5 then
        "just now"

    else if delta < 60 then
        String.fromInt delta ++ "s ago"

    else if delta < 3600 then
        String.fromInt (delta // 60) ++ "m ago"

    else if delta < 86400 then
        String.fromInt (delta // 3600) ++ "h ago"

    else
        String.fromInt (delta // 86400) ++ "d ago"


{-| Wall-clock `HH:MM:SS` in the viewer's local zone. `offsetMin` is what
`new Date().getTimezoneOffset()` returns (minutes to add to local to reach
UTC), which is why it is subtracted rather than added.
-}
clock : Int -> Int -> String
clock offsetMin tsMs =
    let
        totalSeconds =
            tsMs // 1000 - offsetMin * 60

        secondsInDay =
            modBy 86400 totalSeconds

        h =
            secondsInDay // 3600

        m =
            modBy 60 (secondsInDay // 60)

        s =
            modBy 60 secondsInDay

        pad n =
            if n < 10 then
                "0" ++ String.fromInt n

            else
                String.fromInt n
    in
    pad h ++ ":" ++ pad m ++ ":" ++ pad s


{-| Filesize in the live feed's compact style.
-}
bytes : Int -> String
bytes n =
    if n < 1024 then
        String.fromInt n ++ "B"

    else if n < 1048576 then
        String.fromFloat (toFloat n / 1024) ++ "kb"

    else
        String.fromFloat (toFloat n / 1048576) ++ "mb"


{-| Thousands separators, for counts that can get large.
-}
count : Int -> String
count n =
    group (String.fromInt n) []


group : String -> List String -> String
group remaining acc =
    if String.length remaining <= 3 then
        String.join "" (remaining :: List.reverse acc)

    else
        let
            ( head, tail ) =
                ( String.dropRight 3 remaining, String.right 3 remaining )
        in
        group head (tail :: acc)


{-| `2026-05-04T13:20:11Z` -> `2026-05-04 13:20`. Just enough for a row label.
-}
dateShort : String -> String
dateShort iso =
    iso
        |> String.replace "T" " "
        |> String.left 16
