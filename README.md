# Insh

A graphical, interactive, terminal environment.

## Installation
```
cargo install --git https://github.com/AustinScola/insh --tag latest
```

## Help

By default, Insh starts out in file browser. Insh has two other main components as well: a file
finder and a file contents searcher.

Some commands work from all components:
| Command    | Description |
|------------|-------------|
| `<Ctrl>-x` | Exit Insh.  |
| r          | Refresh.    |

### Browser Help

The file browser shows the current directory at the top, and lists the entries of the directory
below. The currently selected entry is highlighed in yellow.

| Command              | Description                                                                                                                                                                        |
|----------------------|------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------|
| `j`                  | Move the selection down by one entry.                                                                                                                                              |
| `k`                  | Move the selection up by one entry.                                                                                                                                                |
| `J`                  | Move the selection to the last entry.                                                                                                                                              |
| `K`                  | Move the selection to the first entry.                                                                                                                                             |
| `l` \| `<Enter>`     | If the currently selected entry is a file, then open it in vim. Else, if the currently selected entry is a directory, then change the current directory to the selected directory. |
| `h` \| `<Backspace>` | Change directories to the parent of the current directory (if the current directory is not the root directory).                                                                    |
| `b`                  | Run bash with the working directory set to the current directory.                                                                                                                  |
| `f`                  | Open the file finder.                                                                                                                                                              |
| `s`                  | Open the file contents searcher.

### Finder Help

The file finder shows the directory at the top, then an input bar, then the found files. The finder
starts out with the input bar focused. The input can be any valid regular expression.

The finder finds files in the current directory (recursively) for which the regex pattern matches
the file name.

#### Input Bar Commands
| Command       | Description                                |
|---------------|--------------------------------------------|
| `<Ctrl>-q`    | Exit the file finder                       |
| Any character | Append the character to the current input. |
| `<Enter>`     | Search for files matching the input.       |
| `<Backspace>` | Remove the last character from the input.  |
| `<Tab>`       | Fill in the input with the suggestion.     |

#### Found Files Commands:
| Command          | Description                                                                |
|------------------|----------------------------------------------------------------------------|
| `<Ctrl>-q`       | Return focus to the input bar.                                             |
| `j`              | Move the selection down by one hit.                                        |
| `k`              | Move the selection up by one hit.                                          |
| `J`              | Move the selection to the last hit.                                        |
| `K`              | Move the selection to the first hit.                                       |
| `l` \| `<Enter>` | Open the hit in vim.                                                       |
| `g`              | Go to the hit in the file browser.                                         |
| `G`              | Go to the hit in the file browser and select it.                           |
| `y`              | Yank the hit. (Copy the path of the hit to the clipboard.)                 |
| `Y`              | Really yank the hit. (Copy the aboslute path of the hit to the clipboard.) |

### Searcher Help

The file contents searcher searches all files in the current directory recursively for a given input
string.

The searcher displays the directory at the top, then an input bar, and then the hits. For each hit,
the file name is displayed, then a line for each occurance of the string with the line number.

The commands for the input bar are the same as those for the Finder.

#### Searcher Contents Commands
| Command          | Description                                                                                                                                                                                        |
|------------------|----------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------|
| `<Ctrl>-q`       | Return focus to the input bar.                                                                                                                                                                     |
| `j`              | Move the selection down.                                                                                                                                                                           |
| `k`              | Move the selection up.                                                                                                                                                                             |
| `J`              | Move the selection to the last file hit.                                                                                                                                                           |
| `K`              | Move the selection to the first file hit.                                                                                                                                                          |
| `<Ctrl>-j`       | Move the view down.                                                                                                                                                                                |
| `<Ctrl>-k`       | Move the view up.                                                                                                                                                                                  |
| `l` \| `<Enter>` | Open the hit in vim. If the file path of a hit is selected, then open vim at the start of the file. Else, if an occurrence of the string is selected, then open vim at the line of the occurrence. |
| `g`              | Go to the hit in the file browser.                                                                                                                                                                 |
| `G`              | Go to the hit in the file browser and select it.                                                                                                                                                   |
| `y`              | Yank the hit. If the file path of a hit is selected, yank the file path. Else, if an occurence of the string is selected, yank that line.                                                          |
| `Y`              | Really yank the hit. If the file path of a hit is selected, yank the **absolute** file path. Else, if an occurence of the string is selected, yank that line.                                      |


## Configuration

Insh can be configured by the file `~/.insh-config.yaml`.

### Options
`searcher.history.length` (int): The number of searches to store.
