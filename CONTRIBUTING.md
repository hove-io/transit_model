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

## Pull Request (PR)

If you feel to directly submit a PR, please ensure to explain your motivations
in your PR's description. See also the section below about updating versions of
crates.

### Update version

The PR owner is the best to understand which evolution and breaking changes are
introduced since the [last
release](https://crates.io/crates/transit_model/versions). Therefore, the PR
owner is in charge of updating the  version of the respective crates
accordingly, **if needed**. The idea is that, at any point in time, if someone
want to publish a release of the crate, this can be done right away, without
the need to modify any file. Do not hesitate to ask project's owner for guidance on
which version and how to do it.

### Internal work management tool

At Kisio Digital (ex. hove-io) we track tasks and bugs using a private tool.
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
make format
```

[`rustfmt`]: https://github.com/rust-lang/rustfmt

### Static analysis

For the static analysis, we use [`clippy`].

```sh
# Check lints on the source code in the entire repository
make lint
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

To validate the NeTEx output, it is possible to use `xmllint` using the official
XSD schema:

```sh
xmllint --noout --nonet --huge --schema /path/to/transit_model/tests/NeTEx/xsd/NeTEx_publication.xsd your_file.xml
```

Note: this may be very (very) slow on huge files.

#### Launch all tests

```sh
# Run all the tests of `transit_model` in the entire repository,
# activating all features (including `xmllint`), then without features
# to make sure that both work
make test
```

## Environments and tools

At Kisio Digital, we mostly maintain, test and operate on the following
environments and tools:

* Our main target for OS is [Debian].
* Our main target for [PROJ] is the version described in the
  [main README](README.md#PROJ-for-binaries).

However, we are open to contributions to help support more of them.

[Debian]: https://www.debian.org
[PROJ]: https://proj.org

## Conduct

We follow the [Rust Code of Conduct].

[Rust Code of Conduct]: https://www.rust-lang.org/conduct.html
