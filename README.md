# Untest, remove tests from Rust projects

Untest is a tool designed to simplify the process of removing test code from Rust projects.

## Usage

Say you have a project in `my-project`. You can run the following to get a version of it with Rust tests removed in a new directory `my-project-minus-tests`.

```shell
untest my-project my-project-minus-tests
```

You can also explicitly exclude more things with one or more `--exclude` options, such as directories or files which might be part of a private test suite.

```shell
untest my-project my-project-minus-tests \
  --exclude /.github/workflows/private-tests.yaml \
  --exclude /tests
```

The `.git` directory is excluded by default, it is not copied to the output directory.

## How it works

It parses your Rust code with the Rust parser from the [`syn` crate](https://crates.io/crates/syn) and makes copies of it without the blocks that are subject to `#[cfg(test)]`, `#[test]`, etc.

## License

The source code is released under the Apache 2.0 license.
