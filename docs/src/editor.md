# Editor

The editor is a modal editor heavily inspired by vim.

# Modes
| Mode         | Description                 |
|--------------|-----------------------------|
| Normal       | Navigate.                   |
| Command      | Use commands.               |
| Insert       | Insert text.                |
| Visual       | Visually select characters. |
| Visual Line  | Visually select lines.      |
| Visual Block | Visually select blocks.     |
| Search       | Search for text.            |

# Common
| Command    | Description  |
|------------|--------------|
| `<Ctrl>-e` | Scroll down. |
| `<Ctrl>-y` | Scroll up.   |

# Normal Mode
| Command                | Description                                          |
|------------------------|------------------------------------------------------|
| `[modifier]<Movement>` | Move the cursor.                                     |
| `:`                    | Enter command mode.                                  |
| `i`                    | Enter insert mode.                                   |
| `a`                    | Insert after the cursor.                             |
| `A`                    | Insert at the end of the line.                       |
| `o`                    | Open a line after the cursor and enter insert mode.  |
| `O`                    | Open a line before the cursor and enter insert mode. |
| `v`                    | Enter visual mode.                                   |
| `V`                    | Enter visual line mode.                              |
| `<Ctrl>-v`             | Enter visual block mode.                             |
| ` `                    | Insert a space.                                      |
| `y<movement>`          | Yank.                                                |
| `yy`                   | Yank the line.                                       |
| `p`                    | Paste after.                                         |
| `P`                    | Paste before.                                        |
| `x`                    | Delete a character.                                  |
| `d<movement>`          | Delete the movement.                                 |
| `dd`                   | Delete the line.                                     |
| `r`                    | Replace a character.                                 |
| `u`                    | Undo.                                                |
| `<Ctrl>-r`             | Redo.                                                |
| `/`                    | Enter search mode.                                   |
| `n`                    | Go to next search hit.                               |
| `N`                    | Go to previous search hit.                           |

# Command Mode
| Command       | Description                                   |
|---------------|-----------------------------------------------|
| `<Esc>`       | Cancel the command and enter normal mode.     |
| `<Printable>` | Add the printable character to the command.   |
| `<Enter>`     | Run the command and enter normal mode.        |
| `<Backspace>` | Backspace.                                    |

# Commands
| Command | Description                                                   |
|---------|---------------------------------------------------------------|
| `w`     | Write the file.                                               |
| `q`     | Quit (refuse if there are unwritten changes).                 |
| `q!`    | Quit (even if there are unwritten changes).                   |
| `wq`    | Write and quit.                                               |
| `vim`   | Open the file in vim (refuse if there are unwritten changes). |
| `vim!`  | Open the file in vim (even if there are unwritten changes).   |

# Insert Mode
| Command       | Description                     |
|---------------|---------------------------------|
| `<Esc>`       | Enter normal mode.              |
| `<Printable>` | Insert the printable character. |
| `<Tab>`       | Enter 4 spaces.                 |
| `<Enter>`     | Enter a newline character.      |
| `<Backspace>` | Backspace.                      |
| `<Paste>`     | Insert the clipboard.           |

# Common Visual

Visual modes share many of the same commands.

| Command                | Description                                  |
|------------------------|----------------------------------------------|
| `<Esc>`                | Enter normal mode.                           |
| `[modifier]<Movement>` | Move the cursor.                             |
| `y`                    | Yank.                                        |
| `Y`                    | Yank to clipboard.                           |
| `x` \| `d`             | Delete the selection.                        |
| `u`                    | Lowercase the selection.                     |
| `U`                    | Uppercase the selection.                     |
| `r`                    | Replace the selection.                       |
| `p`                    | Replace the selection with the paste buffer. |

# Visual Mode
| Command    | Description              |
|------------|--------------------------|
| `V`        | Enter visual line mode.  |
| `<Ctrl>-v` | Enter visual block mode. |

# Visual Line Mode
| Command    | Description              |
|------------|--------------------------|
| `v`        | Enter visual mode.       |
| `<Ctrl>-v` | Enter visual block mode. |

# Visual Block Mode
| Command | Description              |
|---------|--------------------------|
| `v`     | Enter visual mode.       |
| `V`     | Enter visual line mode.  |

# Search Mode
| Command       | Description                                |
|---------------|--------------------------------------------|
| `<Esc>`       | Cancel the search and enter normal mode.   |
| `<Enter>`     | Go to the next hit.                        |
| `<Printable>` | Add the printable character to the search. |
| `<Backspace>` | Backspace.                                 |

# Modifiers
| Modifier | Description                                       |
|----------|---------------------------------------------------|
| `<Num>`  | Perform the action the specified number of times. |

# Movements
| Movement    | Description                                              |
|-------------|----------------------------------------------------------|
| `h`         | Left.                                                    |
| `j`         | Down.                                                    |
| `k`         | Up.                                                      |
| `l`         | Right.                                                   |
| `w`         | Start of next word.                                      |
| `W`         | Start of next WORD.                                      |
| `b`         | Start of previous word.                                  |
| `B`         | Start of previous WORD.                                  |
| `e`         | End of current/next word.                                |
| `E`         | End of current/next WORD.                                |
| `0`         | Beginning of line.                                       |
| `$`         | End of line.                                             |
| `J` \| `G`  | End of file or line number if a number modifier is used. |
| `K` \| `gg` | Beginning of file.                                       |
