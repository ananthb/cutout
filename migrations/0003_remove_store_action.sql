-- Defensively rewrite any legacy Store-only action to a Drop before the
-- `Action` enum loses its `store` variant. The operator has confirmed no rule
-- currently uses a Store-only action, so in practice this matches zero rows;
-- the rewrite exists so that a pre-existing blob containing
-- `{"type":"store",...}` can never fail deserialization (and take down the
-- rules read path) once the variant is gone. A Store-only rule stored the
-- message and forwarded nothing, so the nearest surviving behaviour is a
-- silent Drop.
UPDATE rules_state
SET rules_json = (
    SELECT json_group_array(
        CASE
            WHEN json_extract(value, '$.action.type') = 'store'
                THEN json_set(value, '$.action.type', 'drop')
            ELSE value
        END
    )
    FROM json_each(rules_state.rules_json)
)
WHERE id = 1
  AND json_valid(rules_json)
  AND EXISTS (
      SELECT 1
      FROM json_each(rules_state.rules_json)
      WHERE json_extract(value, '$.action.type') = 'store'
  );
