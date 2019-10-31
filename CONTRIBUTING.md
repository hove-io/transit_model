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
cargo clippy --all
```

[`clippy`]: https://github.com/rust-lang/rust-clippy

### Tests
The test suite include unit test and integration tests.

```sh
# Run all the tests of `transit_model` in the entire repository
cargo test --all --all-features
```

## Conduct

We follow the [Rust Code of Conduct].

[Rust Code of Conduct]: https://www.rust-lang.org/conduct.html
