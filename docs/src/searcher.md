# Searcher

The file contents searcher searches all files in the current directory recursively for a given input
string.

The searcher displays the directory at the top, then an input bar, and then the hits. For each hit,
the file name is displayed, then a line for each occurrence of the string with the line number.

The commands for the input bar are the same as those for the Finder.

## Searcher Contents Commands
| Command          | Description                                                                                                                                                                                        |
|------------------|----------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------|
| `<Ctrl>-q`       | Return focus to the input bar.                                                                                                                                                                     |
| `j` \| `<Down>`  | Move the selection down.                                                                                                                                                                           |
| `k` \| `<Up>`    | Move the selection up.                                                                                                                                                                             |
| `J` \| `<End>`   | Move the selection to the last file hit.                                                                                                                                                           |
| `K` \| `<Home>`  | Move the selection to the first file hit.                                                                                                                                                          |
| `<Ctrl>-j`       | Move the view down.                                                                                                                                                                                |
| `<Ctrl>-k`       | Move the view up.                                                                                                                                                                                  |
| `l` \| `<Enter>` | Open the hit in vim. If the file path of a hit is selected, then open vim at the start of the file. Else, if an occurrence of the string is selected, then open vim at the line of the occurrence. |
| `g`              | Go to the hit in the file browser.                                                                                                                                                                 |
| `G`              | Go to the hit in the file browser and select it.                                                                                                                                                   |
| `yE`             | Yank the hit to the end. If the file path of a hit is selected, yank the relative file path. Else, if an occurrence of the string is selected, yank that line.                                      |
| `yy`             | Yank the hit line. If the file path of a hit is selected, yank the full file path. Else, if an occurrence of the string is selected, yank that line.                                                |
| `Y`              | Yank the hit file contents to the clipboard.                                                                                                                                                       |
