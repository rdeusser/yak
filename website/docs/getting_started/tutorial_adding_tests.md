---
id: tutorial_adding_tests
title: 'Tutorial: Adding tests'
---

In our previous tutorials, we built a binary `main` target and a `library`
target that uses it, even incorporating a shared `logging_lib`. Now, let's
ensure our `library` target works as expected by adding unit tests. Writing
tests helps us catch bugs early and refactor with confidence.

Our goal is to learn how to define and run Rust unit tests within the Buck2.

## What We'll Do:

1. Create a dedicated directory for our library's tests.
2. Write a simple unit test for the greet function in `greeter_lib`.
3. Update `greeter_lib/YAK` to define a test target using `rust_test`.
4. Run the tests using Buck2 and see the results.

## Prerequisites

- You should follow the [previous tutorial](../tutorial_adding_dependencies), we
  will start from the state as the previous tutorial ends.

## Step 1: Create a dedicated directory for our tests

1. Navigate to your `greeter_lib` directory:
2. Create the `tests` directory:

```bash
mkdir tests
```

Your `greeter_lib` structure should now look like this:

```
greeter_lib
├── YAK
├── src
│   └── lib.rs
└── tests
```

## Step 2: Writing the Unit Test

Now, let's write a simple test for our greet function.

1. New file `greeter_lib/tests/test.rs`, and edit it to look like this:

```rust
#[cfg(test)]
mod tests {
    use library;

    #[test]
    fn test_greet() {
        assert_eq!(library::greet("World"), "Hello, World!");
        assert_eq!(library::greet("Buck2"), "Hello, Buck2!");
    }

    #[test]
    fn test_greet_empty() {
        assert_eq!(library::greet(""), "Hello, !");
    }
}
```

## Step 3: Updating greeter_lib/YAK to Define the Test Target

Next, we need to tell Buck2 about our test file and how to run it.

1. Edit `greeter_lib/YAK`:

```python

...

# New test target for our unit tests
rust_test(
    name = "test",
    srcs = ["tests/test.rs"],
    deps = [
        # The test needs to depend on the library it's testing
        ":library",
    ]
)
```

Key additions and explanations:

- `rust_test(...)`:
  - `name = "test"`: We're naming our test target "test".
  - `srcs = ["tests/test.rs"]`: Specifies our test source file. Buck2 will
    compile this as a separate test binary.
  - `deps = [":library"]`: This is crucial. It tells Buck2 that our test code
    depends on the `:library` target (our `greeter_lib:library`). This makes
    `library` target available to be imported and used within test.rs.

## Step 4: Running Your Tests

With the YAK file updated, let's run our tests!

1. Navigate to the `greeter_lib` directory.
2. Run test using `yak test`:

```bash
yak test :test
```

- `yak test` is the command to run test targets.
- `:test` refers to the `rust_test` target named `test` that we defined in the
  current directory's `YAK` file.

3. Expected Output: You should see something like this:

```
...
Time elapsed: 6.5s
Tests finished: Pass 2. Fail 0. Fatal 0. Skip 0. Build failure 0
```

The key is seeing "Pass" and a summary indicating that all your test cases
(`test_greet` and `test_greet_empty`) passed.

## Conclusion

Congratulations! ✅

You've successfully added unit tests to your `library` target and run them using
Buck2!

We've learned how to:

- Define a test target using `rust_test` for a Rust library.
- Execute tests using `yak test` command.

Testing is a vital skill, and now you know how to integrate it into your Buck2
Rust workflow. This allows you to build more robust and reliable libraries and
applications.
