# Multiplexing

Insh has terminal multplexing inspired heavily by tmux. The session, window, and pane concepts
are all borrowed. As is the usage of a prefix.

## Normal mode
| Command                            | Status bar command    | Description                                       |
|------------------------------------|-----------------------|---------------------------------------------------|
| `<Ctrl-b %`                        | split-vertically      | Split the current pane vertically.                |
| `<Ctrl-b "`                        | split-horizontally    | Split the current pane horizontally.              |
| `<Ctrl>-b x`                       | kill-pane             | Kill the current pane.                            |
| `<Ctrl>-b j` \| `<Ctrl>-b <Down>`  | focus-down            | Select the pane below the current pane.           |
| `<Ctrl>-b k` \| `<Ctrl>-b <Up>`    | focus-up              | Select the pane above the current pane.           |
| `<Ctrl>-b h` \| `<Ctrl>-b <Left>`  | focus-left            | Select the pane to the left of the current pane.  |
| `<Ctrl>-b l` \| `<Ctrl>-b <Right>` | focus-right           | Select the pane to the right of the current pane. |
| `<Ctrl>-b <Ctrl>-<Down>`           | resize-pane-down      | Resize the pane downwards.                        |
| `<Ctrl>-b <Ctrl>-<Up>`             | resize-pane-up        | Resize the pane upwards.                          |
| `<Ctrl>-b <Ctrl>-<Left>`           | resize-pane-left      | Resize the pane left.                             |
| `<Ctrl>-b <Ctrl>-<Right>`          | resize-pane-right     | Resize the pane right.                            |
| `<Ctrl>-b y`                       | synchronize-panes     | Synchronize panes.                                |
| `<Ctrl-b z`                        | toggle-zoom           | Toggle zooming the current pane.                  |
| `<Ctrl>-b {`                       | switch-pane-previous  | Switch the current pane with the previous pane.   |
| `<Ctrl>-b }`                       | switch-pane-next      | Switch the current pane with the next pane.       |
| `<Ctrl-b $`                        | rename-session [name] | Rename the current session.                       |
| `<Ctrl-b ,`                        | rename-window [name]  | Rename the current window.                        |
|                                    | new-session [name]    | Create a new session.                             |
| `<Ctrl>-b c`                       | new-window [name]     | Create a new window.                              |
| `<Ctrl-b [`                        | change-mode copy      | Enter copy mode                                   |
| `<Ctrl>-b s`                       | list-sessions \| ls   | View the session, window, and pane switcher.      |
| `<Ctrl>-b :`                       | change-mode prompt    | Prompt for a command.                             |
|                                    | detach                | Detach from the current session.                  |

## Copy mode
| Command            | Description                                     |
|--------------------|-------------------------------------------------|
| `h` \| `<Left>`    | Move the cursor left.                           |
| `j` \| `<Down>`    | Move the cursor down.                           |
| `k` \| `<Up>`      | Move the cursor up.                             |
| `l` \| `<Right>`   | Move the cursor right.                          |
| `0` \| `<Home>`    | Move the cursor to the start of the line.       |
| `$` \| `<End>`     | Move the cursor to the end of the line.         |
| `gg`               | Move the cursor to the start of the buffer.     |
| `G`                | Move the cursor to the end of the buffer.       |
| `v` \| `<Space>`   | Start selecting at the current cursor position. |
| `y` \| `<Enter>`   | Copy the selection to the clipboard.            |
| `<Escape>`         | Unselect.                                       |
| `q`                | Exit out of copy mode.                          |
