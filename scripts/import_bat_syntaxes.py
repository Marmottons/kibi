#!/usr/bin/env python3
# SPDX-FileCopyrightText: 2020 Ilaï Deutel & Kibi Contributors
#
# SPDX-License-Identifier: MIT OR Apache-2.0

"""Generate Kibi syntax files (syntax.d/*.ini) from the syntaxes used by bat.

bat (https://github.com/sharkdp/bat) ships its syntaxes as Sublime Text
`.sublime-syntax` files (regex-based, with nested contexts). Kibi's format is
much simpler (comments, strings, numbers, keyword lists), so this is a
best-effort extraction: anything that cannot be reduced to plain tokens is
ignored.

Usage:
    git clone --depth 1 https://github.com/sharkdp/bat.git
    git -C bat submodule update --init --depth 1 --jobs 16 assets/syntaxes
    git -C bat submodule update --init --depth 1 assets/themes/Catppuccin
    pip install pyyaml ruamel.yaml
    scripts/import_bat_syntaxes.py bat/assets/syntaxes \\
        "bat/assets/themes/Catppuccin/Catppuccin Mocha.tmTheme" syntax.d

Each keyword is put in the Kibi list whose color is the one bat uses for it
with the given theme (see COLOR_KEYS): the color of its grammar scope, or else
the color bat gives it when it is alone on a line (bat must be installed). The
other options (function calls, operators, ...) are deduced by running bat on
small snippets. Generated files (starting with HEADER)
are regenerated. Hand-written files are kept, except for their keyword lists,
which are re-colored with the bat colors; generated files do not reuse their
extensions.
"""

import plistlib
import re
import subprocess
import sys
from pathlib import Path

import yaml

HEADER = "# Generated from the bat syntaxes (https://github.com/sharkdp/bat) by scripts/import_bat_syntaxes.py"
MAX_KEYWORDS = 400
MAX_EXPANSION = 2048
WORD = re.compile(r"^[A-Za-z_][A-Za-z0-9_]+$")


class Loader(yaml.SafeLoader):
    pass


# A bare `=` is the YAML "value" tag, but in sublime-syntax files it is a plain string.
Loader.add_constructor("tag:yaml.org,2002:value", lambda loader, node: "=")


class Bail(Exception):
    """The regex cannot be reduced to a finite set of literal strings."""


# ---------------------------------------------------------------------------
# Regex -> set of literal strings
# ---------------------------------------------------------------------------


def _skip_group(s, i):
    """`s[i]` is '('. Return the index just after the matching ')'."""
    depth, in_class = 0, False
    while i < len(s):
        c = s[i]
        if c == "\\":
            i += 2
            continue
        if in_class:
            in_class = c != "]"
        elif c == "[":
            in_class = True
        elif c == "(":
            depth += 1
        elif c == ")":
            depth -= 1
            if depth == 0:
                return i + 1
        i += 1
    raise Bail


def _product(left, right):
    out = {a + b for a in left for b in right}
    if len(out) > MAX_EXPANSION:
        raise Bail
    return out


def _parse_alt(s, i=0):
    """Parse alternatives until an unmatched ')' or the end. Return (set, index)."""
    result, seq = set(), {""}
    while i < len(s):
        c = s[i]
        if c == ")":
            break
        if c == "|":
            result |= seq
            seq = {""}
            i += 1
            continue
        atom, i = _parse_atom(s, i)
        if atom == {" "} and i < len(s) and s[i] in "*+":
            # Whitespace: a single space is enough to separate words
            atom = atom | {""} if s[i] == "*" else atom
            i += 1
        elif i < len(s) and s[i] in "?*+{":
            if s[i] != "?":
                raise Bail
            atom = atom | {""}
            i += 1
            if i < len(s) and s[i] in "?+":
                i += 1
        elif i < len(s) and s[i] == "}":
            raise Bail
        seq = _product(seq, atom)
    return result | seq, i


def _parse_atom(s, i):
    c = s[i]
    if c == "\\":
        if i + 1 >= len(s):
            raise Bail
        n = s[i + 1]
        if n in "bBAzZG":
            return {""}, i + 2
        if n == "s":
            return {" "}, i + 2
        if n.isalnum():
            raise Bail  # \d, \w, \s, \1, \n, ...
        return {n}, i + 2
    if c == "(":
        if s.startswith("(?", i):
            if s[i + 2] in "=!" or s.startswith("(?<=", i) or s.startswith("(?<!", i):
                return {""}, _skip_group(s, i)
            if s.startswith("(?#", i):
                return {""}, _skip_group(s, i)
            ignore_case = s.startswith("(?i:", i)
            if s[i + 2] in ":>":
                inner = i + 3
            elif ignore_case:
                inner = i + 4
            else:
                raise Bail
        else:
            ignore_case, inner = False, i + 1
        res, j = _parse_alt(s, inner)
        if j >= len(s) or s[j] != ")":
            raise Bail
        if ignore_case:
            res = {x for w in res for x in (w.lower(), w.upper(), w.capitalize())}
        return res, j + 1
    if c == "[":
        j, chars = i + 1, set()
        if j < len(s) and s[j] == "^":
            raise Bail
        while j < len(s) and s[j] != "]":
            if s[j] == "\\":
                if j + 1 >= len(s) or s[j + 1].isalnum():
                    raise Bail
                chars.add(s[j + 1])
                j += 2
            elif j + 1 < len(s) and s[j + 1] == "-" and s[j + 2:j + 3] != "]":
                raise Bail
            elif s[j] == "[":
                raise Bail
            else:
                chars.add(s[j])
                j += 1
        if j >= len(s) or not chars or len(chars) > 8:
            raise Bail
        return chars, j + 1
    if c in "^$":
        return {""}, i + 1
    if c in ".*+?{}|":
        raise Bail
    return {c}, i + 1


def literals(regex):
    """Return the set of literal strings matched by `regex`, or raise Bail."""
    if regex.startswith("(?x)") or "(?x" in regex[:6]:
        raise Bail
    ignore_case = False
    m = re.match(r"\(\?i\)", regex)
    if m:
        ignore_case, regex = True, regex[m.end():]
    result, i = _parse_alt(regex)
    if i != len(regex):
        raise Bail
    result.discard("")
    if ignore_case:
        result = {x for w in result for x in (w.lower(), w.upper(), w.capitalize())}
    return result


def group_sources(regex):
    """Map capture group number -> source text of the group."""
    groups, stack, count, i, in_class = {0: regex}, [], 0, 0, False
    while i < len(regex):
        c = regex[i]
        if c == "\\":
            i += 2
            continue
        if in_class:
            in_class = c != "]"
        elif c == "[":
            in_class = True
        elif c == "(":
            if regex.startswith("(?", i):
                stack.append(None)
            else:
                count += 1
                stack.append((count, i + 1))
        elif c == ")" and stack:
            entry = stack.pop()
            if entry:
                groups[entry[0]] = regex[entry[1]:i]
        i += 1
    return groups


# ---------------------------------------------------------------------------
# Sublime syntax scanning
# ---------------------------------------------------------------------------


class Theme:
    """Colors of a `.tmTheme` file, with Sublime Text's scope selector matching."""

    def __init__(self, path):
        with open(path, "rb") as f:
            settings = plistlib.load(f)["settings"]
        self.rules = []
        for entry in settings:
            color = (entry.get("settings") or {}).get("foreground")
            if not color or "scope" not in entry:
                continue
            for selector in entry["scope"].split(","):
                if re.search(r"(^|\s)-|[()|&]", selector):
                    continue  # Exclusions and groups are not supported
                if selector.strip():
                    self.rules.append((selector.split(), color.lower()[:7]))

    @staticmethod
    def _matches(element, scope):
        return scope == element or scope.startswith(element + ".")

    def color(self, stack):
        """Color of the innermost scope of `stack` (outermost first), or None."""
        best, best_score = None, None
        for elements, color in self.rules:
            j, positions = len(stack), []
            for element in reversed(elements):
                j = next((k for k in range(j - 1, -1, -1) if self._matches(element, stack[k])), -1)
                if j < 0:
                    break
                positions.append(j)
            else:
                score = (positions[0], elements[-1].count("."), len(elements))
                if best_score is None or score >= best_score:
                    best, best_score = color, score
        return best


# Catppuccin color -> Kibi keyword list (the color of each list is defined in src/syntax.rs)
COLOR_KEYS = {
    "#cba6f7": "keywords_1",  # Mauve
    "#fab387": "keywords_2",  # Peach
    "#f38ba8": "builtins",  # Red
    "#f9e2af": "types",  # Yellow
    "#89b4fa": "functions",  # Blue
    "#94e2d5": "operators",  # Teal
}
LIST_KEYS = list(COLOR_KEYS.values())
TEXT = "#cdd6f4"  # Color of plain text
FLAG_KEYS = [
    "highlight_function_calls",
    "plain_path_calls",
    "plain_capitalized_calls",
    "highlight_operators",
    "highlight_punctuation",
    "highlight_capitalized",
    "highlight_commands",
    "type_definition_keywords",
    "function_definition_keywords",
    "highlight_parameters",
    "highlight_variables",
    "highlight_options",
    "negative_numbers",
    "docstrings",
    "decorators",
    "attributes",
    "keys",
    "key_separators",
    "key_separator",
    "unquoted_values",
    "sections",
    "highlight_markdown",
    "highlight_tags",
    "highlight_css",
    "highlight_units",
]
# Colors -> names of the highlight types accepted by the `keys`, `key_separator`... options
CATEGORIES = {**COLOR_KEYS, "#9399b2": "punctuation", "#a6e3a1": "strings", "#eba0ac": "parameters"}
# Old or rarely used languages that are not imported
EXCLUDED = {
    "php", "php source", "actionscript", "applescript", "asp", "html (asp)", "cfml", "coffeescript",
    "livescript", "java server page (jsp)", "nant build file", "nsis", "pascal", "fortran (fixed form)",
    "fortran (modern)", "fortran namelist", "ada", "tcl", "html (tcl)", "jcl", "textile", "ruby haml",
    "ruby slim", "stylus", "mediawiki ng", "rss", "processing", "raku", "sml", "d", "sass",
    # System and data files
    "cpuinfo", "meminfo", "syslog", "strace", "gosum", "separated values", "tab separated values",
    "requirements.txt", "passwd", "group", "hosts file", "dotenv", "debsources", "varlink", "fstab",
    "sshd config", "email", "lisp", "gitignore", "git attributes", "git commit", "git config",
    "git ignore", "git link", "git log", "git mailmap", "git rebase todo", "manpage", "cmake c++ header", "gdscript (godot engine)",
    "zsh", "nushell", "java properties", "vue component", "svelte", "batch file", "powershell", "less", "ocaml",
    "arm assembly", "x86_64 assembly", "glsl", "verilog", "systemverilog", "vhdl", "viml", "restructuredtext", "latex", "tex", "typst", "diff",
    # Duplicates, bat-specific formats, system files and niche languages
    "bourne again shell (bash)", "javascript (babel)", "html (rails)", "html (eex)", "html (erlang)", "html (jinja2)",
    "html (twig)", "javascript (rails)", "ruby on rails", "sql (rails)", "protocol buffer (text)", "ocamllex",
    "ocamlyacc", "cmake c header", "cmakecache", "highlight non-printables", "dir", "command help",
    "rd (r documentation)", "vimhelp", "man page (groff/troff)", "log", "http request and response", "regular expression",
    "authorized keys", "known hosts", "ssh config", "resolv", "crontab", "apache conf",
    "todo.txt", "salt state (sls)", "puppet", "robot framework", "cabal", "idris",
    "lean 4", "odin", "v", "vyper", "rego", "solidity",
    "wgsl", "gnuplot", "racket", "purescript", "crystal", "jsonnet",
    "jq", "nosql", "prql", "lrc", "bibtex", "asciidoc (asciidoctor)",
    "llvm", "qml", "ninja", "graphviz (dot)",
}
MERGED = "# Keyword colors merged with the bat syntaxes by scripts/import_bat_syntaxes.py"


class Extracted:
    def __init__(self):
        self.sl_comments = []
        self.ml_comments = []
        self.quotes = []
        self.ml_strings = []
        self.words = {}  # word -> scope stack of its first occurrence
        self.numbers = False

    def add(self, bucket, items):
        for item in items:
            if item not in bucket:
                bucket.append(item)


def is_scope(scope, prefix):
    return any(s.startswith(prefix) for s in (scope or "").split())


class Scanner:
    def __init__(self, docs):
        self.contexts, self.variables = {}, {}
        for doc in docs:
            for k, v in (doc.get("variables") or {}).items():
                self.variables[k] = str(v)
            for name, rules in (doc.get("contexts") or {}).items():
                self.contexts.setdefault(name, []).extend(rules or [])
        self.base = [str(docs[-1].get("scope") or "")]
        self.out = Extracted()

    def expand(self, regex):
        for _ in range(10):
            new = re.sub(r"\{\{(\w+)\}\}", lambda m: self.variables.get(m.group(1), m.group(0)), regex)
            if new == regex:
                break
            regex = new
        if "{{" in regex:
            raise Bail
        return regex

    def lits(self, regex):
        return literals(self.expand(str(regex)))

    def target_contexts(self, rule):
        """Rule lists of the contexts pushed (or set) by `rule`."""
        out = []
        for key in ("push", "set"):
            target = rule.get(key)
            if target is None:
                continue
            if isinstance(target, str):
                out.append(self.contexts.get(target, []))
            elif isinstance(target, list):
                if target and all(isinstance(t, str) for t in target):
                    out.extend(self.contexts.get(t, []) for t in target)
                else:
                    out.append(target)
        return out

    def run(self):
        for rules in list(self.contexts.values()):
            self.scan(rules)
        return self.out

    def scan(self, rules):
        self.punctuation(rules)
        for rule in rules:
            if not isinstance(rule, dict):
                continue
            for key in ("push", "set"):
                target = rule.get(key)
                if isinstance(target, list) and target and not all(isinstance(t, str) for t in target):
                    self.scan(target)
            if "match" in rule:
                try:
                    self.rule(rule)
                except Bail:
                    pass

    def punctuation(self, rules):
        """Comment delimiters declared through `punctuation.definition.comment.*`
        scopes, inside a single context."""
        found = {"begin": [], "end": [], "line": []}
        for rule in rules:
            if not isinstance(rule, dict) or "match" not in rule:
                continue
            for scope in str(rule.get("scope") or "").split():
                if not scope.startswith("punctuation.definition.comment"):
                    continue
                parts = scope.split(".")[3:]
                kind = "begin" if "begin" in parts else "end" if "end" in parts else "line"
                try:
                    toks = self.lits(rule["match"])
                except Bail:
                    continue
                if len(toks) == 1:
                    found[kind].append(next(iter(toks)))
        if found["begin"] and found["end"]:
            pair = f"{found['begin'][0]},{found['end'][0]}"
            if not re.search(r"\s", pair) and pair not in self.out.ml_comments:
                self.out.ml_comments.append(pair)
        elif found["line"] and not found["end"]:
            self.out.add(self.out.sl_comments, [t for t in found["line"] if len(t) <= 3 and not re.search(r"\s", t)])

    def rule(self, rule):
        scope = str(rule.get("scope") or "")
        targets = self.target_contexts(rule)
        meta = scope
        for t in targets:
            for r in t:
                if isinstance(r, dict):
                    meta += " " + str(r.get("meta_scope") or "") + " " + str(r.get("meta_content_scope") or "")
        if is_scope(scope, "constant.numeric") or is_scope(meta, "constant.numeric"):
            self.out.numbers = True

        # Comments and strings
        if targets and (is_scope(meta, "comment") or is_scope(meta, "string.quoted")):
            try:
                start = self.lits(rule["match"])
            except Bail:
                start = set()
            if len(start) == 1:
                start = next(iter(start))
                self.delimited(start, meta, targets)
        elif is_scope(scope, "comment.line"):
            self.line_comment(rule)

        # Words, with the scope stack used to find their color
        if scope and not is_scope(scope, "comment") and not is_scope(scope, "string"):
            self.words(self.lits(rule["match"]), self.base + scope.split())
        captures = rule.get("captures") or {}
        if captures and isinstance(captures, dict):
            groups = group_sources(self.expand(str(rule["match"])))
            for num, cscope in captures.items():
                cscope = str(cscope)
                if is_scope(cscope, "comment") or is_scope(cscope, "string") or int(num) not in groups:
                    continue
                try:
                    self.words(literals(groups[int(num)]), self.base + scope.split() + cscope.split())
                except Bail:
                    pass

    def line_comment(self, rule):
        """A rule like `match: '(?:^\\s+)?(#).*$'` with scope `comment.line`."""
        regex = self.expand(str(rule["match"]))
        captures = rule.get("captures") or {}
        groups = group_sources(regex)
        for num, cscope in captures.items() if isinstance(captures, dict) else []:
            if is_scope(str(cscope), "punctuation.definition.comment") and int(num) in groups:
                try:
                    toks = literals(groups[int(num)])
                except Bail:
                    continue
                self.out.add(self.out.sl_comments, sorted(t for t in toks if t and len(t) <= 3))
                return
        # Otherwise, use the literal prefix of the regex
        regex = re.sub(r"^(\(\?<[=!][^)]*\)|\(\?:[^()]*\)\?|\^|\\s\*|\\s\+)+", "", regex)
        m = re.match(r"((?:\\[^\w\s]|[^\\\[\](){}.*+?|^$\w\s]){1,3})", regex)
        if m:
            self.out.add(self.out.sl_comments, [re.sub(r"\\(.)", r"\1", m.group(1))])

    def words(self, words, stack):
        for w in sorted(w.strip() for w in words):
            if WORD.fullmatch(w):
                self.out.words.setdefault(w, stack)

    def delimited(self, start, meta, targets):
        if re.search(r"[,\s]", start):
            return
        if is_scope(meta, "comment.line"):
            self.out.add(self.out.sl_comments, [start])
        elif is_scope(meta, "comment"):
            end = self.end_token(targets)
            if end and not re.search(r"[,\s]", end):
                pair = f"{start},{end}"
                if pair not in self.out.ml_comments:
                    self.out.ml_comments.append(pair)
        elif is_scope(meta, "string.quoted"):
            if len(start) == 1 and start in "\"'`":
                self.out.add(self.out.quotes, [start])
            elif len(start) == 3 and start in ('"""', "'''", "```"):
                self.out.add(self.out.ml_strings, [start])

    def end_token(self, targets):
        for t in targets:
            for r in t:
                if isinstance(r, dict) and "match" in r and r.get("pop"):
                    try:
                        lits = self.lits(r["match"])
                    except Bail:
                        continue
                    if len(lits) == 1:
                        return next(iter(lits))
        return None


# ---------------------------------------------------------------------------
# Driver
# ---------------------------------------------------------------------------


def load(path):
    with open(path, encoding="utf8") as f:
        text = f.read()
    try:
        return yaml.load(text, Loader=Loader), text
    except yaml.YAMLError:
        # PyYAML rejects some flow mappings that ruamel accepts (e.g. Go, Erlang)
        from ruamel.yaml import YAML
        from ruamel.yaml.constructor import SafeConstructor

        SafeConstructor.add_constructor("tag:yaml.org,2002:value", lambda loader, node: "=")
        return YAML(typ="safe", pure=True).load(text), text


def read_ini(path):
    """Return (name, extensions, lines) for an INI syntax file."""
    name, exts, lines = "", [], path.read_text(encoding="utf8").splitlines()
    for line in lines:
        key, _, val = line.partition("=")
        if key.strip() == "name":
            name = val.strip()
        elif key.strip() == "extensions":
            exts += [e.strip() for e in val.split(",")]
    return name, exts, lines


def slug(name):
    return re.sub(r"[^a-z0-9]+", "-", name.lower()).strip("-") or "syntax"


def find_parent(doc, by_scope_path):
    ext = doc.get("extends")
    if isinstance(ext, str):
        ext = [ext]
    return [by_scope_path[Path(e).name] for e in ext or [] if Path(e).name in by_scope_path]


def collect_docs(doc, by_name, seen=None):
    seen = seen or set()
    docs = []
    for parent in find_parent(doc, by_name):
        if id(parent) not in seen:
            seen.add(id(parent))
            docs += collect_docs(parent, by_name, seen)
    return docs + [doc]


def doc_exts(doc):
    return [str(e) for e in (doc.get("file_extensions") or []) + (doc.get("hidden_file_extensions") or [])]


class Bat:
    """Ask bat how it colors some code, to check what the grammar does in practice."""

    def __init__(self, theme_name):
        self.theme_name = theme_name

    def tokens(self, lang, code):
        """Return the lines of (token, color) pairs for `code`, or None if bat fails."""
        cmd = ["bat", "--color=always", "--style=plain", "--paging=never", f"--theme={self.theme_name}", "-l", lang]
        try:
            res = subprocess.run(cmd, input=code, capture_output=True, text=True, check=True)
        except (OSError, subprocess.CalledProcessError):
            return None
        lines = []
        for out_line in res.stdout.splitlines():
            color, toks = None, []
            for part in re.split(r"(\x1b\[[0-9;]*m)", out_line):
                m = re.fullmatch(r"\x1b\[([0-9;]*)m", part)
                if m:
                    p = m.group(1).split(";")
                    if len(p) >= 5 and p[:2] == ["38", "2"]:
                        color = "#%02x%02x%02x" % tuple(map(int, p[2:5]))
                    elif p in (["0"], [""]):
                        color = None
                else:
                    toks += [(t, color) for t in re.findall(r"\w+|[^\w\s]", part)]
            lines.append(toks)
        return lines

    def word_colors(self, lang, words):
        """Color of each word, when alone on its line."""
        lines = self.tokens(lang, "".join(w + "\n" for w in words)) or []
        return {w: line[0][1] for w, line in zip(words, lines) if line}

    def definition_keywords(self, lang, keywords):
        """Keywords after which bat colors a name as a type (e.g. `struct Foo`) or as
        a function (e.g. `def foo`)."""
        snippets = ["{} Foo", "{} Foo {{}}", "{} foo", "{} foo(a, b)"]
        # "zork" is a control: the names must not be colored after any word
        words = ["zork", *keywords]
        code = "".join(snippet.format(k) + "\n" for k in words for snippet in snippets)
        lines = self.tokens(lang, code) or []
        lines += [[]] * (len(snippets) * len(words) - len(lines))
        found, params = {}, set()
        for n, k in enumerate(words):
            upper, upper_block, lower, call = (lines[n * len(snippets) + j] for j in range(len(snippets)))
            if ("Foo", "#f9e2af") in upper[1:2] + upper_block[1:2]:
                found[k] = "type"
            elif ("foo", "#89b4fa") in lower[1:2]:
                found[k] = "function"
            # Parameters, e.g. `a` in `def foo(a, b)`
            if ("a", "#eba0ac") in call:
                params.add(k)
        control = found.pop("zork", None)
        # Only a few keywords define functions (e.g. `def`); if most keywords are followed
        # by parameters, these are not parameters of definitions (e.g. F#)
        has_params = "zork" not in params and 0 < len(params) <= 3
        types = [k for k, v in found.items() if v == "type" and control != "type"]
        functions = [k for k, v in found.items() if v == "function" and control != "function"]
        functions += [k for k in keywords if has_params and k in params and k not in functions]
        out = []
        if types:
            out.append("type_definition_keywords=" + ", ".join(types))
        if functions:
            out.append("function_definition_keywords=" + ", ".join(functions))
        if has_params:
            out.append("highlight_parameters=true")
        return out

    def flags(self, lang):
        """Options deduced from how bat colors small snippets."""
        lines = self.tokens(lang, PROBE)
        if not lines:
            return []
        toks = [t for line in lines for t in line]
        color = lambda names: {c for t, c in toks if t in names}  # noqa: E731
        out = []
        # At the start of a line, some languages have keys (e.g. `foo` and `x` in TOML)
        line_start_call = ("foo", "#89b4fa") in lines[0] and ("x", "#89b4fa") not in lines[0]
        if line_start_call or ("foo", "#89b4fa") in lines[1] + lines[2]:
            out.append("highlight_function_calls=true")
            path_call, capitalized_call, *_ = (self.tokens(lang, "a::foo(x);\nFoo(x);\n") or []) + [[], []]
            if ("foo", "#89b4fa") not in path_call:
                out.append("plain_path_calls=true")
            if ("Foo", "#89b4fa") not in capitalized_call:
                out.append("plain_capitalized_calls=true")
        if "#94e2d5" in color({"=", "+", "."}):
            out.append("highlight_operators=true")
        if "#9399b2" in color({"(", ")", "{", "}", ";", ","}):
            out.append("highlight_punctuation=true")
        capitalized = [c for t, c in toks if t in {"Foo", "Bar", "Qux"}]
        if capitalized.count("#f9e2af") * 2 >= len(capitalized):
            out.append("highlight_capitalized=true")
        if ("zork", "#89b4fa") in (self.tokens(lang, "echo hi | zork arg\n") or [[]])[0]:
            out.append("highlight_commands=true")
        more = self.more_flags(lang)
        code = any(f.startswith(("highlight_function_calls", "highlight_commands")) for f in out)
        if code:
            # In code, `key: value` or `key = value` are labels, assignments or commands
            more = [f for f in more if not f.startswith(("keys=", "key_sep", "unquoted_values", "sections="))]
        if "highlight_operators=true" in out or "highlight_markdown=true" in more:
            # Tags would be confused with comparisons, e.g. `i<n`
            more = [f for f in more if f != "highlight_tags=true"]
        return out + more

    def separately(self, lang, snippets):
        """Tokens of each line of `snippets`, each line being colored on its own."""
        return [((self.tokens(lang, line + "\n") or [[]]) + [[]])[0] for line in snippets.splitlines()]

    def more_flags(self, lang):
        """Options for shells, configuration files, markup languages, decorators..."""
        shell, option, negative, doc, decorator, attribute, md, tag, css = self.separately(lang, PROBE_2)
        out = []
        if ("$", "#f38ba8") in shell:
            out.append("highlight_variables=true")
        if ("la", "#eba0ac") in option:
            out.append("highlight_options=true")
        if ("-", "#fab387") in negative:
            out.append("negative_numbers=true")
        if ("Doc", "#9399b2") in doc:
            out.append("docstrings=true")
        if ("property", "#fab387") in decorator:
            out.append("decorators=true")
        if ("derive", "#f9e2af") in attribute and ("#", "#fab387") in attribute:
            out.append("attributes=true")
        if md[:2] == [("#", "#9399b2"), ("Title", "#94e2d5")]:
            out.append("highlight_markdown=true")
        if ("div", "#89b4fa") in tag and ("class", "#f9e2af") in tag:
            out.append("highlight_tags=true")
        if ("a", "#f9e2af") in css and ("margin", "#89b4fa") in css:
            # CSS properties are keys, e.g. `color` in `color: red;`
            out += ["highlight_css=true", "keys=functions", "key_separators=:", "key_separator=operators"]
            return out + ["highlight_units=true"] * (("px", "#cba6f7") in css)
        return out + self.key_flags(lang)

    def key_flags(self, lang):
        """Options for the keys of configuration files, e.g. `name` in `name: value`."""
        snippets = 'key: value\nkey = value\n{"key": 1}\n[sec]\n'
        colon, equal, quoted, section = self.separately(lang, snippets)
        out, separators, key_type, sep_type, unquoted = [], [], None, None, False
        for line, sep in ((colon, ":"), (equal, "="), (quoted, ":")):
            colors = dict(line)
            key = CATEGORIES.get(colors.get("key") or "")
            # Teal keys are labels, e.g. `key:` in JavaScript
            if key and key not in ("strings", "punctuation", "operators"):
                key_type = key_type or key
                if sep not in separators:
                    separators.append(sep)
                sep_type = sep_type or CATEGORIES.get(colors.get(sep) or "")
                unquoted = unquoted or colors.get("value") == "#a6e3a1"
        if key_type:
            out += [f"keys={key_type}", "key_separators=" + ", ".join(separators)]
            if sep_type:
                out.append(f"key_separator={sep_type}")
            if unquoted:
                out.append("unquoted_values=true")
            section_type = CATEGORIES.get(dict(section).get("sec") or "")
            if section_type and section_type != "punctuation" and "=" in separators:
                out.append(f"sections={section_type}")
        return out


# Snippets used to deduce more options of a language (see Bat.more_flags), one per
# line, each line being colored separately
PROBE_2 = """echo "$HOME" $x
ls -la
x = -1;
\"\"\"Doc.\"\"\"
@property
#[derive(Debug)]
# Title
<div class="a">
.a { margin: 4px; }
"""

# Snippets used to deduce the options of a language (see Bat.flags)
PROBE = """foo(x, y);
y = foo(x) + 1;
{ x.foo(y); }
Foo x = Bar.baz;
x = Qux;
fn f() { let x: Foo = Bar::baz(Qux); }
zork arg
"""


def keyword_lines(assigned):
    """`assigned` maps word -> list key. Return the INI lines, in LIST_KEYS order."""
    lines = []
    for key in LIST_KEYS:
        words = [w for w, k in assigned.items() if k == key][:MAX_KEYWORDS]
        if words:
            lines.append(f"{key}=" + ", ".join(words))
    return lines


def bat_lists(ex, theme, bat, lang, extra=()):
    """word -> list key, or None if bat shows it as plain text.

    The color comes from the grammar scope of the word if the theme gives it a
    color, otherwise from bat when the word is alone on its line."""
    colors = {w: theme.color(stack) or TEXT for w, stack in ex.words.items()}
    unknown = [w for w in dict.fromkeys([*colors, *extra]) if colors.get(w, TEXT) == TEXT]
    for w, color in bat.word_colors(lang, unknown).items():
        if color in COLOR_KEYS or w in colors:
            colors[w] = color
    return {w: COLOR_KEYS.get(color or "") for w, color in colors.items()}


def merge_hand_written(path, lines, doc, ex, theme, bat):
    """Re-color the keywords of a hand-written file with the colors that bat uses."""
    original, kept = {}, []
    for line in lines:
        key, _, val = line.partition("=")
        if key.strip() in LIST_KEYS:
            for w in filter(None, (w.strip() for w in val.split(","))):
                original.setdefault(w, key.strip())
            # Drop the comment lines that described this list
            while kept and kept[-1].startswith(("#", ";")):
                kept.pop()
        elif key.strip() not in FLAG_KEYS and line != MERGED:
            kept.append(line)
    lang = bat_lang(doc)
    from_bat = bat_lists(ex, theme, bat, lang, extra=list(original))
    assigned = {}
    for w, key in original.items():
        if w not in from_bat:
            assigned[w] = key
        elif from_bat[w]:
            assigned[w] = from_bat[w]
    for w, key in from_bat.items():
        if key and w not in assigned:
            assigned[w] = key
    # Drop the comment lines that described the removed lists
    while kept and kept[-1].startswith(("#", ";")):
        kept.pop()
    kw1 = [w for w, k in assigned.items() if k == "keywords_1" and WORD.fullmatch(w)]
    out = kept + [MERGED, *bat.flags(lang), *bat.definition_keywords(lang, kw1), *keyword_lines(assigned)]
    path.write_text("\n".join(out) + "\n", encoding="utf8")


def bat_lang(doc):
    """Argument for `bat -l` that selects the syntax of `doc`."""
    return str(doc.get("name") or "")


def main(src, theme_path, dst):
    src, dst, theme = Path(src), Path(dst), Theme(theme_path)
    bat = Bat(Path(theme_path).stem)
    files = sorted(src.rglob("*.sublime-syntax"), key=lambda p: ("02_Extra" in p.parts, str(p)))
    docs, by_name = [], {}
    for f in files:
        try:
            doc, text = load(f)
        except Exception:  # noqa: BLE001
            print(f"skip {f.name}: unparsable YAML", file=sys.stderr)
            continue
        if not isinstance(doc, dict) or doc.get("hidden"):
            continue
        doc.setdefault("name", f.stem)  # Sublime Text uses the file name by default
        by_name[f.name] = doc
        docs.append((f, doc, text))

    # Hand-written files: keep them, but use the bat colors for their keywords
    taken_names, taken_exts, existing_files, merged = set(), set(), set(), []
    for ini in sorted(dst.glob("*.ini")):
        name, exts, lines = read_ini(ini)
        if lines and lines[0] == HEADER:
            ini.unlink()  # Generated file: regenerate it
            continue
        taken_names.add(name.lower())
        taken_exts |= set(exts)
        existing_files.add(ini.name)
        match = next(((d, t) for _, d, t in docs if set(exts) & set(doc_exts(d))), None)
        if match:
            doc, _ = match
            merge_hand_written(ini, lines, doc, Scanner(collect_docs(doc, by_name)).run(), theme, bat)
            merged.append(name)

    written = 0
    for f, doc, _ in docs:
        name = str(doc.get("name") or f.stem)
        exts = [e for e in dict.fromkeys(doc_exts(doc)) if e not in taken_exts and not re.search(r"[,\s/*]", e)]
        out_file = slug(name) + ".ini"
        if not exts or name.lower() in taken_names | EXCLUDED or out_file in existing_files:
            continue
        ex = Scanner(collect_docs(doc, by_name)).run()
        lines = [HEADER, f"name={name}", "extensions=" + ", ".join(exts)]
        if ex.numbers:
            lines.append("highlight_numbers=true")
        if ex.quotes:
            lines.append("singleline_string_quotes=" + ", ".join(ex.quotes))
        if ex.sl_comments:
            lines.append("singleline_comment_start=" + ", ".join(ex.sl_comments))
        if ex.ml_comments:
            lines.append("multiline_comment_delims=" + ex.ml_comments[0])
        if ex.ml_strings:
            lines.append("multiline_string_delim=" + ex.ml_strings[0])
        assigned = {w: k for w, k in bat_lists(ex, theme, bat, bat_lang(doc)).items() if k}
        lines += bat.flags(bat_lang(doc))
        lines += bat.definition_keywords(bat_lang(doc), [w for w, k in assigned.items() if k == "keywords_1"])
        lines += keyword_lines(assigned)
        (dst / out_file).write_text("\n".join(lines) + "\n", encoding="utf8")
        taken_names.add(name.lower())
        taken_exts |= set(exts)
        existing_files.add(out_file)
        written += 1
    print(f"Merged {len(merged)} hand-written files, wrote {written} generated files to {dst}")
    print("Hand-written files without a bat syntax:", file=sys.stderr)
    for ini in sorted(dst.glob("*.ini")):
        name, _, lines = read_ini(ini)
        if lines and lines[0] != HEADER and name not in merged:
            print(f"  {ini.name}", file=sys.stderr)


if __name__ == "__main__":
    if len(sys.argv) != 4:
        sys.exit(__doc__)
    main(*sys.argv[1:])
