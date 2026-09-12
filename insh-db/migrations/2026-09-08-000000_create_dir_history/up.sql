CREATE TABLE dir_history (
    id           BIGSERIAL   PRIMARY KEY,
    path         TEXT        NOT NULL UNIQUE,
    last_visited TIMESTAMPTZ NOT NULL DEFAULT now()
);

-- Suggesting a directory looks for the most recently visited paths which start with what has been
-- typed so far. The `text_pattern_ops` operator class is what lets a `LIKE 'prefix%'` use the index
-- regardless of the collation the database was created with.
CREATE INDEX dir_history_path_prefix ON dir_history (path text_pattern_ops);

-- Both suggesting a directory and dropping the directories which no longer fit in the history order
-- by when the directory was last visited.
CREATE INDEX dir_history_last_visited ON dir_history (last_visited DESC);
