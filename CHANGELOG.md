# 0.4.4
- Add creating a directory with `C` from the file browser.

# 0.4.3
- Fix exiting bash on MacOS.

# 0.4.2
- Fix compilation on MacOS.

# 0.4.1
- Add more inshd commands:
    - `-f|--force` for `start`/`stop`
    - `restart`

# 0.4.0
- Add a daemon for performing tasks. 

# 0.3.22
- Add the ability to create a file with the the `c` command from the file browser.

# 0.3.21
- Fix the build for latest stable Rust.

# 0.3.20
- Add the yank (`y`) and really yank (`Y`) commands to the file browser.

# 0.3.19
- Fix restoring the terminal screen after running vim by wrapping vim's stdout and stripping the
ANSI escape codes for enabling and disabling the alternative screen.

# 0.3.18
- Add making of the bell sound for invalid commands. This can be turned off via the configuration
setting `general.bell`.

# 0.3.17
- Fix yanking on Ubuntu 22.04.

# 0.3.16
- Add a better message to the browser for when the directory does not exist.

# 0.3.15
- Fix the displaying of tabs in the Searcher contents and add a configuration option
`general.tab_width` to enable changing the width that is used for tabs.

# 0.3.14
- Add logging as a complile feature.

# 0.3.13
- Fix the crash in the browser when permission is denied or the directory cannot be read for some other
reason.

# 0.3.12
- Add documentation of the configuration options.
- Fix the file used for configuration.

# 0.3.11
- Add searcher history.
- Add searcher auto complete and `<Tab>` completion.
- Add a configuration file.

# 0.3.10
- Add a message for an empty directory in the browser.
- Fix crash when the width of the terminal is less than the length of the messages for no hits in the
searcher and finder.

# 0.3.9
- Bump the `clap` dependency from version `3.1.14` to version `3.2.17`.
- Add an `edit` subcommand.

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
