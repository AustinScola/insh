# Logging

Logging for debug purposes can be enabled by building with the "logging" feature.

It is recommended to use a unix named pipe for the logs. One way to do this is with the tool
`socat` (`brew install socat`). Use `socat` to create a named pipe and have it echo the contents to
stdout with the command `socat -u pipe:<log-file>,mode=700 -` where `<log-file>` is the file path
for the named pipe (e.g. `/tmp/insh-log`). 

For example:
```
socat -u pipe:/tmp/insh-log,mode=700 -
```

Then run `Insh` with the argument `--log-file <log-file>`.

For example:
```
cargo run --bin insh --features logging -- --log-file /tmp/insh-log
```

## Log Levels

The overall log level can be controlled with the `--log-level` argument and log levels for
individual modules can be controlled with `--module-log-level` arguments. Each module log
level argument is of the form `<module-name>=<log-level>`.

Example:
```
cargo run \
    --features logging -- \
    --log-file /tmp/insh-log \
    --log-level error \
    --module-log-level insh::auto_completers::search_completer=debug
```
