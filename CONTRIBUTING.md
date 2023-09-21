Thank you for taking the time to contribute to `Polylang`!

The following is a set of guidelines for contributing to `Polylang`. Remember that these are guidelines, not strict rules. So use your judgment, but
adhering to the guidelines as closely as possible will make the contribution process smoother.

## What can I contribute?

We welcome all kinds of contributions - bug reports, feature requests, or enhancement requests.

## Project Setup

### Project Structure

The `Polylang` repository is a `Cargo` [workspace](https://doc.rust-lang.org/cargo/reference/workspaces.html) comprising of the following packages with the [package
name](https://doc.rust-lang.org/cargo/reference/manifest.html#the-name-field) in parentheses:

  * parser (`polylang_parser`)
  * prover (`polylang-prover`)
  * abi (`abi`)
  * miden-run (`miden-run`)
  * error (`error`)
  * tests (`tests`)

in addition to the `Polylang` [root package](https://doc.rust-lang.org/cargo/reference/workspaces.html#root-package).

### Setup

Clone the `Polylang` repository:

```bash
$ https://github.com/polybase/polylang
```

Build the project:

```bash
$ cargo build
```

Or, for a release build:

```bash
$ cargo build --release
```

Run the tests to ensure that everything is working as expected. 

To run all tests:

```bash
$ cargo test --workspace --all-targets
```

To run tests for a specific package:

```bash
$ cargo test -p <package-name>
```

Note that the `<package-name>` must be the value of the `name` field in the package's `Cargo.toml` file, and **not** the name in the workspace's `members` list.

For instance, to run tests for the `Polylang` prover:

```bash
$ cargo test -p polylang-prover
```

## How to contribute?

### Style Guide
