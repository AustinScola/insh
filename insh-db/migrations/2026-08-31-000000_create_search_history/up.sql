CREATE TABLE search_history (
    id            BIGSERIAL   PRIMARY KEY,
    phrase        TEXT        NOT NULL UNIQUE,
    last_searched TIMESTAMPTZ NOT NULL DEFAULT now()
);

-- Suggesting a search phrase looks for the most recently searched phrase which starts with what
-- has been typed so far. The `text_pattern_ops` operator class is what lets a `LIKE 'prefix%'` use
-- the index regardless of the collation the database was created with.
CREATE INDEX search_history_phrase_prefix ON search_history (phrase text_pattern_ops);

-- Both suggesting a phrase and dropping the phrases which no longer fit in the history order by
-- when the phrase was last searched for.
CREATE INDEX search_history_last_searched ON search_history (last_searched DESC);
