# Configuration

## Insh Configuration

Insh can be configured by the file `~/.insh/insh-config.yaml`.

**general.tab_width** (`usize`): The width of the `<Tab>` character (default=`4`).

**general.bell** (`bool`): Whether or not the bell sound should be made (default=`true`).

**browser.sort** (`map`|`null`): How files are sorted alphabetically. If `null`, then files are not sorted (default=`{}`).

**browser.sort.case_insensitive** (`bool`): Whether or not the filename case is ignored when sorting files (default=`true`).

**browser.sort.hidden** (`first`|`last`|`mixed`): How hidden files (files with a leading `.`) are sorted. If `mixed`, then the leading `.` is ignored when sorting files (default=`last`).

**browser.metadata** (`bool`): Whether or not to show metadata (default=`false`).

**render.engine** (`full`|`incr`): The rendering engine to use (default=`incr`).

## Inshd Configuration

Inshd can be configured by the file `~/.insh/inshd-config.yaml`.

**server.request_handlers.num** (`usize`): The number of request handlers (default=`8`).

**database.pool.size** (`usize`): The size of the database pool (default=`8`).

**browser.history.length** (`usize`): The number of visited directories to store (default=`1000`).

**finder.history.length** (`usize`): The number of file finds to store (default=`1000`).

**searcher.history.length** (`usize`): The number of searches to store (default=`1000`).

**ai.base_url** (`str`|`null`): The base URL for the AI inference engine (default=`null`).

**ai.api_type** (`anthropic`|`openai`): The AI inference engine type (default=`anthropic`).

**ai.api_key** (`str`|`null`): The AI inference engine API key (default=`null`).

**ai.model** (`str`): The AI model (default=`claude-opus-5`).

**ai.instructions.override** (`str`|`null`): Used to override the AI instructions (default=`null`).

**ai.instructions.additional** (`str`|`null`): Additional AI instructions (default=`null`).

**ai.search.threshold** (`float`): The maximum cosine distance for semantic search to be considered a hit; lower is stricter (default=`0.7`).
