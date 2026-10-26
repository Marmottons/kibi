<!--
SPDX-FileCopyrightText: 2020 Ilaï Deutel & Kibi Contributors
SPDX-License-Identifier: MIT or Apache-2.0
-->

<div align="center">
    <!-- markdownlint-disable-next-line line_length -->
    <img src="./assets/kibi.svg" alt="Logo of Kibi. A dark gray square with rounded corners contains a large, light-orange, backward-pointing chevron and a bright green, vertical rectangle. To the left of the chevron is a column of ten light-orange, horizontal rectangles. These ten rectangles function as a 10-bit binary counter, with the bottom bar being the least significant bit and the top the most significant. The bars blink in a sequence that counts from 0 to 1023 in binary." width="256" />
</div>

# Kibi: A small text editor, written in Rust

A configurable text editor with UTF-8 support, incremental search, syntax
highlighting, line numbers and more, written in Rust with minimal dependencies.

Kibi is compatible with Linux, macOS, Windows
10[^1], and WASI.

[^1]: Kibi requires the terminal to support ANSI escape sequences. Windows 10 version 1703 ([Creators Update](https://devblogs.microsoft.com/commandline/windows-10-creators-update-whats-new-in-bashwsl-windows-console), April 2017) and above are supported.

This project is inspired by [`kilo`](https://github.com/antirez/kilo), a text
editor written in C.

## Table of contents

- [Installation](#installation)
- [Usage](#usage)
  - [Keyboard shortcuts](#keyboard-shortcuts)
  - [Configuration](#configuration)
    - [Global configuration](#global-configuration)
    - [Syntax highlighting](#syntax-highlighting)
- [License](#license)

## Installation

### Build from source

```bash
git clone https://github.com/Marmottons/kibi.git
cd kibi
cargo build --release
```

The binary will be at `target/release/kibi`.

Install the binary **and** the syntax highlighting files (`syntax.d`) with:

```bash
./install.sh
```

This runs `cargo install --path . --locked` and copies the files from
[`syntax.d`](syntax.d) to `$XDG_DATA_HOME/kibi/syntax.d`
(`~/.local/share/kibi/syntax.d` by default). Run it again after each update.

The files in [`syntax.d`](syntax.d) include hand-written syntaxes and syntaxes
generated from [bat](https://github.com/sharkdp/bat)'s (see
[`scripts/import_bat_syntaxes.py`](scripts/import_bat_syntaxes.py)), which start
with a `# Generated from the bat syntaxes` line. The keyword colors of the
hand-written files are also taken from bat. Kibi's highlighting stays much
simpler than bat's, so colors only match bat's on the most common elements.

Syntax highlighting configuration files can also be placed manually in one of
the directories mentioned in the
[Configuration/Syntax Highlighting](#syntax-highlighting) section.

## Usage

```bash
kibi              # Start a new text buffer
kibi <file path>  # Open a file
kibi --version    # Print version information and exit
```

### Keyboard shortcuts

| Keyboard shortcut              | Description                                                   |
| ------------------------------ | ------------------------------------------------------------- |
| <kbd>Ctrl</kbd> + <kbd>F</kbd> | Incremental search; use arrows to navigate                    |
| <kbd>Ctrl</kbd> + <kbd>S</kbd> | Save the buffer to the current file, or specify the file path |
| <kbd>Ctrl</kbd> + <kbd>G</kbd> | Go to `<line number>[:<column number>]` position              |
| <kbd>Ctrl</kbd> + <kbd>Q</kbd> | Quit; with unsaved changes, confirm with <kbd>y</kbd> or <kbd>Ctrl</kbd> + <kbd>Q</kbd> again |
| <kbd>Ctrl</kbd> + <kbd>D</kbd> | Duplicate the current row                                     |
| <kbd>Ctrl</kbd> + <kbd>E</kbd> | Toggle line numbers                                  |
| <kbd>Ctrl</kbd> + <kbd>R</kbd> | Remove an entire line                                         |
| <kbd>Ctrl</kbd> + <kbd>C</kbd> | Copies the text after the cursor, or the whole line if the cursor is at its end |
| <kbd>Ctrl</kbd> + <kbd>X</kbd> | Cuts the text after the cursor, or the whole line if the cursor is at its end   |
| <kbd>Ctrl</kbd> + <kbd>V</kbd> | Pastes the copied text at the cursor position                   |
| <kbd>Ctrl</kbd> + <kbd>U</kbd> | Deletes the text before the cursor                              |
| <kbd>Ctrl</kbd> + <kbd>K</kbd> | Deletes the text after the cursor                               |
| <kbd>Ctrl</kbd> + <kbd>Z</kbd> | Undoes the last shortcut action (single level)                 |
| <kbd>Ctrl</kbd> + <kbd>←</kbd> | Moves cursor to previous word                                 |
| <kbd>Ctrl</kbd> + <kbd>→</kbd> | Moves cursor to next word                                     |
| <kbd>Ctrl</kbd> + <kbd>/</kbd> | Comment or uncomment the current line                         |

### Configuration

#### Global configuration

Kibi can be configured using a configuration file. It must follow this format:

```ini
# The size of a tab. Must be > 0.
tab_stop=4
# The duration for which messages are shown in the status bar, in seconds.
message_duration=3
# Whether to show line numbers.
show_line_numbers=true
```

The location of these files is described below.

##### Linux / macOS

kibi follows the [XDG Base Directory Specification](https://specifications.freedesktop.org/basedir-spec/basedir-spec-latest.html):

- A user-level configuration file can be located at
  `$XDG_CONFIG_HOME/kibi/config.ini` if environment variable `$XDG_CONFIG_HOME`
  is defined, `~/.config/kibi/config.ini` otherwise.
- A system-wide configuration file can be located at
  `$XDG_CONFIG_DIRS/kibi/config.ini` if environment variable
  `$XDG_CONFIG_DIRS` is defined, `/etc/kibi/config.ini` or
  `/etc/xdg/kibi/config.ini` otherwise.

##### Windows

A configuration file can be located at `%APPDATA%\Kibi\config.ini`.

#### Syntax highlighting

Syntax highlighting can be configured using INI files which follow this format:

```ini
### ~/.local/share/kibi/syntax.d/rust.ini ###
# Kibi syntax highlighting configuration for Rust (shortened)

name=Rust
extensions=rs
highlight_numbers=true
singleline_string_quotes="
singleline_comment_start=//
multiline_comment_delims=/*, */
; In Rust, the multi-line string delimiter is the same as the single-line string delimiter
multiline_string_delim="
highlight_function_calls=true
plain_path_calls=true
highlight_operators=true
highlight_punctuation=true
attributes=true
type_definition_keywords=enum, impl, struct, trait, type
function_definition_keywords=fn
highlight_parameters=true
keywords_1=async, await, break, const, continue, else, enum, fn, for, if, impl, let, match, mut, pub, return, struct, use, while
builtins=false, true
types=Box, Option, Result, Some, None, Ok, Err, String, Vec
operators=as, in
```

Colors follow the Catppuccin Mocha theme, as used by [bat](https://github.com/sharkdp/bat)
(24-bit colors). Besides the keys above, these keys are supported:

| Key | Effect |
|-----|--------|
| `keywords_1` | Keywords (mauve) |
| `keywords_2` | Constants (peach) |
| `builtins` | Built-ins, e.g. `true` in Rust (red) |
| `types` | Types (yellow) |
| `functions` | Functions and commands (blue) |
| `operators` | Word operators (teal) |
| `highlight_function_calls` | `foo(` is highlighted as a function |
| `plain_path_calls` | ...except after `::`, e.g. `Vec::new(` |
| `plain_capitalized_calls` | ...except for `Capitalized` names, e.g. `Some(` |
| `highlight_operators` | `+`, `=`, `.`, `::`... are highlighted as operators |
| `highlight_punctuation` | `(`, `;`, `,`... are highlighted as punctuation |
| `highlight_capitalized` | `Capitalized` words are highlighted as types |
| `highlight_commands` | The first word of a shell command is highlighted as a function |
| `type_definition_keywords` | The word after one of these keywords (e.g. `struct`) is a type |
| `function_definition_keywords` | The word after one of these keywords (e.g. `fn`) is a function |
| `highlight_parameters` | Parameters of function definitions, e.g. `a` in `def f(a)` (maroon) |
| `highlight_variables` | `$name` variables, also in double-quoted strings |
| `highlight_options` | Command options, e.g. `-l` or `--all` |
| `negative_numbers` | `-1` is a number |
| `docstrings` | Multi-line strings that start a line are comments |
| `decorators` | `@name` at the start of a line is a decorator |
| `attributes` | `#[...]` at the start of a line is an attribute |
| `keys`, `key_separators`, `key_separator` | Keys of configuration files, e.g. `keys=functions` and `key_separators=:` for `name: value` |
| `unquoted_values` | Unquoted values and list items are strings, e.g. in YAML |
| `sections` | Highlight type of section names, e.g. `[package]` in TOML |
| `highlight_markdown` | Markdown headings, lists, quotes, tables, emphasis, links and images; code blocks use the syntax of their language (e.g. ```` ```rust ````) |
| `highlight_tags` | HTML/XML tags, attributes and entities |
| `highlight_css` / `highlight_units` | CSS selectors and at-rules / units, e.g. `px` |

The location of these files is described below.

##### Linux / macOS

kibi follows the [XDG Base Directory Specification](https://specifications.freedesktop.org/basedir-spec/basedir-spec-latest.html):

- User-level syntax highlighting configuration files can be located at `$XDG_DATA_HOME/kibi/syntax.d/<file_name>.ini`
  if environment variable `$XDG_DATA_HOME` is defined,
  `~/.local/share/kibi/syntax.d/<file_name>.ini` otherwise.
- System-wide syntax highlighting configuration files can be located at `$XDG_DATA_DIRS/kibi/syntax.d/<file_name>.ini`
  if environment variable `$XDG_DATA_DIRS` is defined,
  `/usr/local/share/kibi/syntax.d/<file_name>.ini` or
  `/usr/share/kibi/syntax.d/<file_name>.ini` otherwise.

##### Windows

Syntax highlighting configuration files can be located at `%APPDATA%\Kibi\syntax.d\<file_name>.ini`.

## License

This project is licensed under either of

- [Apache License, Version 2.0](LICENSE-APACHE)
- [MIT License](LICENSE-MIT)

at your option.
