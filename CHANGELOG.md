# Changelog
All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/), and this project
adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

The crates [`aurum_actors`] and [`aurum_actors_macros`] will always have the same version number.
Changes to both are listed here.

## Unreleased
### Breaking
- `unify!` has been completely revamped into something more readable and extendable. It now has
decent error messages.

### Added
- `core::Host` can now be converted from any `ToString` implementor instead of just `String`.

[`aurum_actors`]: https://crates.io/crates/aurum_actors
[`aurum_actors_macros`]: https://crates.io/crates/aurum_actors_macros