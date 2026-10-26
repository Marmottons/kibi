// SPDX-FileCopyrightText: 2020 Ilaï Deutel & Kibi Contributors
//
// SPDX-License-Identifier: MIT OR Apache-2.0

use std::fmt::{self, Display, Formatter};
use std::path::{Path, PathBuf};

use crate::config::{self, parse_value as pv, parse_values as pvs};

/// Type of syntax highlighting for a single rendered character.
///
/// Each `HLType` is associated with a 24-bit color from the Catppuccin Mocha
/// theme, as used by bat (<https://github.com/catppuccin/bat>).
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum HlType {
    Normal,      // Text
    Number,      // Peach
    Match,       // Find highlight (background)
    String,      // Green
    MlString,    // Green
    Escape,      // Pink
    Comment,     // Overlay 2
    MlComment,   // Overlay 2
    Keyword1,    // Mauve
    Keyword2,    // Peach (constants)
    Builtin,     // Red
    Type,        // Yellow
    Function,    // Blue
    Operator,    // Teal
    Punctuation, // Overlay 2
    Parameter,   // Maroon
}

impl Display for HlType {
    /// Write the ANSI color escape sequence for the `HLType` using the given
    /// formatter.
    fn fmt(&self, f: &mut Formatter) -> fmt::Result {
        let (layer, (r, g, b)) = match self {
            Self::Normal => (38, (0xcd, 0xd6, 0xf4)),
            Self::Number | Self::Keyword2 => (38, (0xfa, 0xb3, 0x87)),
            Self::Match => (48, (0x3e, 0x57, 0x67)),
            Self::String | Self::MlString => (38, (0xa6, 0xe3, 0xa1)),
            Self::Escape => (38, (0xf5, 0xc2, 0xe7)),
            Self::Comment | Self::MlComment | Self::Punctuation => (38, (0x93, 0x99, 0xb2)),
            Self::Keyword1 => (38, (0xcb, 0xa6, 0xf7)),
            Self::Builtin => (38, (0xf3, 0x8b, 0xa8)),
            Self::Type => (38, (0xf9, 0xe2, 0xaf)),
            Self::Function => (38, (0x89, 0xb4, 0xfa)),
            Self::Operator => (38, (0x94, 0xe2, 0xd5)),
            Self::Parameter => (38, (0xeb, 0xa0, 0xac)),
        };
        write!(f, "\x1b[{layer};2;{r};{g};{b}m")
    }
}

/// Configuration for syntax highlighting.
#[expect(clippy::struct_excessive_bools, reason = "Independent options from the INI file")]
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct Conf {
    /// The name of the language, e.g. "Rust".
    pub name: String,
    /// Whether to highlight numbers.
    pub highlight_numbers: bool,
    /// Quotes for single-line strings.
    pub sl_string_quotes: Vec<char>,
    /// The tokens that starts a single-line comment, e.g. "//".
    pub sl_comment_start: Vec<String>,
    /// The tokens that start and end a multi-line comment, e.g. ("/*", "*/").
    pub ml_comment_delims: Option<(String, String)>,
    /// The token that start and end a multi-line strings, e.g. "\"\"\"" for
    /// Python.
    pub ml_string_delim: Option<String>,
    /// Keywords to highlight and there corresponding `HLType` (typically
    /// `HLType::Keyword1` or `HLType::Keyword2`)
    pub keywords: Vec<(HlType, Vec<String>)>,
    /// Whether to highlight identifiers followed by `(` as functions.
    pub highlight_function_calls: bool,
    /// Whether function calls after `::` (e.g. `Vec::new()`) stay plain.
    pub plain_path_calls: bool,
    /// Whether function calls of `Capitalized` names (e.g. `Some(x)`) stay plain.
    pub plain_capitalized_calls: bool,
    /// Whether to highlight operators, e.g. `+` or `=`.
    pub highlight_operators: bool,
    /// Whether to highlight punctuation, e.g. `(` or `;`.
    pub highlight_punctuation: bool,
    /// Whether to highlight `Capitalized` identifiers as types.
    pub highlight_capitalized: bool,
    /// Whether to highlight the first word of a shell command as a function.
    pub highlight_commands: bool,
    /// Keywords after which the next word is highlighted as a type, e.g. `struct`.
    pub type_def_keywords: Vec<String>,
    /// Keywords after which the next word is highlighted as a function, e.g. `fn`.
    pub function_def_keywords: Vec<String>,
    /// Whether to highlight the parameters of function definitions.
    pub highlight_parameters: bool,
    /// Whether to highlight `$name` variables, also in double-quoted strings.
    pub highlight_variables: bool,
    /// Whether to highlight command options, e.g. `-l` or `--all`.
    pub highlight_options: bool,
    /// Whether `-1` is a number (and not an operator followed by a number).
    pub negative_numbers: bool,
    /// Whether multi-line strings that start a line are docstrings (comments).
    pub docstrings: bool,
    /// Whether `@name` at the start of a line is a decorator.
    pub decorators: bool,
    /// Whether `#[...]` at the start of a line is an attribute.
    pub attributes: bool,
    /// Highlight type of keys, e.g. `name` in `name: value` (JSON, YAML, TOML...).
    pub keys: Option<HlType>,
    /// Characters between a key and its value, e.g. `:` or `=`.
    pub key_separators: Vec<char>,
    /// Highlight type of the key separators.
    pub key_separator: Option<HlType>,
    /// Whether unquoted values and list items are strings, e.g. in YAML.
    pub unquoted_values: bool,
    /// Highlight type of section names, e.g. `package` in `[package]`.
    pub sections: Option<HlType>,
    /// Whether to highlight Markdown (headings, lists, emphasis, links, code).
    pub highlight_markdown: bool,
    /// Whether to highlight HTML/XML tags and attributes.
    pub highlight_tags: bool,
    /// Whether to highlight CSS selectors and at-rules.
    pub highlight_css: bool,
    /// Whether to highlight CSS units, e.g. `px` in `4px`.
    pub highlight_units: bool,
    /// Syntaxes (and their extensions) of the code blocks of Markdown files.
    pub embedded: Vec<(Vec<String>, Self)>,
}

/// Parse a highlight type from the name of the corresponding keyword list.
fn parse_hl_type(name: &str) -> Result<HlType, String> {
    Ok(match name {
        "keywords_1" => HlType::Keyword1,
        "keywords_2" => HlType::Keyword2,
        "builtins" => HlType::Builtin,
        "types" => HlType::Type,
        "functions" => HlType::Function,
        "operators" => HlType::Operator,
        "punctuation" => HlType::Punctuation,
        "strings" => HlType::String,
        "parameters" => HlType::Parameter,
        _ => return Err(format!("Unknown highlight type: {name}")),
    })
}

impl Conf {
    /// Return the syntax configuration corresponding to the given file
    /// name, if a matching INI file is found in a config directory.
    /// If no matching configuration is found, return the default.
    ///
    /// An entry in `extensions` matches if it is the whole file name (e.g.
    /// `Makefile`) or if the file name ends with it, preceded by a dot (e.g.
    /// `rs` or `bashrc`). In a given directory, the longest match wins.
    pub fn find(name: &str, data_dirs: &[String]) -> Self {
        let base_name = Path::new(name).file_name().map_or(name, |n| n.to_str().unwrap_or(name));
        let match_len = |ext: &String| {
            let matches = base_name == ext
                || base_name.strip_suffix(ext.as_str()).is_some_and(|rest| rest.ends_with('.'));
            matches.then_some(ext.len())
        };
        for data_dir in data_dirs {
            match PathBuf::from(data_dir).join("syntax.d").read_dir() {
                Ok(dir_entries) => {
                    let mut best: Option<(usize, Self)> = None;
                    let mut all = Vec::new();
                    for dir_entry in dir_entries {
                        match dir_entry.map(|dir_entry| Self::parse(&dir_entry.path())) {
                            Ok((sc, exts)) => {
                                let len = exts.iter().filter_map(match_len).max();
                                if let Some(len) = len.filter(|l| best.as_ref().is_none_or(|b| *l > b.0)) {
                                    best = Some((len, sc.clone()));
                                }
                                all.push((exts, sc));
                            }
                            Err(e) => eprintln!("Error iterating through {data_dir}/syntax.d: {e}"),
                        }
                    }
                    if let Some((_, mut sc)) = best {
                        if sc.highlight_markdown {
                            sc.embedded = all.into_iter().filter(|(_, c)| !c.highlight_markdown).collect();
                        }
                        return sc;
                    }
                }
                Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
                Err(e) => eprintln!("Error iterating through {data_dir}/syntax.d: {e}"),
            }
        }
        Self::default()
    }

    /// Return the index in `self.embedded` of the syntax of a code block, given
    /// the language written after the opening fence, e.g. `rust` or `rs`.
    pub fn embedded_index(&self, lang: &str) -> Option<usize> {
        let lang = match lang.to_ascii_lowercase().as_str() {
            "shell" | "console" | "shell-session" | "zsh" => "sh".to_owned(),
            lang => lang.to_owned(),
        };
        (!lang.is_empty()).then_some(())?;
        self.embedded
            .iter()
            .position(|(exts, c)| c.name.eq_ignore_ascii_case(&lang) || exts.iter().any(|e| e.eq_ignore_ascii_case(&lang)))
    }

    /// Load and parse a `SyntaxConf` from file.
    pub fn parse(path: &Path) -> (Self, Vec<String>) {
        let (mut sc, mut suffixes) = (Self::default(), Vec::new());
        config::process_ini_file(path, &mut |key, val| {
            match key {
                "name" => sc.name = pv(val)?,
                "extensions" => suffixes.extend(val.split(',').map(|u| u.trim().to_owned())),
                "highlight_numbers" => sc.highlight_numbers = pv(val)?,
                "singleline_string_quotes" => sc.sl_string_quotes = pvs(val)?,
                "singleline_comment_start" => sc.sl_comment_start = pvs(val)?,
                "multiline_comment_delims" =>
                    sc.ml_comment_delims = match val.split_once(',') {
                        Some((v1, v2)) if !v2.contains(',') => Some((pv(v1.trim())?, pv(v2.trim())?)),
                        _ => return Err(format!("Expected 2 delimiters, got {val}")),
                    },
                "multiline_string_delim" => sc.ml_string_delim = Some(pv(val)?),
                "keywords_1" => sc.keywords.push((HlType::Keyword1, pvs(val)?)),
                "keywords_2" => sc.keywords.push((HlType::Keyword2, pvs(val)?)),
                "builtins" => sc.keywords.push((HlType::Builtin, pvs(val)?)),
                "types" => sc.keywords.push((HlType::Type, pvs(val)?)),
                "functions" => sc.keywords.push((HlType::Function, pvs(val)?)),
                "operators" => sc.keywords.push((HlType::Operator, pvs(val)?)),
                "highlight_function_calls" => sc.highlight_function_calls = pv(val)?,
                "plain_path_calls" => sc.plain_path_calls = pv(val)?,
                "plain_capitalized_calls" => sc.plain_capitalized_calls = pv(val)?,
                "highlight_operators" => sc.highlight_operators = pv(val)?,
                "highlight_punctuation" => sc.highlight_punctuation = pv(val)?,
                "highlight_capitalized" => sc.highlight_capitalized = pv(val)?,
                "highlight_commands" => sc.highlight_commands = pv(val)?,
                "type_definition_keywords" => sc.type_def_keywords = pvs(val)?,
                "function_definition_keywords" => sc.function_def_keywords = pvs(val)?,
                "highlight_parameters" => sc.highlight_parameters = pv(val)?,
                "highlight_variables" => sc.highlight_variables = pv(val)?,
                "highlight_options" => sc.highlight_options = pv(val)?,
                "negative_numbers" => sc.negative_numbers = pv(val)?,
                "docstrings" => sc.docstrings = pv(val)?,
                "decorators" => sc.decorators = pv(val)?,
                "attributes" => sc.attributes = pv(val)?,
                "keys" => sc.keys = Some(parse_hl_type(val)?),
                "key_separators" => sc.key_separators = pvs(val)?,
                "key_separator" => sc.key_separator = Some(parse_hl_type(val)?),
                "unquoted_values" => sc.unquoted_values = pv(val)?,
                "sections" => sc.sections = Some(parse_hl_type(val)?),
                "highlight_markdown" => sc.highlight_markdown = pv(val)?,
                "highlight_tags" => sc.highlight_tags = pv(val)?,
                "highlight_css" => sc.highlight_css = pv(val)?,
                "highlight_units" => sc.highlight_units = pv(val)?,
                _ => return Err(String::from("Invalid key")),
            }
            Ok(())
        });
        (sc, suffixes)
    }
}

#[cfg(test)]
#[cfg(not(target_family = "wasm"))] // No filesystem on wasm
mod tests {
    use std::collections::HashSet;
    use std::fs;

    use tempfile::TempDir;

    use super::*;

    #[test]
    fn syntax_d_files() {
        let mut file_count = 0;
        let mut syntax_names = HashSet::new();
        for path in fs::read_dir("./syntax.d").unwrap() {
            let (conf, extensions) = Conf::parse(&path.unwrap().path());
            assert_ne!(extensions.len(), 0);
            syntax_names.insert(conf.name);
            file_count += 1;
        }
        assert!(file_count > 0);
        assert_eq!(file_count, syntax_names.len());
    }

    #[test]
    fn find_matches_extension_or_file_name_longest_first() {
        let tmp_dir = TempDir::new().expect("Could not create temporary directory");
        let syntax_dir = tmp_dir.path().join("syntax.d");
        fs::create_dir_all(&syntax_dir).unwrap();
        for (file, name, exts) in [
            ("ts.ini", "TS", "ts"),
            ("dts.ini", "DTS", "d.ts"),
            ("make.ini", "Make", "Makefile"),
            ("sh.ini", "Sh", "bashrc"),
        ] {
            fs::write(syntax_dir.join(file), format!("name={name}\nextensions={exts}\n")).unwrap();
        }
        let dirs = [tmp_dir.path().to_string_lossy().to_string()];
        let find = |file_name: &str| Conf::find(file_name, &dirs).name;

        assert_eq!(find("a/b/index.ts"), "TS");
        assert_eq!(find("types.d.ts"), "DTS", "longest match wins");
        assert_eq!(find("src/Makefile"), "Make", "whole file name");
        assert_eq!(find("src/NotMakefile"), "", "partial file name does not match");
        assert_eq!(find("/home/user/.bashrc"), "Sh", "dot file");
        assert_eq!(find("README"), "");
    }

    #[test]
    fn parse_trims_delimiters_and_options() {
        let tmp_dir = TempDir::new().expect("Could not create temporary directory");
        let path = tmp_dir.path().join("test.ini");
        let ini = "name=Test\nextensions=t\nmultiline_comment_delims={-, -}\n\
                   types=Foo\nhighlight_operators=true\ntype_definition_keywords=data\n";
        fs::write(&path, ini).unwrap();
        let (conf, exts) = Conf::parse(&path);
        assert_eq!(exts, ["t"]);
        assert_eq!(conf.ml_comment_delims, Some(("{-".into(), "-}".into())));
        assert_eq!(conf.keywords, [(HlType::Type, vec!["Foo".into()])]);
        assert!(conf.highlight_operators);
        assert_eq!(conf.type_def_keywords, ["data"]);
    }

    #[test]
    fn conf_from_invalid_path() {
        let tmp_dir = TempDir::new().expect("Could not create temporary directory");
        let tmp_path = tmp_dir.path().join("path_does_not_exist.ini");
        assert_eq!(Conf::parse(&tmp_path), (Conf::default(), Vec::<String>::new()));
    }
}
