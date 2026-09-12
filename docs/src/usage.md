# Usage

By default, Insh starts out in a file browser. Insh has two other main components as well: a file
finder and a file contents searcher.

## Universal commands
Some commands work from all components:
| Command    | Description |
|------------|-------------|
| `<Ctrl>-x` | Exit Insh.  |
| `r`        | Refresh.    |

## Input Bar Commands
Several components use the input bar which has the following commands:
| Command       | Description                                |
|---------------|--------------------------------------------|
| `<Ctrl>-q`    | Exit the input bar.                        |
| Any character | Append the character to the current input. |
| `<Enter>`     | Search for files matching the input.       |
| `<Backspace>` | Remove the last character from the input.  |
| `<Tab>`       | Fill in the input with the suggestion.     |

## Directory Bar Commands
Several components use the directory bar. From these components `Ctrl-d` focuses on the directory
bar. The directory bar has the following commands:
| Command       | Description                                |
|---------------|--------------------------------------------|
| `<Ctrl>-q`    | Exit the directory bar.                    |
| Any character | Append the character to the current input. |
| `<Backspace>` | Remove the last character from the input.  |
| `<Ctrl>-w`    | Remove the last part from the input.       |
| `<Tab>`       | Fill in the input with the suggestion.     |
| `<Enter>`     | Use the directory and return focus         |
