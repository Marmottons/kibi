#!/usr/bin/env bash
# SPDX-FileCopyrightText: 2020 Ilaï Deutel & Kibi Contributors
#
# SPDX-License-Identifier: MIT OR Apache-2.0

# Install the Kibi binary (to ~/.cargo/bin) and the syntax highlighting files
# (to $XDG_DATA_HOME/kibi/syntax.d, i.e. ~/.local/share/kibi/syntax.d by default).

set -euo pipefail

cd "$(dirname "${BASH_SOURCE[0]}")"

syntax_dir="${XDG_DATA_HOME:-$HOME/.local/share}/kibi/syntax.d"

cargo install --path . --locked

mkdir -p "$syntax_dir"
cp -f syntax.d/*.ini "$syntax_dir"/

echo "Installed kibi and $(ls syntax.d/*.ini | wc -l) syntax files in $syntax_dir"
