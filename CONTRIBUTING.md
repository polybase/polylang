Thank you for taking the time to contribute to `Polylang`!

The following is a set of guidelines for contributing to `Polylang`. Remember that these are guidelines, not strict rules. So use your judgment, but
adhering to the guidelines as closely as possible will make the contribution process smoother.

## What can I contribute?

We welcome all kinds of contributions - bug reports, feature requests, or enhancement requests.

## How to contribute?

  1. Create a tracking issue in the `Polylang` repo here - https://github.com/polybase/polylang/issues.

  2. Fork the `Polylang` project, and create a clone from your fork.

  3. Make the code changes in a branch of your clone. Ensure that you test your changes locally.

  4. Create a Pull Request (PR) with your code changes linking the issue created in step 1.

## Setup

The `Polylang` repository is a modular `Cargo` [workspace](https://doc.rust-lang.org/cargo/reference/workspaces.html) comprising of the following packages with the [package
name](https://doc.rust-lang.org/cargo/reference/manifest.html#the-name-field) in parentheses:

  * parser (`polylang_parser`)
  * prover (`polylang-prover`)
  * abi (`abi`)
  * miden-run (`miden-run`)
  * error (`error`)
  * tests (`tests`)

in addition to the `Polylang` [root package](https://doc.rust-lang.org/cargo/reference/workspaces.html#root-package).

The first step is to fork the `Polylang` repository on Github, and clone your fork. Navigate to your clone:

```bash
$ cd polylang # this is your clone
```

Now build the project:

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

For instance, to run the tests for the `Polylang` prover:

```bash
$ cargo test -p polylang-prover
```