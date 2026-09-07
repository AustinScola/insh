# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## What this is

Insh is a graphical, interactive terminal environment (a TUI file browser / file finder / file
contents searcher). It ships as two binaries: `insh` (the terminal client) and `inshd` (a
background daemon that does filesystem work on the client's behalf). See `README.md` for the user
facing keybindings and the `~/.insh-config.yaml` configuration options.

## Repository setup

The repo uses git submodules (`cargo-scripts`, `bash-lib`). Clone/checkout with
`git submodule update --init` or the scripts under `cargo-scripts/` will be missing.

`.envrc` adds `cargo-scripts/` and `scripts/` to `PATH` via `direnv`, so the scripts are typically
invoked by bare name (`check`, `lint`, `all`). Without direnv, invoke them by path
(`./cargo-scripts/check`). Note that `./scripts/all` shells out to unqualified `format`/`check`/`lint`,
so it only works when they are on `PATH`.

## Commands

| Command | What it does |
|---|---|
| `./cargo-scripts/check` | `cargo check` |
| `./cargo-scripts/build` | `cargo build` |
| `./cargo-scripts/release` | `cargo build --release` |
| `./cargo-scripts/lint` | `cargo clippy` |
| `./cargo-scripts/format` | `cargo fmt` (pass `-- --check` to verify only) |
| `./scripts/check-with-logging-feature` | `cargo check --features logging` |
| `./scripts/lint-arbsego` | Runs `arbsego`, a custom linter; installs the version pinned in `truth.yaml` if missing |
| `./scripts/all` | format + check + lint + logging-feature check + arbsego |
| `cargo test` | Run all tests |
| `cargo test -p rend yarn` | Run tests in one crate matching a name |

Tests are inline `#[cfg(test)] mod tests` blocks (in `rend`, in `insh-db`, and in `insh`'s `string`,
`ansi_escaped_text`, `args`, and `searcher/contents` modules), using `test-case` for
parameterization. There are no integration test directories.

CI (`.github/workflows/rust.yaml`) runs check (ubuntu + macos), check-with-logging, clippy, format
`--check`, arbsego, and `cargo test`. The test job installs `libxcb1-dev` (needed by `copypasta`
for clipboard support).

## Running it

**Do not run `insh` or `inshd`.** The maintainer keeps both running while developing and does the
testing himself; starting or restarting the daemon would disturb the instances already running.
Build, check, lint, and `cargo test` freely — just stop short of launching the binaries. When a
change needs to be exercised in the real app, describe what to try and hand it back.

For reference: `inshd` must be running before `insh` will start — `insh` connects to the unix socket
at `~/.insh/daemon/inshd.sock` and exits if the connection fails. `inshd` takes `start`, `stop`,
`restart`, and `status`; `insh` takes `browse`, `find`, `search`, and `edit`.

### Logging

Logging is behind the `logging` cargo feature for `insh` (and is transitively enabled on `til`);
`inshd` always logs. The recommended workflow (see `notes/logging.md`) is to log to a named pipe:

```
socat -u pipe:/tmp/insh-log,mode=700 -                       # in one terminal
cargo run --bin insh --features logging -- --log-file /tmp/insh-log \
    --log-level error --module-log-level insh::auto_completers::search_completer=debug
```

Because `insh` only compiles `log` in under the feature, every `log::` call site in `insh` needs a
`#[cfg(feature = "logging")]` attribute, and `./scripts/check-with-logging-feature` is what catches
mistakes here — plain `cargo check` will not.

## Architecture

### Crate roles (workspace members)

- **`insh`** — the TUI client. UI components, config, clipboard, and the client side of the daemon
  protocol.
- **`inshd`** — the daemon. Multi-threaded server over a unix socket that performs file
  finding/getting/creating, contents searching, search suggestions, and owns persistent data
  (in an embedded PostgreSQL database) and its own config (`~/.inshd-config.yaml`).
- **`insh-api`** — the wire protocol shared by both: `Request`/`RequestParams` and
  `Response`/`ResponseParams`, postcard-serialized.
- **`insh-db`** — the embedded PostgreSQL database and everything Diesel: the `Database` lifecycle,
  the schema, the migrations, and the query API. Only `inshd` depends on it.
- **`til`** — the TUI application framework ("terminal interface library"). `App`, the `Component`
  trait, the event loop, and running foreign programs (vim, bash) that take over the terminal.
- **`rend`** — styled-text rendering primitives: `Cell` (one column, holding a grapheme cluster),
  `Yarn` (a styled line, measured in columns rather than characters), `Fabric` (a grid of yarns,
  stacked with `quilt_bottom`), and `Renderer`, which writes a whole `Fabric` to a terminal. The
  `Renderer` is generic over what it writes to so its output can be checked in tests; it repaints
  every cell rather than diffing, and only writes a select-graphic-rendition sequence where the
  colors actually change.
- **`term`** — raw terminal control (termios, SIGWINCH-based resize detection), reading input, and
  the `TermEvent` / `Key` types. Also builds a `print-event` debug binary.
- **`ansi`** — the ANSI escape sequences which the terminal and the programs in it send, and the
  colors and styles they carry. Two layers: `AnsiEscapeSequence` is the structure of a sequence,
  and `ControlFunction` is what one means. Both directions work, so anything here can be written
  back out as bytes, as short as it goes. It has no dependencies and does no I/O — `term` uses it
  to read input and `rend` uses it to draw, which is how the two of them share one vocabulary.
  `ansi/src/lib.rs` lists the standards it is written against; check anything added to it there.
- **`size`**, **`file-type`**, **`file-info`**, **`path-finder`**, **`phrase-searcher`**, **`common`**
  — small shared leaf crates. `path-finder` does regex-matched, gitignore-respecting directory
  walking; `phrase-searcher` walks a directory tree matching file contents against a phrase;
  `common` holds the `~/.insh` path constants and directory permissions used by both binaries.

### Client/daemon split

`insh` and `inshd` share a version number and are released together (`notes/release_process.md`).
Communication is a `UnixStream` at `common::paths::INSHD_SOCKET`, with postcard-encoded
`insh_api::Request`/`Response`. Each `Request` carries a `Uuid`; responses echo that uuid and set
`last` on the final one, so a single request may stream multiple responses (e.g. find-files
results arriving incrementally). Components hold the uuid of their in-flight request and ignore
responses that don't match.

On the client, `til::App` owns two boxed traits — `Requester` (drains a request channel to the
socket) and `ResponseHandler` (reads the socket into a response channel) — implemented in
`insh/src/requester.rs` and `insh/src/response_handler.rs`. `inshd` is a thread-per-role design:
`ConnHandler` accepts connections, a `ClientHandler` per client, a `Scheduler` +
`RequestHandlerManager` pool for work, and a `ResponseHandler` writing back, all wired with
`crossbeam` channels.

Browsing, finding, file creation, contents searching, and search suggestions all go through
`inshd`. The `phrase-searcher` crate holds the shared, serializable hit types (`FileHit`/`LineHit`)
and the walking/matching logic; `inshd/src/file_searcher.rs` runs it on a worker thread per search,
mirroring how `path-finder`/`inshd/src/file_finder.rs` back file finding.

### Component model (`til`)

```rust
pub trait Component<Props, Event, Effect> {
    fn new(props: Props) -> Self;
    fn on_created(&mut self) -> Option<Box<dyn Iterator<Item = Effect>>> { None }
    fn handle(&mut self, event: Event) -> Option<Effect>;
    fn render(&self, size: Size) -> Fabric;
}
```

`App::run` owns the event loop: it `select!`s over terminal events and daemon responses, feeds them
to the root component as `til::Event<Response>`, and interprets the returned
`til::SystemEffect<Request>` (`RunProgram`, `Request`, `Bell`, `Exit`). Rendering is pull-based —
after each event the root's `render(size)` produces a `Fabric` which the `Renderer` writes to the
screen.

The loop waits for one event, then goes on handling however many are already waiting before
rendering again. Holding a key down sends them faster than the screen can be drawn for each one, and
the ones which are skipped over have all been superseded by the ones after them anyway. Nothing is
ever waited for in order to batch them, so a key pressed on its own is still drawn straight away.

`SystemEffect::RunProgram` suspends the TUI, runs a `til::Program` (see `insh/src/programs/` for
vim and bash), forwards terminal resizes to it, and restores the screen after. Vim's stdout is
piped through a parser that strips alternative-screen escape codes.

`insh/src/components/insh.rs` is the root component; it owns a `Mode` and delegates to `Browser`,
`Finder`, `Searcher`, or `FileCreator`, translating each child's `Effect` into an `Action` on its
own state. Nesting follows the same shape all the way down (e.g. `Browser` → `Dir` + `Contents`).

### Conventions to follow when adding code

- **Props/State/Effect/Action per component.** Each component file defines nested private
  `mod props`, `mod state`, `mod effect`, `mod action` blocks and re-exports the public names with
  `pub use`. This keeps each type's imports local. Match this layout rather than flattening.
- **Imports are grouped and ordered**: `std`, then crate-local (`super::` before `crate::`), then
  workspace sibling crates (`common`, `insh_api`, `insh_db`, `rend`, `til`, ...), then third-party.
  A blank line separates each group and each group is alphabetical. Note that a workspace sibling
  goes in the third group, not with `crate::` — `insh_db` belongs next to `insh_api`. Existing files
  vary, so follow this rather than whichever file you happen to be reading.
- **State transitions go through `Stateful`** (`insh/src/stateful.rs`):
  `fn perform(&mut self, action: Action) -> Option<Effect>`. Components translate events into
  actions and let the state machine produce effects.
- **`typed-builder`** is used pervasively for struct construction (`Foo::builder()...build()`) —
  new structs with optional fields should derive `TypedBuilder` rather than hand-writing
  constructors.
- **Docs are enforced.** `insh` has `#![deny(warnings)]` + `#![deny(missing_docs)]`, and `inshd`
  additionally has `#![deny(clippy::missing_docs_in_private_items)]` — every item in `inshd`,
  including private ones, needs a doc comment. `til`, `rend`, `term`, and the leaf crates do not
  enforce this.
- Crate-level `#![allow(clippy::needless_return)]` is common here; explicit `return` in tail
  position is idiomatic in this codebase.
- New crates go in the workspace `members` list in the root `Cargo.toml` and depend on siblings by
  relative path.

### Persistent state

`~/.insh/` (mode 0700) holds `daemon/` (socket, pid file, logs) and `daemon/postgres/` (the
embedded database). `insh` never touches any of it directly, instead going through the daemon
(e.g. `SearchPhrase`/`SuggestSearchPhrase` requests).

Application data lives in an embedded PostgreSQL database in the `insh-db` crate, which `Server::run`
starts and stops. The PostgreSQL binaries are embedded in the `inshd` executable at build time by
`postgresql_embedded`'s `bundled` feature, so PostgreSQL does not have to be installed separately;
they are extracted to a directory named after their version under `daemon/postgres/` on first run,
which is also where the unix socket goes. `listen_addresses` is set to empty so the server never
binds a TCP port.

**Connections are authenticated by operating system user, not by a password.** `insh-db` writes
`pg_hba.conf` and `pg_ident.conf` into the data directory after `setup()` and before `start()`, on
every start so a renamed user still works. The ident map is needed because `postgresql_embedded`
hardcodes the superuser as `postgres`, which is not the name of the user running the daemon. A
password still has to exist because `initdb` is always run with `--auth password`, but `insh-db`
writes the password file itself (`postgresql_embedded` would create it world readable) and deletes
it once the database is created, so no secret is left on disk.

Handlers go through the query API in `insh-db/src/search_history.rs` rather than touching the schema
directly.

**Permissions matter here and nothing enforces them for you.** Several dependencies create files
under `~/.insh` with whatever the process umask allows: `flexi_logger` makes the log directories,
`UnixListener` makes the socket and `postgresql_embedded` extracts the server binaries.
`common::paths::make_private_dir` creates a directory 0700 — along with the ones above it, up to but
never past `~/.insh` — and fixes up any that already exist. **Each module makes the directories it
needs itself** rather than `main` doing it up front: `logging.rs` makes the log directory before
configuring the logger, and `insh-db` makes the PostgreSQL directory. The `daemon` crate sets the
pid file's mode explicitly when it creates it. Anything new that lands in `~/.insh` needs its mode
set explicitly. Do not reach for a process-wide umask: `request_handlers/create_file.rs`
creates files for the *user*, and tightening the daemon umask would silently make those private too.
The log files are the one exception — `flexi_logger` has no permissions option, so they keep the
daemon's umask and are protected by the 0700 `logs/` directory instead.

Queries go through Diesel. `insh-db/src/schema.rs` is written by hand (not generated with
`diesel print-schema`) and migrations under `insh-db/migrations/` are embedded with
`embed_migrations!`, so **building never requires a running database** — keep it that way.
`insh-db/build.rs` exists only to re-run the build when a migration changes.

Because `postgresql_embedded` needs a newer toolchain than insh otherwise would, `rust-toolchain.toml`
pins one, and `.cargo/config.toml` pins `POSTGRESQL_VERSION` to an exact version so the build script
reuses its `~/.theseus` download cache instead of re-resolving over the network every build.

The `search_history` table holds one row per distinct phrase, with `last_searched` bumped by an
upsert when a phrase is searched for again, so suggestions do not repeat.

Configuration is split by binary and read-only for each: `insh` reads `~/.insh-config.yaml` via
`insh/src/config.rs`; `inshd` separately reads `~/.inshd-config.yaml` via `inshd/src/config.rs`
(`searcher.history.length`, which governs how many rows `inshd` keeps in the `search_history`
table; `server.request_handlers`; and `database.pool.size`, which defaults to
`server.request_handlers` — `Config::db_conn_pool_size` resolves that, since serde cannot express a
default which depends on another field). `insh-db` installs an r2d2 event handler which logs how
many connections are in use as it changes.

Note the existing `SearcherHistoryConfig` puts `#[serde(default)]` on the `length` *field*, which
resolves to `usize::default()` — that is `0`, not the struct's `Default` of `1000`. New config
structs should use `#[serde(default = "...")]` naming a function instead, the way
`DatabasePoolConfig::size` does.

## Releasing

**Releases are done manually by the maintainer** — don't perform the release process. The steps are
documented in `notes/release_process.md` (matching version bumps in `insh/Cargo.toml` and
`inshd/Cargo.toml`, rebuild so `Cargo.lock` updates, changelog entry, then tagging `v<version>`,
moving the `latest` tag, and drafting the GitHub release); treat it as reference for answering
questions, not as a task to carry out.

Editing `CHANGELOG.md` as part of ordinary work is fine. Entries are bullet points under a
`# <version>` heading, newest first.
