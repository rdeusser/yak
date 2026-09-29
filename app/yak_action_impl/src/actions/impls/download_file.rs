/*
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is dual-licensed under either the MIT license found in the
 * LICENSE-MIT file in the root directory of this source tree or the Apache
 * License, Version 2.0 found in the LICENSE-APACHE file in the root directory
 * of this source tree. You may select, at your option, one of the
 * above-listed licenses.
 */

use std::borrow::Cow;
use std::sync::Arc;

use allocative::Allocative;
use async_trait::async_trait;
use dupe::Dupe;
use jiff::SignedDuration;
use jiff::Timestamp;
use pagable::Pagable;
use pagable::pagable_typetag;
use starlark::values::OwnedFrozen;
use starlark::values::Value;
use yak_artifact::artifact::build_artifact::BuildArtifact;
use yak_build_api::actions::Action;
use yak_build_api::actions::ActionExecutionCtx;
use yak_build_api::actions::UnregisteredAction;
use yak_build_api::actions::errors::execute_error::ExecuteError;
use yak_build_api::actions::execute::action_executor::ActionExecutionKind;
use yak_build_api::actions::execute::action_executor::ActionExecutionMetadata;
use yak_build_api::actions::execute::action_executor::ActionOutputs;
use yak_build_api::artifact_groups::ArtifactGroup;
use yak_build_signals::env::WaitingData;
use yak_common::cas_digest::RawDigest;
use yak_common::file_ops::metadata::FileDigest;
use yak_common::file_ops::metadata::FileMetadata;
use yak_common::file_ops::metadata::TrackedFileDigest;
use yak_common::io::trace::TracingIoProvider;
use yak_core::category::CategoryRef;
use yak_core::fs::project_rel_path::ProjectRelativePathBuf;
use yak_core::fs::yak_out_path::BuildArtifactPath;
use yak_error::ErrorTag;
use yak_error::YakErrorContext;
use yak_error::conversion::from_any_with_tag;
use yak_error::yak_error;
use yak_execute::artifact_value::ArtifactValue;
use yak_execute::digest::CasDigestToReExt;
use yak_execute::digest_config::DigestConfig;
use yak_execute::execute::clean_output_paths::CleanOutputPaths;
use yak_execute::execute::command_executor::ActionExecutionTimingData;
use yak_execute::materialize::http::Checksum;
use yak_execute::materialize::http::http_download;
use yak_execute::materialize::http::http_head;
use yak_execute::materialize::materializer::CasDownloadInfo;
use yak_execute::materialize::materializer::DeclareArtifactPayload;
use yak_execute::materialize::materializer::DeclareMatchOutcome;
use yak_hash::YakIndexSet;
use yak_http::HttpClient;

use crate::actions::impls::offline;

#[derive(Debug, yak_error::Error)]
#[yak(tag = Input)]
enum DownloadFileActionError {
    #[error("Exactly one output file must be specified for a download file action, got {0}")]
    WrongNumberOfOutputs(usize),
    #[error(
        "Downloads using content-based path {0} must supply metadata (usually in the form of a sha1)!"
    )]
    ContentBasedPathWithoutMetadata(BuildArtifactPath),
}

/// Minimum remaining CAS TTL for a probe hit to be declared as a CAS download instead of
/// fetching the content. Nothing but the CAS backs such a declaration, so the blob has to
/// survive until the TTL refresher adopts it.
// FIXME(materializer): This has to exceed the refresher's pass interval plus the remaining TTL
// below which it extends, which should be a static assertion against the refresher's constants
// rather than a number chosen here. Do that once the standalone refresher has replaced the
// materializer's and there is one set of constants to assert against.
const PROBE_MIN_REMAINING_TTL: SignedDuration = SignedDuration::from_hours(2);

enum DeclaredMetadata {
    /// The file's digest is known without downloading it.
    Known(FileMetadata),
    /// No checksum the digest config accepts, or no size: only a download can tell.
    Unknown,
    /// The size had to come from a HEAD request, and that failed.
    HeadFailed(yak_error::Error),
}

#[derive(Debug, Allocative, Pagable)]
pub(crate) struct UnregisteredDownloadFileAction {
    checksum: Checksum,
    size_bytes: Option<u64>,
    url: Arc<str>,
    is_executable: bool,
}

impl UnregisteredDownloadFileAction {
    pub(crate) fn new(
        checksum: Checksum,
        size_bytes: Option<u64>,
        url: Arc<str>,
        is_executable: bool,
    ) -> Self {
        Self {
            checksum,
            size_bytes,
            url,
            is_executable,
        }
    }
}

impl UnregisteredAction for UnregisteredDownloadFileAction {
    fn register(
        self: Box<Self>,
        outputs: YakIndexSet<BuildArtifact>,
        _starlark_data: Option<OwnedFrozen<Value<'static>>>,
        _error_handler: Option<OwnedFrozen<Value<'static>>>,
    ) -> yak_error::Result<Box<dyn Action>> {
        Ok(Box::new(DownloadFileAction::new(outputs, *self)?))
    }
}

#[derive(Debug, Allocative, Pagable)]
struct DownloadFileAction {
    outputs: Box<[BuildArtifact]>,
    inner: UnregisteredDownloadFileAction,
}

impl DownloadFileAction {
    fn new(
        outputs: YakIndexSet<BuildArtifact>,
        inner: UnregisteredDownloadFileAction,
    ) -> yak_error::Result<Self> {
        if outputs.len() != 1 {
            Err(DownloadFileActionError::WrongNumberOfOutputs(outputs.len()).into())
        } else {
            Ok(Self {
                outputs: outputs.into_iter().collect(),
                inner,
            })
        }
    }

    fn output(&self) -> &BuildArtifact {
        self.outputs
            .iter()
            .next()
            .expect("a single artifact by construction")
    }

    /// Works out what the downloaded file's metadata will be without downloading it.
    async fn declared_metadata(
        &self,
        client: &HttpClient,
        digest_config: DigestConfig,
    ) -> DeclaredMetadata {
        let digest = if digest_config.cas_digest_config().allows_sha1() {
            self.inner
                .checksum
                .sha1()
                .and_then(|sha1| RawDigest::parse_sha1(sha1.as_bytes()).ok())
        } else if digest_config.cas_digest_config().allows_sha256() {
            self.inner
                .checksum
                .sha256()
                .and_then(|sha256| RawDigest::parse_sha256(sha256.as_bytes()).ok())
        } else {
            None
        };

        let digest = match digest {
            Some(digest) => digest,
            None => return DeclaredMetadata::Unknown,
        };

        let size = match self.inner.size_bytes {
            Some(s) => Some(s),
            None => match self.head_content_length(client).await {
                Ok(size) => size,
                Err(e) => return DeclaredMetadata::HeadFailed(e),
            },
        };

        match size {
            Some(size) => {
                let digest = TrackedFileDigest::new(
                    FileDigest::new(digest, size),
                    digest_config.cas_digest_config(),
                );
                DeclaredMetadata::Known(FileMetadata {
                    digest,
                    is_executable: self.inner.is_executable,
                })
            }
            None => DeclaredMetadata::Unknown,
        }
    }

    /// The `Content-Length` a HEAD request reports for the URL, if the server sends one.
    async fn head_content_length(&self, client: &HttpClient) -> yak_error::Result<Option<u64>> {
        let url = &self.inner.url;
        let head = http_head(client, url)
            .await
            .map_err(|e| e.tag([ErrorTag::DownloadFileHeadRequest]))?;

        head.headers()
            .get(http::header::CONTENT_LENGTH)
            .map(|content_length| {
                let content_length = content_length
                    .to_str()
                    .map_err(|e| from_any_with_tag(e, yak_error::ErrorTag::Http))
                    .yak_error_context("Header is not valid utf-8")?;
                let content_length_number =
                    content_length.parse().with_yak_error_context(|| {
                        format!("Header is not a number: `{content_length}`")
                    })?;
                yak_error::Ok(content_length_number)
            })
            .transpose()
            .with_yak_error_context(|| {
                format!(
                    "Request to `{}` returned an invalid `{}` header",
                    url,
                    http::header::CONTENT_LENGTH
                )
            })
    }

    fn output_path(
        &self,
        ctx: &dyn ActionExecutionCtx,
        value: &ArtifactValue,
    ) -> yak_error::Result<ProjectRelativePathBuf> {
        let path = self.output().get_path();
        ctx.fs().resolve_build(
            path,
            path.is_content_based_path()
                .then(|| value.content_based_path_hash())
                .as_ref(),
        )
    }

    /// Declares the output as a CAS download when the CAS already holds the content, which
    /// leaves nothing to fetch from the origin server or to write to disk now. `None` when the
    /// content is not there, is about to expire, or there is no CAS to ask.
    async fn declare_from_cas(
        &self,
        ctx: &dyn ActionExecutionCtx,
        metadata: &FileMetadata,
    ) -> yak_error::Result<Option<ArtifactValue>> {
        if !ctx.cas_configured() {
            return Ok(None);
        }
        let use_case = ctx.invocation_re_use_case();
        let info = Arc::new(CasDownloadInfo::new_probed(use_case));
        let digest = metadata.digest.data();

        let expiration = match ctx
            .re_client()
            .with_use_case(use_case)
            .get_digest_expirations(vec![digest.to_re()], &info)
            .await
        {
            Ok(expirations) => match expirations.into_iter().next() {
                Some((_, expiration)) => expiration,
                None => return Ok(None),
            },
            Err(e) => {
                // Falling back to the download is correct for this action, but a CAS that cannot
                // answer this query would silently turn every download into an origin fetch,
                // which should be visible somewhere.
                let _ignored = yak_core::soft_error!(
                    "download_file_cas_probe_failed",
                    e.context(format!("CAS lookup for `{digest}` failed; downloading instead")),
                    quiet: true
                );
                return Ok(None);
            }
        };

        if expiration < Timestamp::now() + PROBE_MIN_REMAINING_TTL {
            return Ok(None);
        }

        let value = ArtifactValue::file(FileMetadata {
            digest: TrackedFileDigest::new_expires(
                digest.dupe(),
                expiration,
                ctx.digest_config().cas_digest_config(),
            ),
            is_executable: metadata.is_executable,
        });
        let path = self.output_path(ctx, &value)?;
        ctx.materializer()
            .declare_cas_many(
                info,
                vec![DeclareArtifactPayload {
                    path,
                    artifact: value.dupe(),
                }],
            )
            .await?;

        Ok(Some(value))
    }

    /// Puts the content at the output path now, unless the materializer already has exactly this
    /// content there from an earlier daemon.
    async fn download(
        &self,
        ctx: &dyn ActionExecutionCtx,
        client: &HttpClient,
        url: &str,
        metadata: Option<&FileMetadata>,
    ) -> yak_error::Result<(ArtifactValue, ActionExecutionKind)> {
        let materializer = ctx.materializer();

        let path = match metadata {
            Some(metadata) => {
                let value = ArtifactValue::file(metadata.dupe());
                let path = self.output_path(ctx, &value)?;
                if let DeclareMatchOutcome::Match = materializer
                    .declare_match(vec![(path.clone(), value.dupe())])
                    .await?
                {
                    return Ok((value, ActionExecutionKind::Simple));
                }
                path
            }
            None => ctx.fs().resolve_build(self.output().get_path(), None)?,
        };

        // Whatever is at the path is stale or untracked; the `declare_existing` below replaces
        // the materializer's record of it, so the disk has to be cleared to match. The lease is
        // held until that report is in.
        let output_lease = materializer.prepare_outputs(vec![path.clone()]).await?;
        ctx.blocking_executor()
            .execute_io(
                Box::new(CleanOutputPaths {
                    paths: vec![path.clone()],
                }),
                ctx.cancellation_context(),
            )
            .await?;

        let digest = http_download(
            client,
            ctx.fs().fs(),
            ctx.digest_config(),
            &path,
            url,
            &self.inner.checksum,
            self.inner.is_executable,
        )
        .await?;

        // RE knows this file by the digest (checksum, size), where the size came from
        // `size_bytes` or the HEAD response rather than from the content. A wrong size would leave
        // every remote consumer unable to find its input, so it is caught here.
        if let Some(metadata) = metadata
            && digest.size() != metadata.digest.size()
        {
            return Err(yak_error!(
                ErrorTag::DownloadSizeMismatch,
                "Downloaded size ({}) does not match expected size ({})",
                digest.size(),
                metadata.digest.size(),
            ));
        }

        // The declared metadata, when there is any, is the value: the checksum verified the
        // content and the size was checked above, and a content-based output path was resolved
        // from it. The digest the download computed is under the digest config's preferred
        // algorithm, which need not be the checksum's, so it only stands in when nothing was
        // declared.
        let value = match metadata {
            Some(metadata) => ArtifactValue::file(metadata.dupe()),
            None => ArtifactValue::file(FileMetadata {
                digest,
                is_executable: self.inner.is_executable,
            }),
        };
        materializer
            .declare_existing(
                &output_lease,
                vec![DeclareArtifactPayload {
                    path,
                    artifact: value.dupe(),
                }],
            )
            .await?;

        Ok((value, ActionExecutionKind::Simple))
    }

    /// Execute this action for offline builds (e.g. no network).
    async fn execute_for_offline(
        &self,
        ctx: &mut dyn ActionExecutionCtx,
    ) -> yak_error::Result<(ActionOutputs, ActionExecutionMetadata)> {
        let outputs = offline::declare_copy_from_offline_cache(ctx, &[self.output()]).await?;

        Ok((
            outputs,
            ActionExecutionMetadata {
                dep_file_db_writes_queued: 0,
                execution_kind: ActionExecutionKind::Simple,
                timing: ActionExecutionTimingData::default(),
                input_files_bytes: None,
                waiting_data: WaitingData::new(),
            },
        ))
    }
}

#[pagable_typetag]
#[async_trait]
impl Action for DownloadFileAction {
    fn kind(&self) -> yak_data::ActionKind {
        yak_data::ActionKind::DownloadFile
    }

    fn inputs(&self) -> yak_error::Result<Cow<'_, [ArtifactGroup]>> {
        Ok(Cow::Borrowed(&[]))
    }

    fn outputs(&self) -> Cow<'_, [BuildArtifact]> {
        Cow::Borrowed(&self.outputs)
    }

    fn first_output(&self) -> &BuildArtifact {
        self.output()
    }

    fn category(&self) -> CategoryRef<'_> {
        CategoryRef::unchecked_new("download_file")
    }

    fn identifier(&self) -> Option<&str> {
        self.outputs
            .iter()
            .next()
            .map(|o| o.get_path().path().as_str())
    }

    async fn execute(
        &self,
        ctx: &mut dyn ActionExecutionCtx,
        waiting_data: WaitingData,
    ) -> Result<(ActionOutputs, ActionExecutionMetadata), ExecuteError> {
        // Early return - if this path exists, it's because we're running in a
        // special offline mode where the HEAD request below will likely fail.
        // Shortcut and just return this path as the action output.
        //
        // This mostly looks like a "copy" action.
        if ctx.run_action_knobs().use_network_action_output_cache {
            return self.execute_for_offline(ctx).await.map_err(Into::into);
        }

        let client = ctx.http_client();
        let url = &self.inner.url;
        let is_content_based = self.output().get_path().is_content_based_path();

        let metadata = match self.declared_metadata(client, ctx.digest_config()).await {
            DeclaredMetadata::Known(metadata) => Some(metadata),
            DeclaredMetadata::Unknown => None,
            DeclaredMetadata::HeadFailed(e) => {
                if is_content_based {
                    return Err(e.into());
                }
                // Not a soft error: servers legitimately refuse HEAD, and a server that is
                // actually down fails the GET that follows.
                tracing::debug!(
                    "HEAD request for `{}` failed, downloading instead: {:#}",
                    url,
                    e
                );
                None
            }
        };
        if metadata.is_none() && is_content_based {
            return Err(ExecuteError::Error {
                error: DownloadFileActionError::ContentBasedPathWithoutMetadata(
                    self.output().get_path().dupe(),
                )
                .into(),
            });
        }

        let (value, execution_kind) = match &metadata {
            Some(metadata) => match self.declare_from_cas(ctx, metadata).await? {
                Some(value) => (value, ActionExecutionKind::Deferred),
                None => self.download(ctx, client, url, Some(metadata)).await?,
            },
            None => self.download(ctx, client, url, None).await?,
        };

        // If we're tracing I/O, get the materializer to copy to the offline cache
        // so we can include it in the offline archive manifest later.
        let io_provider = ctx.io_provider();
        if let Some(tracer) = TracingIoProvider::from_io(io_provider) {
            let offline_cache_path =
                offline::declare_copy_to_offline_output_cache(ctx, self.output(), value.dupe())
                    .await?;
            tracer.add_yak_out_entry(offline_cache_path);
        }

        Ok((
            ActionOutputs::from_single(self.output().get_path().dupe(), value),
            ActionExecutionMetadata {
                dep_file_db_writes_queued: 0,
                execution_kind,
                timing: ActionExecutionTimingData::default(),
                input_files_bytes: None,
                waiting_data,
            },
        ))
    }
}

#[cfg(test)]
mod tests {
    // TODO: This needs proper tests, but right now it's kind of a pain to get the
    //       action framework up and running to test actions
    #[test]
    fn downloads_file() {}
}
