CREATE TABLE find_history (
    id         BIGSERIAL   PRIMARY KEY,
    pattern    TEXT        NOT NULL UNIQUE,
    last_found TIMESTAMPTZ NOT NULL DEFAULT now()
);

-- Suggesting a find pattern looks for the most recently used pattern which starts with what has
-- been typed so far. The `text_pattern_ops` operator class is what lets a `LIKE 'prefix%'` use the
-- index regardless of the collation the database was created with.
CREATE INDEX find_history_pattern_prefix ON find_history (pattern text_pattern_ops);

-- Both suggesting a pattern and dropping the patterns which no longer fit in the history order by
-- when the pattern was last used.
CREATE INDEX find_history_last_found ON find_history (last_found DESC);
