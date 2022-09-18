# Logging

Logging for debug purposes can be enabled by building with the "logging" feature.

It is recommended to use a unix named pipe for the logs. One way to do this is with the tool
`socat` (`brew install socat`). Use `socat` to create a named pipe and have it echo the contents to
stdout with the command:

```
socat -u pipe:<log-file>,mode=700 -
```

where `<log-file>` is the file path for the named pipe (e.g. `/tmp/insh-pipe`). 

Then run `Insh` with the argument `--log-file <log-file>`.
