// SPDX-FileCopyrightText: 2020 Ilaï Deutel & Kibi Contributors
//
// SPDX-License-Identifier: MIT OR Apache-2.0

//! # Row
//!
//! Utilities for rows. A `Row` owns the underlying characters, the rendered
//! string and the syntax highlighting information.

use std::{iter::repeat_n, num::NonZeroUsize};

use unicode_width::UnicodeWidthChar;

use crate::ansi_escape::{RESET, WBG, push_colored};
use crate::syntax::{Conf as SyntaxConf, HlType};

/// The "Highlight State" of the row
#[derive(Clone, Copy, Default, PartialEq, Eq)]
pub enum HlState {
    /// Normal state.
    #[default]
    Normal,
    /// A multi-line comment has been open, but not yet closed.
    MultiLineComment,
    /// A string has been open with the given quote character (for instance
    /// b'\'' or b'"'), but not yet closed.
    String(u8),
    /// A multi-line string has been open, but not yet closed.
    MultiLineString,
    /// A docstring (a multi-line string that starts a line) has been open, but
    /// not yet closed.
    Docstring,
    /// Inside a Markdown code block, highlighted with the given embedded syntax
    /// (see `SyntaxConf::embedded`), in the given state.
    Fenced(u8, CodeState),
}

/// The state of a row inside a Markdown code block, for the syntax of the code.
#[derive(Clone, Copy, Default, PartialEq, Eq, Debug)]
pub enum CodeState {
    #[default]
    Normal,
    MultiLineComment,
    MultiLineString,
    Docstring,
}

impl From<CodeState> for HlState {
    fn from(state: CodeState) -> Self {
        match state {
            CodeState::Normal => Self::Normal,
            CodeState::MultiLineComment => Self::MultiLineComment,
            CodeState::MultiLineString => Self::MultiLineString,
            CodeState::Docstring => Self::Docstring,
        }
    }
}

impl From<HlState> for CodeState {
    fn from(state: HlState) -> Self {
        match state {
            HlState::MultiLineComment => Self::MultiLineComment,
            HlState::MultiLineString => Self::MultiLineString,
            HlState::Docstring => Self::Docstring,
            HlState::Normal | HlState::String(_) | HlState::Fenced(..) => Self::Normal,
        }
    }
}

/// Represents a row of characters and how it is rendered.
#[derive(Default)]
pub struct Row {
    /// The characters of the row.
    pub chars: Vec<u8>,
    /// How the characters are rendered. In particular, tabs are converted into
    /// several spaces, and bytes may be combined into single UTF-8
    /// characters.
    render: String,
    /// Mapping from indices in `self.chars` to the corresponding indices in
    /// `self.render`.
    pub cx2rx: Vec<usize>,
    /// Mapping from indices in `self.render` to the corresponding indices in
    /// `self.chars`.
    pub rx2cx: Vec<usize>,
    /// The vector of `HLType` for each rendered character.
    pub hl: Vec<HlType>,
    /// The final state of the row.
    pub hl_state: HlState,
    /// If not `None`, the range that is currently matched during a FIND
    /// operation.
    pub match_segment: Option<std::ops::Range<usize>>,
}

impl Row {
    /// Create a new row, containing characters `chars`.
    pub fn new(chars: Vec<u8>) -> Self { Self { chars, cx2rx: vec![0], ..Self::default() } }

    // TODO: Combine update and update_syntax
    /// Update the row: convert tabs into spaces and compute highlight symbols
    /// The `hl_state` argument is the `HLState` for the previous row.
    pub fn update(&mut self, syntax: &SyntaxConf, hl_state: HlState, tab: NonZeroUsize) -> HlState {
        let (..) = (self.render.clear(), self.cx2rx.clear(), self.rx2cx.clear());
        let (mut cx, mut rx) = (0, 0);
        for c in String::from_utf8_lossy(&self.chars).chars() {
            // The number of rendered characters
            let n_rend_chars =
                if c == '\t' { tab.get() - (rx % tab) } else { c.width().unwrap_or(1) };
            self.render.push_str(&(if c == '\t' { " ".repeat(n_rend_chars) } else { c.into() }));
            self.cx2rx.extend(repeat_n(rx, c.len_utf8()));
            self.rx2cx.extend(repeat_n(cx, n_rend_chars));
            (rx, cx) = (rx + n_rend_chars, cx + c.len_utf8());
        }
        let (..) = (self.cx2rx.push(rx), self.rx2cx.push(cx));
        self.update_syntax(syntax, hl_state)
    }

    /// Obtain the character size, in bytes, given its position in
    /// `self.render`. This is done in constant time by using the difference
    /// between `self.rx2cx[rx]` and the cx for the next character.
    pub fn get_char_size(&self, rx: usize) -> usize {
        self.rx2cx.iter().skip(rx + 1).map(|cx| cx - self.rx2cx[rx]).find(|d| *d > 0).unwrap_or(1)
    }

    /// Update the syntax highlighting types of the row.
    fn update_syntax(&mut self, syntax: &SyntaxConf, mut hl_state: HlState) -> HlState {
        if syntax.highlight_markdown
            && let Some(state) = self.update_code_block(syntax, hl_state)
        {
            self.hl_state = state;
            return state;
        }
        self.hl.clear();
        let line = self.render.as_bytes();
        let start_state = hl_state;
        let mut ctx = LineContext::default();

        // Delimiters for multi-line comments and multi-line strings, as Option<&String,
        // &String>
        let ml_comment_delims = syntax.ml_comment_delims.as_ref().map(|(start, end)| (start, end));
        let ml_string_delims = syntax.ml_string_delim.as_ref().map(|x| (x, x));

        'syntax_loop: while self.hl.len() < line.len() {
            let i = self.hl.len();
            let find_str = |s: &str| line.get(i..(i + s.len())).is_some_and(|r| r.eq(s.as_bytes()));

            if hl_state == HlState::Normal && syntax.sl_comment_start.iter().any(|s| find_str(s)) {
                self.hl.extend(repeat_n(HlType::Comment, line.len() - i));
                continue;
            }

            // Multi-line strings and multi-line comments have the same behavior; the only
            // differences are: the start/end delimiters, the `HLState`, the `HLType`.
            for (delims, mstate, mtype) in &[
                (ml_comment_delims, HlState::MultiLineComment, HlType::MlComment),
                (ml_string_delims, HlState::MultiLineString, HlType::MlString),
                (ml_string_delims, HlState::Docstring, HlType::MlComment),
            ] {
                if let Some((start, end)) = delims {
                    if hl_state == *mstate {
                        if find_str(end) {
                            // Highlight the remaining symbols of the multi line comment end
                            self.hl.extend(repeat_n(mtype, end.len()));
                            hl_state = HlState::Normal;
                        } else {
                            self.hl.push(*mtype);
                        }
                        continue 'syntax_loop;
                    } else if hl_state == HlState::Normal && find_str(start) {
                        // A multi-line string that starts a line is a docstring
                        let docstring = *mstate == HlState::MultiLineString
                            && syntax.docstrings
                            && line[..i].iter().all(u8::is_ascii_whitespace);
                        let (state, hl_type) =
                            if docstring { (HlState::Docstring, HlType::MlComment) } else { (*mstate, *mtype) };
                        // Highlight the remaining symbols of the multi line comment start
                        self.hl.extend(repeat_n(hl_type, start.len()));
                        hl_state = state;
                        continue 'syntax_loop;
                    }
                }
            }

            let c = line[i];

            // At this point, hl_state is Normal or String
            if let HlState::String(quote) = hl_state {
                if c == b'\\' && i != line.len() - 1 {
                    self.hl.extend([HlType::Escape; 2]);
                    continue;
                }
                if quote == b'"' && syntax.highlight_variables && push_variable(&mut self.hl, line, i) {
                    continue;
                }
                self.hl.push(HlType::String);
                if c == quote {
                    hl_state = HlState::Normal;
                }
                continue;
            } else if syntax.sl_string_quotes.contains(&(c as char)) {
                hl_state = HlState::String(c);
                self.hl.push(HlType::String);
                continue;
            }

            if syntax.highlight_tags
                && let Some((hl_type, len)) = tag_hl_type(&mut ctx, line, i)
            {
                self.hl.extend(repeat_n(hl_type, len));
                continue;
            }

            if let Some(hl_type) = number_hl_type(syntax, line, &self.hl) {
                self.hl.push(hl_type);
                continue;
            }

            let prev_sep = (i == 0) || is_sep(line[i - 1]);

            if prev_sep && !ctx.in_tag && !syntax.highlight_tags {
                // This filters makes sure that names such as "in_comment" are not partially
                // highlighted (even though "in" is a keyword in rust)
                // The argument is the keyword that is matched at `i`.
                let s_filter = |kw: &str| line.get(i + kw.len()).is_none_or(|c| is_sep(*c));
                for (keyword_highlight_type, kws) in &syntax.keywords {
                    if let Some(kw) = kws.iter().find(|kw| !kw.is_empty() && find_str(kw) && s_filter(kw)) {
                        self.hl.extend(repeat_n(*keyword_highlight_type, kw.len()));
                        continue 'syntax_loop;
                    }
                }
            }

            if prev_sep && !is_sep(c) {
                let word = line[i..].split(|c| is_sep(*c)).next().unwrap_or_default();
                let hl_type = word_hl_type(syntax, &mut ctx, line, i, word);
                self.hl.extend(repeat_n(hl_type, word.len()));
                continue;
            }

            self.hl.push(punctuation_hl_type(syntax, &mut ctx, line, i));
        }

        highlight_whole_line(syntax, line, &mut self.hl, start_state);

        // String state doesn't propagate to the next row
        self.hl_state =
            if matches!(hl_state, HlState::String(_)) { HlState::Normal } else { hl_state };
        self.hl_state
    }

    /// Highlight the row if it is a fence or a line of a Markdown code block whose
    /// language has a syntax, e.g. ```` ```rust ````, and return the new state.
    /// Return `None` for other rows.
    fn update_code_block(&mut self, syntax: &SyntaxConf, hl_state: HlState) -> Option<HlState> {
        let line = self.render.as_bytes();
        let indent = line.iter().take_while(|c| c.is_ascii_whitespace()).count();
        let ticks = line[indent..].iter().take_while(|c| **c == b'`').count();
        let fence = ticks >= 3;
        let state = match hl_state {
            // Closing fence
            HlState::Fenced(..) if fence => HlState::Normal,
            HlState::Fenced(idx, code_state) => {
                let (_, code_syntax) = syntax.embedded.get(usize::from(idx))?;
                let new_state = self.update_syntax(code_syntax, code_state.into());
                return Some(HlState::Fenced(idx, new_state.into()));
            }
            // Opening fence, e.g. ```rust
            HlState::Normal if fence => {
                let info = String::from_utf8_lossy(&line[indent + ticks..]);
                let lang = info.trim().split([' ', '{', ',']).next().unwrap_or_default();
                HlState::Fenced(u8::try_from(syntax.embedded_index(lang)?).ok()?, CodeState::Normal)
            }
            HlState::Normal
            | HlState::MultiLineComment
            | HlState::String(_)
            | HlState::MultiLineString
            | HlState::Docstring => return None,
        };
        self.hl.clear();
        self.hl.extend(repeat_n(HlType::Normal, line.len()));
        self.hl[indent..indent + ticks].fill(HlType::Punctuation);
        Some(state)
    }

    /// Draw the row and write the result to a buffer. An `offset` can be given,
    /// as well as a limit on the length of the row (`max_len`). After
    /// writing the characters, clear the rest of the line and move the
    /// cursor to the start of the next line.
    pub fn draw(&self, offset: usize, max_len: usize, buffer: &mut String, use_color: bool) {
        let mut current_hl_type = HlType::Normal;
        let chars = self.render.chars().skip(offset).take(max_len);
        let mut rx = self.render.chars().take(offset).map(|c| c.width().unwrap_or(1)).sum();
        for (c, mut hl_type) in chars.zip(self.hl.iter().skip(offset)) {
            if c.is_ascii_control() {
                let rendered_char = if (c as u8) <= 26 { (b'@' + c as u8) as char } else { '?' };
                push_colored(buffer, WBG, &rendered_char.to_string(), use_color);
                // Restore previous color
                if use_color && current_hl_type != HlType::Normal {
                    buffer.push_str(&current_hl_type.to_string());
                }
            } else {
                if let Some(match_segment) = &self.match_segment {
                    if match_segment.contains(&rx) {
                        // Set the highlight type to Match, i.e. set the background to cyan
                        hl_type = &HlType::Match;
                    } else if use_color && rx == match_segment.end {
                        // Reset the formatting, in particular the background
                        buffer.push_str(RESET);
                    }
                }
                if use_color && current_hl_type != *hl_type {
                    buffer.push_str(&hl_type.to_string());
                }
                current_hl_type = *hl_type;
                buffer.push(c);
            }
            rx += c.width().unwrap_or(1);
        }
        buffer.push_str(if use_color { RESET } else { "" });
    }
}

/// State that is only needed while highlighting a single row.
#[derive(Default)]
struct LineContext {
    /// Inside an HTML/XML tag, e.g. after `<div`.
    in_tag: bool,
    /// The next word is a tag name, e.g. after `<` or `</`.
    expect_tag_name: bool,
    /// The next `(` opens the parameters of a function definition.
    pending_params: bool,
    /// Depth of parentheses inside the parameters of a function definition.
    param_depth: usize,
}

/// Return whether `c` is part of a word, e.g. a variable name.
const fn is_word_char(c: u8) -> bool { c.is_ascii_alphanumeric() || c == b'_' }

/// The non-whitespace character before `line[i]`, if any.
fn prev_non_space(line: &[u8], i: usize) -> Option<u8> {
    line[..i].iter().rev().find(|c| !c.is_ascii_whitespace()).copied()
}

/// If `line[i]` starts a variable (`$name`, `${name}`, `$1`...), highlight it
/// and return true.
fn push_variable(hl: &mut Vec<HlType>, line: &[u8], i: usize) -> bool {
    if line[i] != b'$' {
        return false;
    }
    let len = match line.get(i + 1) {
        Some(b'{') => match line[i..].iter().position(|c| *c == b'}') {
            Some(end) => {
                hl.extend([HlType::Builtin, HlType::Punctuation]);
                hl.extend(repeat_n(HlType::Normal, end - 2));
                hl.push(HlType::Punctuation);
                return true;
            }
            None => return false,
        },
        Some(c) if is_word_char(*c) => line[i + 1..].iter().take_while(|c| is_word_char(**c)).count(),
        Some(b'@' | b'*' | b'#' | b'?' | b'!' | b'$' | b'-') => 1,
        _ => return false,
    };
    hl.push(HlType::Builtin);
    hl.extend(repeat_n(HlType::Normal, len));
    true
}

/// Return the highlight type and length of an HTML/XML tag delimiter or
/// entity at `line[i]`, if any.
fn tag_hl_type(ctx: &mut LineContext, line: &[u8], i: usize) -> Option<(HlType, usize)> {
    let next = line.get(i + 1).copied().unwrap_or_default();
    match line[i] {
        b'<' if next.is_ascii_alphabetic() || matches!(next, b'/' | b'?' | b'!') => {
            (ctx.in_tag, ctx.expect_tag_name) = (true, true);
            Some((HlType::Operator, 1))
        }
        b'/' | b'?' | b'!' | b'=' if ctx.in_tag => Some((HlType::Operator, 1)),
        b'>' if ctx.in_tag => {
            ctx.in_tag = false;
            Some((HlType::Operator, 1))
        }
        // Entities, e.g. `&amp;`
        b'&' if !ctx.in_tag => {
            let len = line[i + 1..].iter().take_while(|c| is_word_char(**c) || **c == b'#').count();
            (len > 0 && line.get(i + 1 + len) == Some(&b';')).then_some((HlType::Builtin, len + 2))
        }
        _ => None,
    }
}

/// Return the highlight type of `line[i]`, where `i = hl.len()`, if it is part
/// of a number or of a CSS unit (e.g. `px` in `4px`).
fn number_hl_type(syntax: &SyntaxConf, line: &[u8], hl: &[HlType]) -> Option<HlType> {
    let (i, c) = (hl.len(), line[hl.len()]);
    let prev_sep = (i == 0) || is_sep(line[i - 1]);
    let in_number = i != 0 && hl[i - 1] == HlType::Number;
    let in_unit = i != 0 && hl[i - 1] == HlType::Keyword1 && hl.contains(&HlType::Number);
    if syntax.highlight_units && !prev_sep && c.is_ascii_alphabetic() && (in_number || in_unit) {
        return Some(HlType::Keyword1);
    }
    (syntax.highlight_numbers
        && ((c.is_ascii_digit() && prev_sep)
            || (in_number && !prev_sep && !is_sep(c))
            || (in_number && c == b'.' && line.get(i + 1).is_some_and(u8::is_ascii_digit))
            || (syntax.negative_numbers && negative_number_start(line, i))))
    .then_some(HlType::Number)
}

/// Apply the highlighting rules that need the whole line: keys of configuration
/// files, CSS selectors, Markdown and attributes.
fn highlight_whole_line(syntax: &SyntaxConf, line: &[u8], hl: &mut [HlType], start_state: HlState) {
    if start_state == HlState::MultiLineComment {
        return;
    }
    let key_found = highlight_keys(syntax, line, hl);
    if syntax.highlight_css && !key_found && start_state == HlState::Normal {
        highlight_css_selectors(line, hl);
    }
    if syntax.highlight_markdown {
        highlight_markdown(line, hl, start_state);
    }
    if syntax.attributes {
        highlight_attribute(line, hl);
    }
}

/// Return whether `line[i]` is the `-` of a negative number, e.g. in `x = -1`.
fn negative_number_start(line: &[u8], i: usize) -> bool {
    line[i] == b'-'
        && line.get(i + 1).is_some_and(u8::is_ascii_digit)
        && (prev_non_space(line, i).is_none_or(|c| b"=([,:+-*/%<>!&|?{".contains(&c))
            || line[..i].trim_ascii_end().ends_with(b"return"))
}

/// Return the highlight type of `word`, which starts at `line[i]` and is not
/// a keyword.
fn word_hl_type(syntax: &SyntaxConf, ctx: &mut LineContext, line: &[u8], i: usize, word: &[u8]) -> HlType {
    let next = line.get(i + word.len());
    let before = line[..i].last().copied();
    // A command starts a line, or follows `|`, `;`, `&`, `(` or a backtick
    let prev = prev_non_space(line, i);
    // The word before, e.g. `struct` in `struct Foo`
    let prev_word = line[..i].trim_ascii_end().rsplit(|c| is_sep(*c)).next().unwrap_or_default();
    let after = |kws: &[String]| !prev_word.is_empty() && kws.iter().any(|k| k.as_bytes() == prev_word);
    // Options, e.g. `l` in `ls -l` or `all` in `ls --all`
    let dashes = line[..i].iter().rev().take_while(|c| **c == b'-').count();
    let is_option = (1..=2).contains(&dashes) && (i == dashes || line[i - dashes - 1].is_ascii_whitespace());
    // Parameters, e.g. `a` in `def f(a, *b)`
    let param_prev = line[..i].trim_ascii_end();
    let param_prev = param_prev.strip_suffix(b"**").or_else(|| param_prev.strip_suffix(b"*")).unwrap_or(param_prev);
    let is_param = ctx.param_depth == 1 && matches!(prev_non_space(param_prev, param_prev.len()), Some(b'(' | b','));
    if ctx.in_tag {
        let expect_name = std::mem::take(&mut ctx.expect_tag_name);
        return match next {
            _ if expect_name => HlType::Function,
            Some(b'=') => HlType::Type,
            _ if before == Some(b'=') => HlType::String,
            _ => HlType::Type,
        };
    }
    match word.first() {
        _ if syntax.highlight_variables && before == Some(b'$') => HlType::Normal,
        _ if (syntax.decorators || syntax.highlight_css) && before == Some(b'@') =>
            if syntax.decorators { HlType::Keyword2 } else { HlType::Keyword1 },
        _ if syntax.highlight_options && is_option => HlType::Parameter,
        _ if syntax.highlight_parameters && is_param => HlType::Parameter,
        _ if after(&syntax.type_def_keywords) => HlType::Type,
        _ if after(&syntax.function_def_keywords) => {
            ctx.pending_params = syntax.highlight_parameters && next == Some(&b'(');
            HlType::Function
        }
        Some(c) if syntax.highlight_capitalized && c.is_ascii_uppercase()
            && word.iter().any(u8::is_ascii_lowercase) => HlType::Type,
        Some(c) if syntax.highlight_function_calls && next == Some(&b'(')
            && !(syntax.plain_path_calls && line[..i].ends_with(b"::"))
            && !(syntax.plain_capitalized_calls && c.is_ascii_uppercase()) => HlType::Function,
        _ if syntax.highlight_commands && next != Some(&b'=') && prev.is_none_or(|c| b"|;&(`".contains(&c)) =>
            HlType::Function,
        _ => HlType::Normal,
    }
}

/// Return the highlight type of the punctuation character at `line[i]`.
fn punctuation_hl_type(syntax: &SyntaxConf, ctx: &mut LineContext, line: &[u8], i: usize) -> HlType {
    let next = line.get(i + 1).copied();
    // `::` is an operator (path separator), a single `:` is punctuation
    let double_colon = next == Some(b':') || (i != 0 && line[i - 1] == b':');
    let line_start = line[..i].iter().all(u8::is_ascii_whitespace);
    match line[i] {
        b'(' if ctx.pending_params => (ctx.pending_params, ctx.param_depth) = (false, 1),
        b'(' if ctx.param_depth > 0 => ctx.param_depth += 1,
        b')' if ctx.param_depth > 0 => ctx.param_depth -= 1,
        _ => (),
    }
    match line[i] {
        b'$' if syntax.highlight_variables
            && next.is_some_and(|c| is_word_char(c) || b"{@*#?!$".contains(&c)) => HlType::Builtin,
        b'@' if syntax.decorators && line_start => HlType::Keyword2,
        b'@' if syntax.highlight_css => HlType::Keyword1,
        b'-' if syntax.highlight_options
            && next.is_some_and(|c| is_word_char(c) || c == b'-')
            && (i == 0 || line[i - 1].is_ascii_whitespace() || line[i - 1] == b'-') => HlType::Punctuation,
        b':' if double_colon && syntax.highlight_operators => HlType::Operator,
        b'(' | b')' | b'[' | b']' | b'{' | b'}' | b',' | b';' | b':' if syntax.highlight_punctuation =>
            HlType::Punctuation,
        b'+' | b'-' | b'*' | b'/' | b'%' | b'=' | b'<' | b'>' | b'!' | b'&' | b'|' | b'^' | b'~' | b'?'
        | b'.'
            if syntax.highlight_operators =>
            HlType::Operator,
        _ => HlType::Normal,
    }
}

/// Highlight keys, values and sections of configuration files, e.g. `name` in
/// `name: value` (YAML), `"name": 1` (JSON), `name = 1` (TOML) or `[section]`.
/// Return whether a key was found.
fn highlight_keys(syntax: &SyntaxConf, line: &[u8], hl: &mut [HlType]) -> bool {
    let Some(key_type) = syntax.keys else { return false };
    let is_sep_char = |c: u8| syntax.key_separators.contains(&(c as char));
    let skip_spaces = |mut j: usize| {
        while line.get(j).is_some_and(u8::is_ascii_whitespace) {
            j += 1;
        }
        j
    };
    let mut i = skip_spaces(0);
    // List items, e.g. `- item` (YAML)
    let list_item =
        syntax.unquoted_values && line.get(i) == Some(&b'-') && line.get(i + 1).is_none_or(|c| *c == b' ');
    if list_item {
        hl[i] = HlType::Punctuation;
        i = skip_spaces(i + 1);
    }
    let mut value_start = list_item.then_some(i);
    // Sections, e.g. `[package]` (TOML)
    if let Some(section_type) = syntax.sections
        && line.get(i) == Some(&b'[')
        && hl[i] != HlType::Comment
    {
        for (c, t) in line[i..].iter().zip(&mut hl[i..]) {
            *t = if matches!(c, b'[' | b']') { HlType::Punctuation } else { section_type };
        }
        return false;
    }
    let mut key_found = false;
    // Quoted keys, e.g. `"name": 1` (JSON)
    let mut j = 0;
    while j < line.len() {
        if hl[j] == HlType::String && syntax.sl_string_quotes.contains(&(line[j] as char)) {
            let end = j + 1 + hl[j + 1..].iter().take_while(|t| matches!(t, HlType::String | HlType::Escape)).count();
            let k = skip_spaces(end);
            if end - j >= 2 && line.get(k).is_some_and(|c| is_sep_char(*c)) {
                hl[j] = HlType::Punctuation;
                hl[end - 1] = HlType::Punctuation;
                hl[j + 1..end - 1].fill(key_type);
                if let Some(t) = syntax.key_separator {
                    hl[k] = t;
                }
                (key_found, value_start) = (true, Some(k + 1));
            }
            j = end;
        } else {
            j += 1;
        }
    }
    // Unquoted keys at the start of the line, e.g. `name: value` (YAML)
    if !key_found {
        let end = i + line[i..]
            .iter()
            .zip(&hl[i..])
            .take_while(|(c, t)| !is_sep_char(**c) && !matches!(t, HlType::Comment | HlType::String))
            .count();
        let key_len = line[i..end].trim_ascii_end().len();
        let sep_ok = line.get(end).is_some_and(|c| *c != b':' || line.get(end + 1).is_none_or(u8::is_ascii_whitespace));
        if key_len > 0 && sep_ok && line.get(end).is_some_and(|c| is_sep_char(*c)) {
            hl[i..i + key_len].fill(key_type);
            if let Some(t) = syntax.key_separator {
                hl[end] = t;
            }
            (key_found, value_start) = (true, Some(end + 1));
        }
    }
    // Unquoted values, e.g. `Test User` in `name: Test User` (YAML)
    if let Some(v) = value_start.filter(|_| syntax.unquoted_values) {
        let v = skip_spaces(v);
        let end = v + hl[v..].iter().take_while(|t| **t != HlType::Comment).count();
        let end = v + line[v..end].trim_ascii_end().len();
        let plain = hl[v..end].iter().all(|t| matches!(t, HlType::Normal | HlType::Punctuation | HlType::Operator));
        if v < end && plain {
            hl[v..end].fill(HlType::String);
        }
    }
    key_found
}

/// Highlight CSS selectors, e.g. `.class`, `#id`, `tag` and `:hover` in
/// `tag.class #id:hover {`.
fn highlight_css_selectors(line: &[u8], hl: &mut [HlType]) {
    let trimmed = line.trim_ascii();
    if trimmed.first() == Some(&b'@') || !matches!(trimmed.last(), Some(b'{' | b',')) {
        return;
    }
    let mut i = 0;
    while i < line.len() {
        let c = line[i];
        if matches!(hl[i], HlType::Comment | HlType::MlComment | HlType::String) {
            i += 1;
        } else if is_word_char(c) || c == b'-' && line.get(i + 1).is_some_and(|c| is_word_char(*c)) {
            let len = line[i..].iter().take_while(|c| is_word_char(**c) || **c == b'-').count();
            let hl_type = match i.checked_sub(1).map(|j| line[j]) {
                Some(b'.' | b'#') => HlType::Type,
                Some(b':') => HlType::Normal,
                _ => HlType::Function,
            };
            hl[i..i + len].fill(hl_type);
            i += len;
        } else {
            if matches!(c, b'.' | b'#' | b':' | b'>' | b'+' | b'~' | b'*' | b',' | b'&') {
                hl[i] = HlType::Punctuation;
            }
            i += 1;
        }
    }
}

/// Highlight Markdown: headings, thematic breaks, list markers, code spans and
/// code blocks, emphasis and links.
fn highlight_markdown(line: &[u8], hl: &mut [HlType], start_state: HlState) {
    let i = line.iter().take_while(|c| c.is_ascii_whitespace()).count();
    let rest = &line[i..];
    // Code blocks: bat highlights them with the syntax of their language, here they stay plain
    if rest.starts_with(b"```") {
        hl.fill(HlType::Normal);
        hl[i..i + rest.iter().take_while(|c| **c == b'`').count()].fill(HlType::Punctuation);
        return;
    }
    if start_state == HlState::MultiLineString {
        hl.fill(HlType::Normal);
        return;
    }
    let hashes = rest.iter().take_while(|c| **c == b'#').count();
    if (1..=6).contains(&hashes) && rest.get(hashes).is_none_or(|c| *c == b' ') {
        hl[i..i + hashes].fill(HlType::Punctuation);
        hl[i + hashes..].fill(HlType::Operator);
        return;
    }
    let marks: Vec<u8> = rest.iter().copied().filter(|c| !c.is_ascii_whitespace()).collect();
    if marks.len() >= 3 && matches!(marks[0], b'-' | b'*' | b'_') && marks.iter().all(|c| *c == marks[0]) {
        hl[i..].fill(HlType::Punctuation);
        return;
    }
    // Every other line is plain text, apart from the inline elements below
    for t in hl.iter_mut().filter(|t| !matches!(t, HlType::String | HlType::Comment | HlType::MlComment)) {
        *t = HlType::Normal;
    }
    // Tables, e.g. `a | b` and `--- | ---`
    if rest.contains(&b'|') {
        let separator_row = rest.iter().all(|c| b"-|: ".contains(c)) && rest.contains(&b'-');
        for (c, t) in line.iter().zip(hl.iter_mut()) {
            if separator_row || *c == b'|' && *t == HlType::Normal {
                *t = HlType::Punctuation;
            }
        }
    }
    // Block quotes, e.g. `> quote`
    if rest.first() == Some(&b'>') {
        hl[i] = HlType::Punctuation;
        hl[i + 1..].fill(HlType::Escape); // Pink
    }
    // List markers, e.g. `-` in `- item` or `.` in `1. item`
    let digits = rest.iter().take_while(|c| c.is_ascii_digit()).count();
    if matches!(rest.first(), Some(b'-' | b'*' | b'+')) && rest.get(1) == Some(&b' ') {
        hl[i] = HlType::Punctuation;
    } else if digits > 0 && matches!(rest.get(digits), Some(b'.' | b')')) && rest.get(digits + 1) == Some(&b' ') {
        hl[i + digits] = HlType::Punctuation;
    }
    highlight_markdown_inline(line, hl, i);
}

/// Highlight Markdown inline elements, starting at `line[i]`: code spans, links,
/// images and emphasis.
fn highlight_markdown_inline(line: &[u8], hl: &mut [HlType], i: usize) {
    let mut j = i;
    while j < line.len() {
        match line[j] {
            // Code spans, e.g. `code`
            b'`' if hl[j] == HlType::String => {
                let end = j + 1 + hl[j + 1..].iter().take_while(|t| **t == HlType::String).count();
                hl[j..end].fill(HlType::Normal);
                hl[j] = HlType::Punctuation;
                if end - j >= 2 && line[end - 1] == b'`' {
                    hl[end - 1] = HlType::Punctuation;
                }
                j = end;
            }
            // Links, e.g. [text](url), and images, e.g. ![text](url)
            b'[' => {
                let image = j > 0 && line[j - 1] == b'!';
                let bracket = if image { HlType::Punctuation } else { HlType::Function };
                let close = line[j..].iter().position(|c| *c == b']').map(|p| j + p);
                let url_end = close.filter(|c| line.get(c + 1) == Some(&b'(')).and_then(|c| {
                    line[c..].iter().position(|ch| *ch == b')').map(|p| c + p)
                });
                if let (Some(close), Some(url_end)) = (close, url_end) {
                    if image {
                        hl[j - 1] = HlType::Punctuation;
                    }
                    hl[j] = bracket;
                    hl[close] = bracket;
                    hl[close + 1] = HlType::Punctuation;
                    hl[close + 2..url_end].fill(HlType::Function);
                    hl[url_end] = HlType::Punctuation;
                    j = url_end + 1;
                } else {
                    j += 1;
                }
            }
            // Emphasis, e.g. *italic*, _italic_, **bold**, __bold__ or ~~strikethrough~~
            m @ (b'*' | b'_' | b'~') if j == 0 || !is_word_char(line[j - 1]) || m != b'_' => {
                let n = if line.get(j + 1) == Some(&m) { 2 } else { 1 };
                if m == b'~' && n == 1 {
                    j += 1;
                    continue;
                }
                let body = j + n;
                let close = line.get(body..).and_then(|s| {
                    s.windows(n).position(|w| w.iter().all(|c| *c == m)).map(|p| body + p)
                });
                match close {
                    Some(close) if close > body && !line[body].is_ascii_whitespace()
                        && !line[close - 1].is_ascii_whitespace() =>
                    {
                        hl[j..body].fill(HlType::Punctuation);
                        hl[close..close + n].fill(HlType::Punctuation);
                        hl[body..close].fill(if n == 1 { HlType::Builtin } else { HlType::Normal });
                        j = close + n;
                    }
                    _ => j += n,
                }
            }
            _ => j += 1,
        }
    }
}

/// Highlight an attribute at the start of the line, e.g. `#[derive(Debug)]`.
fn highlight_attribute(line: &[u8], hl: &mut [HlType]) {
    let i = line.iter().take_while(|c| c.is_ascii_whitespace()).count();
    let rest = &line[i..];
    if !(rest.starts_with(b"#[") || rest.starts_with(b"#![")) {
        return;
    }
    let end = rest.iter().rposition(|c| *c == b']').map_or(line.len(), |p| i + p + 1);
    hl[i] = HlType::Keyword2;
    for t in &mut hl[i + 1..end] {
        if !matches!(t, HlType::String | HlType::Escape | HlType::Operator | HlType::Number) {
            *t = HlType::Type;
        }
    }
}

/// Return whether `c` is an ASCII separator.
const fn is_sep(c: u8) -> bool {
    c.is_ascii_whitespace() || c == b'\0' || (c.is_ascii_punctuation() && c != b'_')
}

#[cfg(test)]
mod tests {
    use super::*;

    /// One letter per highlight type, to keep the expected values readable.
    fn letter(hl: HlType) -> char {
        match hl {
            HlType::Normal => '.',
            HlType::Number => 'n',
            HlType::String | HlType::MlString => 's',
            HlType::Escape => 'e',
            HlType::Comment | HlType::MlComment => 'c',
            HlType::Keyword1 => 'k',
            HlType::Keyword2 => 'K',
            HlType::Builtin => 'b',
            HlType::Type => 't',
            HlType::Function => 'f',
            HlType::Operator => 'o',
            HlType::Punctuation => 'p',
            HlType::Parameter => 'a',
            HlType::Match => 'm',
        }
    }

    /// Highlight `line` and return one letter per character, to keep the
    /// expected values readable.
    fn highlight(syntax: &SyntaxConf, line: &str) -> String {
        let mut row = Row::new(line.as_bytes().to_vec());
        row.update(syntax, HlState::Normal, NonZeroUsize::new(4).unwrap());
        row.hl.iter().map(|hl| letter(*hl))
            .collect()
    }

    fn rust_like() -> SyntaxConf {
        SyntaxConf {
            highlight_numbers: true,
            sl_string_quotes: vec!['"'],
            keywords: vec![(HlType::Keyword1, vec!["fn".into(), "let".into()]), (HlType::Type, vec!["Vec".into()])],
            highlight_function_calls: true,
            highlight_operators: true,
            highlight_punctuation: true,
            type_def_keywords: vec!["struct".into()],
            ..SyntaxConf::default()
        }
    }

    #[test]
    fn function_calls_operators_and_punctuation() {
        let syntax = rust_like();
        assert_eq!(highlight(&syntax, "let x = foo(1.5);"), "kkk...o.fffpnnnpp");
        assert_eq!(highlight(&syntax, "Vec::new(a, b)"), "tttoofffp.p..p");
        let syntax = SyntaxConf { plain_path_calls: true, plain_capitalized_calls: true, ..rust_like() };
        assert_eq!(highlight(&syntax, "Vec::new(Some(a))"), "tttoo...p....p.pp");
    }

    #[test]
    fn escapes_in_strings() {
        assert_eq!(highlight(&rust_like(), r#"x = "a\nb";"#), "..o.sseessp");
    }

    #[test]
    fn definition_keywords() {
        let syntax = rust_like();
        assert_eq!(highlight(&syntax, "struct Foo {"), ".......ttt.p");
    }

    #[test]
    fn empty_keyword_does_not_hang() {
        let syntax = SyntaxConf { keywords: vec![(HlType::Keyword1, vec![String::new()])], ..SyntaxConf::default() };
        assert_eq!(highlight(&syntax, "a b"), "...");
    }

    #[test]
    fn shell_commands() {
        let syntax = SyntaxConf { highlight_commands: true, ..SyntaxConf::default() };
        assert_eq!(highlight(&syntax, "ls -l | grep x"), "ff......ffff..");
        assert_eq!(highlight(&syntax, "X=1"), "...");
    }

    /// Highlight several lines, carrying the state from one line to the next.
    fn highlight_lines(syntax: &SyntaxConf, lines: &[&str]) -> Vec<String> {
        let mut state = HlState::Normal;
        lines
            .iter()
            .map(|line| {
                let mut row = Row::new(line.as_bytes().to_vec());
                state = row.update(syntax, state, NonZeroUsize::new(4).unwrap());
                row.hl.iter().map(|hl| letter(*hl)).collect()
            })
            .collect()
    }

    #[test]
    fn shell_variables_and_options() {
        let syntax = SyntaxConf {
            sl_string_quotes: vec!['"'],
            highlight_variables: true,
            highlight_options: true,
            highlight_commands: true,
            ..SyntaxConf::default()
        };
        assert_eq!(highlight(&syntax, r#"ls -la "$HOME" ${X}"#), "ff.paa.sb....s.b...");
    }

    #[test]
    fn function_parameters() {
        let syntax = SyntaxConf {
            function_def_keywords: vec!["def".into()],
            highlight_parameters: true,
            ..SyntaxConf::default()
        };
        assert_eq!(highlight(&syntax, "def f(a, *b) = g(c)"), "....f.a...a........");
    }

    #[test]
    fn configuration_keys_and_values() {
        let yaml = SyntaxConf {
            keys: Some(HlType::Function),
            key_separators: vec![':'],
            key_separator: Some(HlType::Operator),
            unquoted_values: true,
            highlight_numbers: true,
            ..SyntaxConf::default()
        };
        assert_eq!(highlight(&yaml, "name: Test User"), "ffffo.sssssssss");
        assert_eq!(highlight(&yaml, "id: 42"), "ffo.nn");
        assert_eq!(highlight(&yaml, "- item"), "p.ssss");
        assert_eq!(highlight(&yaml, "url: http://x"), "fffo.ssssssss");
        let json = SyntaxConf {
            keys: Some(HlType::Function),
            key_separators: vec![':'],
            sl_string_quotes: vec!['"'],
            ..SyntaxConf::default()
        };
        assert_eq!(highlight(&json, r#"{"a": "b"}"#), ".pfp..sss.");
        let toml = SyntaxConf {
            keys: Some(HlType::Function),
            key_separators: vec!['='],
            sections: Some(HlType::Operator),
            ..SyntaxConf::default()
        };
        assert_eq!(highlight(&toml, "[bin]"), "pooop");
        assert_eq!(highlight(&toml, "n = 1"), "f....");
    }

    #[test]
    fn markdown() {
        let syntax = SyntaxConf {
            sl_string_quotes: vec!['`'],
            ml_string_delim: Some("```".into()),
            highlight_markdown: true,
            ..SyntaxConf::default()
        };
        assert_eq!(highlight(&syntax, "## Title"), "ppoooooo");
        assert_eq!(highlight(&syntax, "- *a* **b** `c` [d](e)"), "p.pbp.pp.pp.p.p.f.fpfp");
        assert_eq!(highlight(&syntax, "> q **b**"), "peeepp.pp");
        assert_eq!(highlight(&syntax, "a | b"), "..p..");
        assert_eq!(highlight(&syntax, "--|--"), "ppppp");
        assert_eq!(highlight(&syntax, "~~s~~ ![i](u)"), "pp.pp.pp.ppfp");
        assert_eq!(highlight_lines(&syntax, &["```rs", "let x", "```"]), ["ppp..", ".....", "ppp"]);
    }

    #[test]
    fn markdown_code_blocks_use_the_syntax_of_their_language() {
        let rust = SyntaxConf { ml_comment_delims: Some(("/*".into(), "*/".into())), ..rust_like() };
        let syntax = SyntaxConf {
            ml_string_delim: Some("```".into()),
            highlight_markdown: true,
            embedded: vec![(vec!["rs".into()], SyntaxConf { name: "Rust".into(), ..rust })],
            ..SyntaxConf::default()
        };
        assert_eq!(syntax.embedded_index("rust"), Some(0));
        assert_eq!(syntax.embedded_index("RS"), Some(0));
        assert_eq!(syntax.embedded_index("python"), None);
        let lines = ["```rust", "let x = f(1); /* a", "b */ let", "```", "let"];
        assert_eq!(highlight_lines(&syntax, &lines), [
            "ppp....",
            "kkk...o.fpnpp.cccc",
            "cccc.kkk",
            "ppp",
            "...",
        ]);
    }

    #[test]
    fn html_tags() {
        let syntax = SyntaxConf { sl_string_quotes: vec!['"'], highlight_tags: true, ..SyntaxConf::default() };
        assert_eq!(highlight(&syntax, r#"<a href="x">&amp;</a>"#), "of.ttttosssobbbbboofo");
    }

    #[test]
    fn css() {
        let syntax = SyntaxConf {
            highlight_numbers: true,
            highlight_css: true,
            highlight_units: true,
            keys: Some(HlType::Function),
            key_separators: vec![':'],
            key_separator: Some(HlType::Operator),
            ..SyntaxConf::default()
        };
        assert_eq!(highlight(&syntax, "p.a:hover {"), "fptp.......");
        assert_eq!(highlight(&syntax, "  margin: 4px;"), "..ffffffo.nkk.");
    }

    #[test]
    fn docstrings_decorators_attributes_negative_numbers() {
        let python = SyntaxConf {
            ml_string_delim: Some("\"\"\"".into()),
            docstrings: true,
            decorators: true,
            ..SyntaxConf::default()
        };
        assert_eq!(highlight_lines(&python, &["  \"\"\"Doc", "x\"\"\"", "x = \"\"\"s\"\"\""]), [
            "..cccccc", "cccc", "....sssssss"
        ]);
        assert_eq!(highlight(&python, "@property"), "KKKKKKKKK");
        let rust = SyntaxConf { attributes: true, sl_string_quotes: vec!['"'], ..SyntaxConf::default() };
        assert_eq!(highlight(&rust, r#"#[cfg(x = "y")]"#), "Ktttttttttssstt");
        let js = SyntaxConf { highlight_numbers: true, negative_numbers: true, ..SyntaxConf::default() };
        assert_eq!(highlight(&js, "x = -1 - 2"), "....nn...n");
    }
}
