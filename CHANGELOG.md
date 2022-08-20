# 0.3.8
- Fix bugs related to running with `-d` and a trailing slash in the directory argument.

# 0.3.7
- Add the goto and really goto (`g` and `G`) commands to the searcher.
- Add the yank and really yank (`y` and `Y`) commands to the finder.
- Add refresh (`r`) command to the searcher and finder.
- Add scroll to bottom (`J`) and scroll to top (`K`) commands to the browser, finder, and searcher.
- Add scroll view down (`<Ctrl>-j`) and scroll view up (`<Ctrl>-k`) to searcher.

# 0.3.6
- Add going to files in the browser and selecting them from the finder with `G`.

# 0.3.5
- Add the yank and really yank commands (`y` and `Y`) to the searcher.

# 0.3.4
- Fix the displaying of empty directories in the file browser.

# 0.3.3
- Add installation instructions to the README.
- Change the color of dot files (and directories) to light gray in the browser.
- Add command line interface.
- Fix the flickering of text.

# 0.3.2
- Change to using the `PWD` environment variable to determine the current directory if set. This
means that symbolic links are no longer resolved.

# 0.3.1
- Add help information to the README.
- Fix highlighting of searcher contents when unfocussed.
- Change the finder to show relative paths in addition to file names.

# 0.3.0
- Fix crashing of finder from there being too many files open.
- Remove the current directory from the finder contents.
- Fix the displayed directory in the finder.
- Fix the directory that the goto command (`g`) in the finder navigates to.
- Fix resetting the finder contents scroll position on successive runs.
- Change the finder to only display files.
- Add back the searcher component.

# 0.2.2
- Use `~` to represent the home directory.
- Change the directory to end with a trailing path separator.

# 0.2.1
- Add back refreshing the contents of the file browser with `r`.
