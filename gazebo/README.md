# Gazebo - a library of Rust utilities

This library contains a collection of well-tested utilities. Most modules stand
alone, but taking a few representative examples:

- `gazebo::prelude::*` is intended to be imported as such, and provides
  extension traits to common types. For example, it provides `Vec::map` which is
  equivalent to `iter().map(f).collect::<Vec<_>>()`, and `str::split1` like
  `split` but which only splits once. We hope some of these functions one day
  make it into the Rust standard library.
- `gazebo::dupe` provides the trait `Dupe` with the member `dupe`, all of which
  are exactly like `Clone`. The difference is that `Dupe` should not be
  implemented for types that reallocate or have expensive `clone` operations -
  e.g. there is `Dupe` for `Arc` and `usize`, but not for `String` and `Vec`. By
  using `dupe` it is easy to focus on the `clone` calls (which should be rare)
  and ignore things whose cost is minimal.
- `gazebo::cell::ARef` provides a type which is either a `Ref<T>` or a direct
  reference `&T`, with operations that make it look like `Ref` -- allowing you
  to uniformly convert a reference into something like a `Ref`.

The functionality provided by Gazebo is not stable, and continues to evolve with
both additions (as we find new useful features) and removals (as we find better
patterns or libraries encapsulating the ideas better). While the code varies in
usefulness and design quality, it is all well tested and documented.

## Using Gazebo

Gazebo can be depended upon by adding `gazebo` to your `[dependencies]`, using
the standard
[Cargo patterns](https://doc.rust-lang.org/cargo/reference/specifying-dependencies.html).

The two interesting directories in this repo are `gazebo` (which contains the
source to Gazebo itself) and `gazebo_derive` (which contains support for
`#[derive(Dupe)]` and other Gazebo traits). Usually you will directly import
`gazebo`, but `gazebo_derive` is a required transitive dependency if you are
sourcing the library from GitHub.

## License

Gazebo is both MIT and Apache License, Version 2.0 licensed, as found in the
[LICENSE-MIT](../LICENSE-MIT) and [LICENSE-APACHE](../LICENSE-APACHE) files.
