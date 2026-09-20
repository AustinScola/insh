-- pgvector is built by the build script and installed alongside the PostgreSQL server by
-- `Database::install_pgvector`, so the files this needs are already in place.
CREATE EXTENSION IF NOT EXISTS vector;
