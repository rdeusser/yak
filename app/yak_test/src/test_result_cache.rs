/*
 * This source code is dual-licensed under either the MIT license found in the
 * LICENSE-MIT file in the root directory of this source tree or the Apache
 * License, Version 2.0 found in the LICENSE-APACHE file in the root directory
 * of this source tree. You may select, at your option, one of the
 * above-listed licenses.
 */

//! The local store of passing test results, which lets `yak test` report a test as passed without
//! running it again when its command and inputs are unchanged.
//!
//! Each entry is a directory named by its key under `yak-out/<isolation>/cache/test_results`,
//! holding the command's `stdout` and `stderr`. A daemon restart keeps the entries, and deleting
//! the directory empties the cache.

use std::io;
use std::sync::atomic::AtomicU64;
use std::sync::atomic::Ordering;

use yak_common::cas_digest::CasDigestConfig;
use yak_common::file_ops::metadata::FileDigest;
use yak_execute::execute::action_digest::ActionDigest;
use yak_execute::execute::environment_inheritance::EnvironmentInheritance;
use yak_fs::paths::abs_norm_path::AbsNormPathBuf;
use yak_fs::paths::file_name::FileName;

/// The largest stream that the store keeps. A test whose stdout or stderr is larger runs again.
const MAX_STREAM_BYTES: usize = 1 << 20;

/// TestResultCaching is what `yak test` does with stored results for one test command.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub(crate) enum TestResultCaching {
    /// The test's rule does not support caching.
    Off,
    /// The command runs, and a pass is stored, as under `--no-test-cache`.
    RecordOnly,
    /// A stored pass replaces running the command, and a new pass is stored.
    LookUpAndRecord,
}

impl TestResultCaching {
    pub(crate) fn new(supported: bool, lookups_disabled: bool) -> Self {
        match (supported, lookups_disabled) {
            (false, _) => Self::Off,
            (true, true) => Self::RecordOnly,
            (true, false) => Self::LookUpAndRecord,
        }
    }

    pub(crate) fn looks_up(self) -> bool {
        self == Self::LookUpAndRecord
    }

    pub(crate) fn records(self) -> bool {
        self != Self::Off
    }
}

/// TestResultKey identifies a test command and everything it reads: the action digest covers the
/// command line, the action's environment, and the digest of every input file, and the inherited
/// environment adds the values that a local run takes from the daemon's environment.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct TestResultKey(String);

impl TestResultKey {
    pub(crate) fn new(
        action: &ActionDigest,
        inherited_env: Option<&EnvironmentInheritance>,
        config: CasDigestConfig,
    ) -> Self {
        let mut material = action.to_string().into_bytes();
        if let Some(inherited_env) = inherited_env {
            let mut values: Vec<(&str, &std::ffi::OsString)> = inherited_env.values().collect();
            values.sort();
            for (key, value) in values {
                material.push(0);
                material.extend_from_slice(key.as_bytes());
                material.push(b'=');
                material.extend_from_slice(value.as_encoded_bytes());
            }
        }
        let digest = FileDigest::from_content(&material, config);
        Self(digest.to_string().replace(':', "-"))
    }
}

/// CachedPass is the output of a stored passing run.
#[derive(Debug, PartialEq, Eq)]
pub(crate) struct CachedPass {
    pub(crate) stdout: Vec<u8>,
    pub(crate) stderr: Vec<u8>,
}

/// TestResultCache is the store of passing results in one directory.
pub(crate) struct TestResultCache {
    dir: AbsNormPathBuf,
}

impl TestResultCache {
    pub(crate) fn new(dir: AbsNormPathBuf) -> Self {
        Self { dir }
    }

    fn entry(&self, key: &TestResultKey) -> AbsNormPathBuf {
        self.dir.join(FileName::unchecked_new(&key.0))
    }

    /// The stored pass of `key`, if there is one.
    pub(crate) async fn get(&self, key: &TestResultKey) -> io::Result<Option<CachedPass>> {
        let entry = self.entry(key);
        let stdout = match tokio::fs::read(entry.join(FileName::unchecked_new("stdout"))).await {
            Ok(stdout) => stdout,
            Err(e) if e.kind() == io::ErrorKind::NotFound => return Ok(None),
            Err(e) => return Err(e),
        };
        let stderr = tokio::fs::read(entry.join(FileName::unchecked_new("stderr"))).await?;
        Ok(Some(CachedPass { stdout, stderr }))
    }

    /// Stores a pass of `key` with its output. A stream over `MAX_STREAM_BYTES` stores nothing.
    pub(crate) async fn put(
        &self,
        key: &TestResultKey,
        stdout: &[u8],
        stderr: &[u8],
    ) -> io::Result<()> {
        if stdout.len() > MAX_STREAM_BYTES || stderr.len() > MAX_STREAM_BYTES {
            return Ok(());
        }
        // The entry appears whole, through a rename of a directory that holds both streams, so
        // a concurrent reader sees either no entry or a complete one.
        tokio::fs::create_dir_all(&self.dir).await?;
        // The staging name is unique to this write, so that concurrent writes of one key, from
        // this daemon or another, do not share a directory.
        static WRITES: AtomicU64 = AtomicU64::new(0);
        let staging = self.dir.join(FileName::unchecked_new(&format!(
            ".{}.{}.{}",
            key.0,
            std::process::id(),
            WRITES.fetch_add(1, Ordering::Relaxed)
        )));
        tokio::fs::create_dir_all(&staging).await?;
        tokio::fs::write(staging.join(FileName::unchecked_new("stdout")), stdout).await?;
        tokio::fs::write(staging.join(FileName::unchecked_new("stderr")), stderr).await?;
        match tokio::fs::rename(&staging, self.entry(key)).await {
            Ok(()) => Ok(()),
            // Another run stored the same pass first.
            Err(_) if tokio::fs::try_exists(self.entry(key)).await? => {
                tokio::fs::remove_dir_all(&staging).await
            }
            Err(e) => Err(e),
        }
    }
}

#[cfg(test)]
mod tests {
    use yak_execute::digest_config::DigestConfig;

    use super::*;

    #[test]
    fn caching_follows_support_and_the_flag() {
        assert_eq!(TestResultCaching::new(false, false), TestResultCaching::Off);
        assert_eq!(
            TestResultCaching::new(true, true),
            TestResultCaching::RecordOnly
        );
        assert!(TestResultCaching::new(true, false).looks_up());
        assert!(!TestResultCaching::new(true, true).looks_up());
        assert!(TestResultCaching::new(true, true).records());
        assert!(!TestResultCaching::Off.records());
    }

    #[tokio::test]
    async fn a_stored_pass_comes_back() {
        let config = DigestConfig::testing_default();
        let action = ActionDigest::from_content(b"action", config.cas_digest_config());
        let key = TestResultKey::new(&action, None, config.cas_digest_config());
        let other = TestResultKey::new(
            &ActionDigest::from_content(b"other", config.cas_digest_config()),
            None,
            config.cas_digest_config(),
        );

        let dir = tempfile::tempdir().unwrap();
        let cache = TestResultCache::new(AbsNormPathBuf::new(dir.path().to_owned()).unwrap());
        assert_eq!(cache.get(&key).await.unwrap(), None);
        cache.put(&key, b"ok\n", b"").await.unwrap();
        assert_eq!(
            cache.get(&key).await.unwrap(),
            Some(CachedPass {
                stdout: b"ok\n".to_vec(),
                stderr: Vec::new(),
            })
        );
        assert_eq!(cache.get(&other).await.unwrap(), None);
        // A second pass of the same command keeps the entry.
        cache.put(&key, b"ok\n", b"").await.unwrap();

        let large = vec![b'x'; MAX_STREAM_BYTES + 1];
        cache.put(&other, &large, b"").await.unwrap();
        assert_eq!(cache.get(&other).await.unwrap(), None);
    }
}
