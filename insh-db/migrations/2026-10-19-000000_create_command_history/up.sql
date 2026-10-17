CREATE TABLE command_history (
    id        BIGSERIAL   PRIMARY KEY,
    command   TEXT        NOT NULL UNIQUE,
    last_run  TIMESTAMPTZ NOT NULL DEFAULT now()
);

-- Suggesting a command looks for the most recently run one which starts with what has been typed so
-- far. The `text_pattern_ops` operator class is what lets a `LIKE 'prefix%'` use the index
-- regardless of the collation the database was created with.
CREATE INDEX command_history_command_prefix ON command_history (command text_pattern_ops);

-- Both suggesting a command and dropping the commands which no longer fit in the history order by
-- when the command was last run.
CREATE INDEX command_history_last_run ON command_history (last_run DESC);
