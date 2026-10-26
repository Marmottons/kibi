<!--
SPDX-FileCopyrightText: 2020 Ilaï Deutel & Kibi Contributors
SPDX-License-Identifier: MIT or Apache-2.0
-->

# Contributing to Kibi

We welcome contributions from everyone. Whether you are fixing a bug, improving
documentation, or proposing a new feature, your help is appreciated.

## Table of contents

- [Reporting issues](#reporting-issues)
- [Development Environment Setup](#development-environment-setup)
  - [Install Rust](#install-rust)
  - [Clone the Repository](#clone-the-repository)
  - [Run Kibi](#run-kibi)
- [Adding tests](#adding-tests)
  - [For new features](#for-new-features)
  - [For bug fixes](#for-bug-fixes)
- [Verifying Your Changes](#verifying-your-changes)
  - [Run Tests](#run-tests)
  - [Format Code](#format-code)
  - [Run Linters (Nightly)](#run-linters-nightly)
- [Syntax Highlighting Files](#syntax-highlighting-files)
- [Dependency Policy](#dependency-policy)
- [Submitting a Pull Request](#submitting-a-pull-request)
- [License](#license)

## Reporting issues

If you find a bug or have an idea for a new feature, please [open an
issue](https://github.com/Marmottons/kibi/issues/new). For bugs, provide as much
detail as possible, including steps to reproduce the issue and the expected
behavior.

## Development Environment Setup

### Install Rust

To develop on Kibi, you will need [Rust](https://rust-lang.org/tools/install) installed.

While Kibi builds on the **stable** toolchain, we require the **nightly**
toolchain for formatting and linting to ensure consistency across environments.

Install the nightly toolchain:

```bash
rustup toolchain install nightly --component rustfmt --component clippy --allow-downgrade
```

### Clone the Repository

```bash
git clone https://github.com/Marmottons/kibi.git
cd kibi
```

### Run Kibi

Kibi can be run with `cargo`:

```bash
# Open Kibi with an empty buffer
cargo run

# Open a file with Kibi
cargo run -- [file_path]

# Show current version
cargo run -- --version
```

## Adding tests

### For new features

If your change introduces a new feature, please ensure it is appropriately tested:

- Functions can typically be tested using **unit testing** within the source
  file (see [documentation](https://doc.rust-lang.org/book/ch11-03-test-organization.html#unit-tests))
- CLI changes can be tested using **integration tests**  (see
  [documentation](https://doc.rust-lang.org/book/ch11-03-test-organization.html#integration-tests)).
  Integration tests are located within the [`tests/`](tests/) directory.

### For bug fixes

Bug fixes should almost always be accompanied with a [regression
test](https://en.wikipedia.org/wiki/Regression_testing). This can be a unit test
(or, more rarely, an integration test) that should pass with your new changes
but would have failed beforehand. This ensures similar faults cannot re-emerge
without being detected in the future.

## Verifying Your Changes

Before submitting a Pull Request, please run the following checks.

### Run Tests

Ensure all unit tests pass.

```bash
cargo test
```

### Format Code

We use [`rustfmt`](https://github.com/rust-lang/rustfmt) with nightly features
to keep the codebase clean and compact.

```bash
cargo +nightly fmt
```

### Run Linters (Nightly)

We use [`clippy`](https://github.com/rust-lang/rust-clippy) to catch common
mistakes and enforce idiomatic Rust.

```bash
cargo +nightly clippy
```

## Syntax Highlighting Files

Most files in [`syntax.d`](syntax.d) are generated from the syntaxes and the
Catppuccin Mocha theme of [bat](https://github.com/sharkdp/bat) by
[`scripts/import_bat_syntaxes.py`](scripts/import_bat_syntaxes.py) (see the
instructions at the top of the script). Regenerating them requires bat, Python,
`pyyaml` and `ruamel.yaml`; using Kibi does not.

## Dependency Policy

Kibi aims to have minimal dependencies.

- **Production Dependencies:** Do **not** add new dependencies to Kibi without
explicitly discussing it in an issue first. Most features should be implemented
using the Rust standard library.
- **Dev Dependencies:** Adding dependencies for testing or development tools
(e.g. within `[dev-dependencies]`) is generally acceptable, provided they are
justified.

## Submitting a Pull Request

Changes to code for Kibi are made through Pull Requests on GitHub.

> [!TIP]
> If you are new to contributing to an open source project on GitHub, you can
> refer to the [_Creating a pull request_](https://docs.github.com/en/pull-requests/collaborating-with-pull-requests/proposing-changes-to-your-work-with-pull-requests/creating-a-pull-request)
> article in the GitHub documentation.

1. **Branching:** Create a new branch for your work. We don't enforce a naming
   convention, but descriptive names are helpful.
2. **Commits:** Similarly, we do not enforce a convention for commit messages,
   but please ensure they are descriptive enough.
3. **Push & Open PR:** Push your branch to GitHub and open a Pull Request.
4. **Description:** Fill out the PR description clearly, and make sure that the
   checks of the [Verifying Your Changes](#verifying-your-changes) section pass.

## License

Any contribution submitted for inclusion in Kibi by you shall be dual licensed under:

- [Apache License, Version 2.0](LICENSE-APACHE)
- [MIT License](LICENSE-MIT)

without any additional terms or conditions.
