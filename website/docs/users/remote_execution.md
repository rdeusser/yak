---
id: remote_execution
title: Remote Execution
---

yak can use services that expose
[Bazel's remote execution API](https://github.com/bazelbuild/remote-apis) in
order to run actions remotely.

yak projects have been successfully tested for remote execution against
[EngFlow](https://www.engflow.com/),
[BuildBarn](https://github.com/buildbarn/bb-remote-execution) and
[BuildBuddy](https://www.buildbuddy.io). Sample project configurations for those
providers are available under
[examples/remote_execution](https://github.com/rdeusser/yak/tree/main/examples/remote_execution).

## RE configuration in `.yakconfig`

Configuration for remote execution can be found under `[yak_re_client]` in
`.yakconfig`.

Keys supported include:

- `engine_address` - address to your RE's engine. A remote cache without
  remote execution needs no engine, and yak then fetches the server's
  capabilities from `cas_address`.
- `action_cache_address` - address to your action cache endpoint.
- `cas_address` - address to your content-addressable storage (CAS) endpoint.
- `tls` - whether to connect with TLS. The default is `true`. A server that
  serves plaintext gRPC, such as a local bazel-remote, needs `tls = false`, and
  without it every request fails with `The service is currently unavailable`.
- `tls_ca_certs` - path to a CA certificates bundle. This must be PEM-encoded.
  If none is set, a default bundle will be used. This path contains environment
  variables using shell interpolation syntax (i.e. $VAR). They will be
  substituted before reading the file.
- `tls_client_cert` - path to a client certificate (and intermediate chain), as
  well as its associated private key. This must be PEM-encoded. This path can
  contain environment variables using shell interpolation syntax (i.e. $VAR).
  They will be substituted before reading the file.
- `http_headers` - HTTP headers to inject in all requests to RE. This is a
  comma-separated list of `Header: Value` pairs. Minimal validation of those
  headers is done here. This can contain environment variables using shell
  interpolation syntax ($VAR). They will be substituted before reading the file.
- `instance_name` - an instance name to pass on execution, action cache, and CAS
  requests.

yak uses `SHA256` for all its hashing by default. If your RE engine requires
something else, this can be configured in `.yakconfig` as follows:

```ini
[yak]
# Accepts BLAKE3, SHA1, or SHA256
digest_algorithms = BLAKE3
```

## RE platform configuration

Next, your build will need an
[execution platform](../concepts/glossary.md#execution-platform)
that specifies how and where actions should be executed. For a sample platform
definition that sets up an execution platform to utilize RE, take a look at the
[EngFlow example](https://github.com/rdeusser/yak/blob/main/examples/remote_execution/engflow/platforms/defs.bzl),
[BuildBarn example](https://github.com/rdeusser/yak/blob/main/examples/remote_execution/buildbarn/platforms/defs.bzl),
or the
[BuildBuddy example](https://github.com/rdeusser/yak/blob/main/examples/remote_execution/buildbuddy/platforms/defs.bzl).

To enable remote execution, configure the following fields in
[CommandExecutorConfig](../../api/build/CommandExecutorConfig)
as follows:

- `remote_enabled` - set to `True`.
- `local_enabled` - set to `True` if you also want to run actions locally.
- `use_limited_hybrid` - set to `False` unless you want to exclusively run
  remotely when possible.
- `remote_execution_properties` - other additional properties.
  - If the RE engine requires a container image, this can be done by setting
    `container-image` to an image URL, as is done in the example above.

## Remote cache without remote execution

A remote cache, such as
[bazel-remote](https://github.com/buchgr/bazel-remote), stores the results of
actions and tests that ran on one machine, so that other machines reuse them in
place of running them. It needs `action_cache_address` and `cas_address`, and
`tls = false` when the cache serves plaintext gRPC:

```ini
[yak_re_client]
  action_cache_address = grpc://cache.example.com:9092
  cas_address = grpc://cache.example.com:9092

[build]
  remote_cache = read_write
```

`[build] remote_cache` configures the remote cache of the default execution
platform, `prelude//platforms:default`, which `yak init` and `yak generate`
select:

- `off`, the default, uses no remote cache.
- `read` looks up the results of actions and of tests that support caching in
  the remote cache.
- `read_write` also uploads the passes of tests that support caching after they
  run locally, and the results of actions that set `allow_cache_upload = True`.

`[yak] default_allow_cache_upload = true` uploads the results of every build
action that runs locally, so that other machines skip its compile as well:

```ini
[build]
  remote_cache = read_write

[yak]
  default_allow_cache_upload = true
```

A CI job with this configuration and developer machines that set
`remote_cache = read` share the results of CI. `--upload-all-actions` serves
remote execution, which needs every action's inputs in the CAS, and does not
upload the results of local actions. A test that runs on a local executor of its own, as a test
without a remote execution profile does, uses the remote cache of its execution
platform. [Caching test results](../rule_authors/test_execution.md#caching-test-results)
describes which tests support caching.

The key of an action that runs a compiler of `system_toolchains` covers the
identity of that compiler, which the daemon computes from its version output
([Toolchains](../concepts/toolchain.md)). Machines with different versions of
`rustc`, `go`, or `clang` store and look up different results for the actions
that run them. The identity is that of the compiler on the machine that runs
`yak`, so it does not describe the compilers of remote execution workers.

