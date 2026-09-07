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
| `<Ctrl>-q`    | Exit the file finder                       |
| Any character | Append the character to the current input. |
| `<Enter>`     | Search for files matching the input.       |
| `<Backspace>` | Remove the last character from the input.  |
| `<Tab>`       | Fill in the input with the suggestion.     |
