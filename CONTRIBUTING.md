# Contributing to `transit_model`

We welcomes contribution from everyone in the form of suggestions, bug
reports, pull requests, and feedback. This document gives some guidance if you
are thinking of helping us.

## Submitting bug reports and feature requests

When reporting a bug or asking for help, please include enough details so that
the people helping you can reproduce the behavior you are seeing. For some tips
on how to approach this, read about how to produce a [Minimal, Complete, and
Verifiable example].

[Minimal, Complete, and Verifiable example]: https://stackoverflow.com/help/mcve

When making a feature request, please make it clear what problem you intend to
solve with the feature, any ideas for how `transit_model` could support solving
that problem, any possible alternatives, and any disadvantages.

### Internal work management tool

At Kisio Digital (ex. CanalTP) we track tasks and bugs using a private tool.
This tool is private but we sometimes refer to it when submitting
PRs (those `Ref. ND-123`), to help later work.
Feel free to ask for more details if the description is too narrow,
we should be able to provide information from tracking tool if there is more.

## Checking quality

We encourage you to check that the formatting, static analysis and test suite
passes locally before submitting a pull request with your changes. If anything
does not pass, typically it will be easier to iterate and fix it locally than
waiting for the CI servers to run tests for you.

### Formatting

We use the standard Rust formatting tool, [`rustfmt`].

```sh
# To format the source code in the entire repository
cargo fmt --all
```

[`rustfmt`]: https://github.com/rust-lang/rustfmt

### Static analysis

For the static analysis, we use [`clippy`].

```sh
# To format the source code in the entire repository
cargo clippy --workspace --all-features --all-targets -- --warn clippy::cargo --allow clippy::multiple_crate_versions
```

[`clippy`]: https://github.com/rust-lang/rust-clippy

### Tests

The test suite include unit test and integration tests.

#### Test feature `xmllint`

`transit_model` is capable of exporting NeTEx France format.
Integration tests verify that the conversion produces files in accordance with
the NeTEx specification.

For that, tests are using the tool `xmllint` which can be installed on Debian
with the package `libxml2-utils`.\
Tests also depend on NeTEx specification that are imported as a git submodule.
Therefore, these tests are run only if feature `xmllint` is activated.\

To install xmllint and submodules:
```sh
git submodule update --init --recursive
apt install libxml2-utils
```

#### Check outputs manually

To validate the output NeTEx obtained it is possible to use xmllint:
```sh
xmllint --noout --nonet --huge --schema /path/to/NeTEx/xsd/NeTEx_publication.xsd your_file.xml
```

#### Launch all tests

```sh
# Run all the tests of `transit_model` in the entire repository,
# activating all features, including `xmllint`
cargo test --workspace --all-features
```

## Conduct

We follow the [Rust Code of Conduct].

[Rust Code of Conduct]: https://www.rust-lang.org/conduct.html
