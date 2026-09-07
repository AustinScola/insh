# Finder

The file finder shows the directory at the top, then an input bar, then the found files. The finder
starts out with the input bar focused. The input can be any valid regular expression.

The finder finds files in the current directory (recursively) for which the regex pattern matches
the file name.

## Found Files Commands:
| Command          | Description                                                                |
|------------------|----------------------------------------------------------------------------|
| `<Ctrl>-q`       | Return focus to the input bar.                                             |
| `j` \| `<Down>`  | Move the selection down by one hit.                                        |
| `k` \| `<Up>`    | Move the selection up by one hit.                                          |
| `J` \| `<End>`   | Move the selection to the last hit.                                        |
| `K` \| `<Home>`  | Move the selection to the first hit.                                       |
| `l` \| `<Enter>` | Open the hit in vim.                                                       |
| `g`              | Go to the hit in the file browser.                                         |
| `G`              | Go to the hit in the file browser and select it.                           |
| `yE`             | Yank the hit filename to the clipboard.                                    |
| `yy`             | Yank the hit file path to the clipboard.                                   |
| `Y`              | Yank the hit file contents to the clipboard.                               |
