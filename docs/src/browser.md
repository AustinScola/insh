# Browser

The file browser shows the current directory at the top, and lists the entries of the directory
below. The currently selected entry is highlighed in yellow.

| Command              | Description                                                                                                                                                                        |
|----------------------|------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------|
| `j` \| `<Down>`      | Move the selection down by one entry.                                                                                                                                              |
| `k` \| `<Up>`        | Move the selection up by one entry.                                                                                                                                                |
| `J` \| `<End>`       | Move the selection to the last entry.                                                                                                                                              |
| `K` \| `<Home>`      | Move the selection to the first entry.                                                                                                                                             |
| `l` \| `<Enter>`     | If the currently selected entry is a file, then open it in vim. Else, if the currently selected entry is a directory, then change the current directory to the selected directory. |
| `h` \| `<Backspace>` | Change directories to the parent of the current directory (if the current directory is not the root directory).                                                                    |
| `b`                  | Run bash with the working directory set to the current directory.                                                                                                                  |
| `c`                  | Open the file creator for creating a file.                                                                                                                                         |
| `C`                  | Open the file creator for creating a directory.                                                                                                                                    |
| `f`                  | Open the file finder.                                                                                                                                                              |
| `s`                  | Open the file contents searcher.                                                                                                                                                   |
| `m`                  | Toggle showing the metadata of the entries.                                                                                                                                        |
| `yE`                 | Yank the selected entry filename to the clipboard.                                                                                                                                 |
| `yy`                 | Yank the selected entry file path to the clipboard.                                                                                                                                |
| `Y`                  | Yank the selected entry file contents to the clipboard.                                                                                                                            |
