Buck2 logs all the commands it runs. So, after you've run a build, you can query
Buck2 to get access to the exact command it used.

To do so, do your build as normal, then run `yak log what-ran`.

## What Ran output format

This will output a table showing all the commands that were executed, and how
they were executed.

The structure is as follows:

```sh
REASON  <TAB> TARGET <TAB> IDENTIFIER <TAB> EXECUTOR <TAB> REPRODUCER
```

Which should be used as follows:

- REASON - value is either `build` (for building a thing) or `test` (for running
  a test).
- TARGET - the name of the build target that declared an action.
- IDENTIFIER - depends on the target but will usually be something like a file
  name or a module.
- EXECUTOR - value is either `cache`, `re` or `local`.
- REPRODUCER - how you can re-run this yourself.

## Using the What Ran output

Use What Ran as follows:

- Start by identifying the command you're looking for:
  - You can grep the output for a given target.
  - You can then grep by identifier if necessary. For example, if you're after
    C++ compilation, try grepping for the basename of your file (for example,
    for `src/my/stuff.cpp`, grep for `stuff.cpp`).
- Once you found it, reproduce as follows:
  - If the executor was `local`, the command is in the output, so just run it.
    It's expected that you'll do this from the root of your project (use
    `yak root --kind project` to find where that is).
  - If the executor was `re` or `cache`, you're provided a RE digest of the form
    `HASH:SIZE`.

## Examples

The following ran locally:

```bash
build  root//hello:hello (<unspecified>) (greeting)  local  env -C "$(yak root --kind project)" -- 'TMPDIR=/home/user/project/yak-out/v2/tmp/root/ba301b9fe7f0e990/greeting' 'BUCK_SCRATCH_PATH=yak-out/v2/tmp/root/ba301b9fe7f0e990/greeting' 'BUCK2_DAEMON_UUID=d02fef8c-7036-445f-a9e2-154afd2bb4f1' 'BUCK_BUILD_ID=a0e94d01-7a74-427e-bf45-5a0f4483322f' sh -c 'echo "$1" > "$2"' -- hello yak-out/v2/art/root/hello/__hello__/output_artifacts/greeting.txt
```

To repro, you'd run:

```bash
env -C "$(yak root --kind project)" -- 'TMPDIR=/home/user/project/yak-out/v2/tmp/root/ba301b9fe7f0e990/greeting' 'BUCK_SCRATCH_PATH=yak-out/v2/tmp/root/ba301b9fe7f0e990/greeting' 'BUCK2_DAEMON_UUID=d02fef8c-7036-445f-a9e2-154afd2bb4f1' 'BUCK_BUILD_ID=a0e94d01-7a74-427e-bf45-5a0f4483322f' sh -c 'echo "$1" > "$2"' -- hello yak-out/v2/art/root/hello/__hello__/output_artifacts/greeting.txt
```

The following ran on RE:

```bash
build  root//cpp:main (prelude//platforms:default#<hash>) (cxx_compile main.cpp)  re  <digest hash>:<size>
```

Reproducing this command will depend on the particular RE implementation you use.

## Expired Digests

Note that if the action was a cache hit on RE, you might get an error when
downloading it, indicating that it's not found. If that happens, it's because
the cache entry is there but the inputs have expired.

If this happens to you, run your build with `--upload-all-actions`.
