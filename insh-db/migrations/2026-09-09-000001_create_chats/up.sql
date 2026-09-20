CREATE TABLE IF NOT EXISTS chats (
    id      BIGSERIAL   PRIMARY KEY,
    title   TEXT        NOT NULL,
    dir     TEXT        NOT NULL,
    created TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated TIMESTAMPTZ NOT NULL DEFAULT now()
);

-- The sidebar lists the chats which were used most recently first.
CREATE INDEX IF NOT EXISTS chats_updated ON chats (updated DESC);

-- Searching chats matches titles without regard to case, which an index on the title could not be
-- used for. There is no index for it because a person has few enough chats that scanning them
-- costs nothing, unlike the message contents, which is why only those have one.

CREATE TABLE IF NOT EXISTS chat_messages (
    id              BIGSERIAL   PRIMARY KEY,
    chat_id         BIGINT      NOT NULL REFERENCES chats (id) ON DELETE CASCADE,
    role            TEXT        NOT NULL,
    content         TEXT        NOT NULL,
    created         TIMESTAMPTZ NOT NULL DEFAULT now(),

    -- The vector of the message, and the model which made it. The model is recorded so that
    -- vectors made by a model which is no longer the one in use can be told apart and made again
    -- rather than compared against vectors they mean nothing next to.
    embedding       VECTOR(256),
    embedding_model TEXT
);

-- Reading a chat reads its messages oldest first.
CREATE INDEX IF NOT EXISTS chat_messages_chat ON chat_messages (chat_id, id);

-- Searching by meaning orders by how far the vectors are apart.
CREATE INDEX IF NOT EXISTS chat_messages_embedding ON chat_messages USING hnsw (embedding vector_cosine_ops);

-- Backfilling looks for the messages which do not have a vector yet.
CREATE INDEX IF NOT EXISTS chat_messages_unembedded ON chat_messages (id) WHERE embedding IS NULL;
