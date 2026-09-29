/*
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is dual-licensed under either the MIT license found in the
 * LICENSE-MIT file in the root directory of this source tree or the Apache
 * License, Version 2.0 found in the LICENSE-APACHE file in the root directory
 * of this source tree. You may select, at your option, one of the
 * above-listed licenses.
 */

use std::env;
use std::fs;
use std::path::Path;
use std::sync::Arc;
use std::sync::OnceLock;

use cmp_any::PartialEqAny;
use dice::CancellationContext;
use dice::DiceComputations;
use dice::EqualityBehavior;
use dice::Key;
use dice::OkPagableValueSerialize;
use dice::ValueSerialize;
use dupe::Dupe;
use pagable::Pagable;
use pagable::PagableDeserialize;
use pagable::PagableDeserializer;
use pagable::PagableSerialize;
use pagable::PagableSerializer;
use pagable::pagable_typetag;
use yak_build_api::actions::artifact::get_artifact_fs::GetArtifactFs;
use yak_build_api::materialize::invocation_re_use_case;
use yak_common::file_ops::delegate::FileOpsDelegate;
use yak_common::file_ops::metadata::FileMetadata;
use yak_common::file_ops::metadata::FileType;
use yak_common::file_ops::metadata::RawDirEntry;
use yak_common::file_ops::metadata::RawPathMetadata;
use yak_common::file_ops::metadata::TrackedFileDigest;
use yak_common::io::fs::is_executable;
use yak_core::cells::external::ExternalCellOrigin;
use yak_core::cells::name::CellName;
use yak_core::cells::paths::CellRelativePath;
use yak_core::cells::paths::CellRelativePathBuf;
use yak_core::directory_digest::DirectoryDigest;
use yak_core::fs::project_rel_path::ProjectRelativePathBuf;
use yak_core::fs::yak_out_path::YakOutPathKind;
use yak_directory::directory::builder::DirectoryBuilder;
use yak_directory::directory::directory::Directory;
use yak_directory::directory::directory_hasher::DirectoryDigester;
use yak_directory::directory::directory_iterator::DirectoryIterator;
use yak_directory::directory::directory_ref::DirectoryRef;
use yak_directory::directory::directory_ref::FingerprintedDirectoryRef;
use yak_directory::directory::entry::DirectoryEntry;
use yak_directory::directory::find::DirectoryFindError;
use yak_directory::directory::find::find;
use yak_directory::directory::immutable_directory::ImmutableDirectory;
use yak_error::YakErrorContext;
use yak_error::YakErrorOptionContext;
use yak_error::conversion::from_any_with_tag;
use yak_error::yak_error;
use yak_execute::artifact_value::ArtifactValue;
use yak_execute::digest_config::DigestConfig;
use yak_execute::digest_config::HasDigestConfig;
use yak_execute::materialize::materializer::HasMaterializer;
use yak_execute::materialize::materializer::MaterializationPurpose;
use yak_execute::materialize::materializer::MaterializeRequest;
use yak_execute::materialize::materializer::WriteRequest;
use yak_external_cells_bundled::BundledCell;
use yak_external_cells_bundled::BundledFile;
use yak_external_cells_bundled::get_bundled_data;
use yak_fs::error::IoResultExt;
use yak_fs::fs_util;
use yak_fs::paths::abs_path::AbsPathBuf;
use yak_fs::paths::file_name::FileName;
use yak_fs::paths::forward_rel_path::ForwardRelativePath;
use yak_fs::paths::forward_rel_path::ForwardRelativePathBuf;
use yak_util::strong_hasher::Blake3StrongHasher;

fn load_nano_prelude() -> yak_error::Result<BundledCell> {
    let path = env::var("NANO_PRELUDE")
        .map_err(|e| from_any_with_tag(e, yak_error::ErrorTag::Input))
        .yak_error_context(
            "NANO_PRELUDE env var must be set to the location of nano prelude\n\
        Consider `export NANO_PRELUDE=<yak repository>/tests/e2e_util/nano_prelude`",
        )?;
    if path.is_empty() {
        return Err(yak_error!(
            yak_error::ErrorTag::Input,
            "NANO_PRELUDE env var must not be empty"
        ));
    }
    let path = AbsPathBuf::new(Path::new(&path))
        .yak_error_context("NANO_PRELUDE env var must point to absolute path")?;

    let mut files = Vec::new();
    let mut dir_stack = Vec::new();
    dir_stack.push((path, ForwardRelativePathBuf::empty()));
    while let Some((dir, rel_path)) = dir_stack.pop() {
        for entry in fs::read_dir(dir)? {
            let entry = entry?;
            let entry_path = AbsPathBuf::new(entry.path())?;
            let entry_rel_path = rel_path.join(FileName::new(
                entry
                    .file_name()
                    .to_str()
                    .internal_error("not UTF-8 string")?,
            )?);
            match FileType::from(entry.file_type()?) {
                FileType::Directory => dir_stack.push((entry_path, entry_rel_path)),
                FileType::File => {
                    let contents = fs_util::read(&entry_path).categorize_internal()?;
                    files.push(BundledFile {
                        path: entry_rel_path.as_str().to_owned().leak(),
                        contents: contents.leak(),
                        is_executable: is_executable(&entry.metadata()?),
                    });
                }
                FileType::Symlink | FileType::Unknown => {
                    // We don't have these in nano-prelude.
                }
            }
        }
    }

    Ok(BundledCell {
        name: "nano_prelude",
        files: files.leak(),
        is_testing: true,
    })
}

fn nano_prelude() -> yak_error::Result<BundledCell> {
    static NANO_PRELUDE: OnceLock<BundledCell> = OnceLock::new();
    Ok(*NANO_PRELUDE
        .get_or_try_init(|| load_nano_prelude().yak_error_context("loading nano_prelude"))?)
}

pub(crate) fn find_bundled_data(cell_name: CellName) -> yak_error::Result<BundledCell> {
    #[derive(yak_error::Error, Debug)]
    #[error("No bundled cell named `{0}`, options are `{}`", _1.join(", "))]
    #[yak(tag = Input)]
    struct CellNotBundled(String, Vec<&'static str>);

    let cell_name = cell_name.as_str();

    if cell_name == "nano_prelude" {
        return nano_prelude();
    }

    get_bundled_data()
        .iter()
        .find(|data| data.name == cell_name)
        .copied()
        .ok_or_else(|| {
            CellNotBundled(
                cell_name.to_owned(),
                get_bundled_data()
                    .iter()
                    .filter(|data| !data.is_testing)
                    .map(|data| data.name)
                    .collect(),
            )
            .into()
        })
}

#[derive(Clone, PartialEq, Eq, Hash, Debug, allocative::Allocative)]
struct ContentsAndMetadata {
    contents: &'static [u8],
    metadata: FileMetadata,
}

/// We don't actually need the directory digest, but unfortunately the directory tooling kind of
/// requires us to have one.
#[derive(
    allocative::Allocative,
    derive_more::Display,
    Debug,
    PartialEq,
    Eq,
    Hash,
    Copy,
    Clone
)]
struct BundledDirectoryDigest(#[allocative(skip)] blake3::Hash);

impl Dupe for BundledDirectoryDigest {
    fn dupe(&self) -> Self {
        *self
    }
}

impl DirectoryDigest for BundledDirectoryDigest {}

struct BundledDirectoryDigester;

impl DirectoryDigester<ContentsAndMetadata, BundledDirectoryDigest> for BundledDirectoryDigester {
    fn hash_entries<'a, D, I>(&self, entries: I) -> BundledDirectoryDigest
    where
        I: IntoIterator<Item = (&'a FileName, DirectoryEntry<D, &'a ContentsAndMetadata>)>,
        D: FingerprintedDirectoryRef<
                'a,
                Leaf = ContentsAndMetadata,
                DirectoryDigest = BundledDirectoryDigest,
            > + 'a,
        Self: Sized,
    {
        use std::hash::Hash;

        let mut hasher = Blake3StrongHasher::default();
        for (name, entry) in entries {
            name.hash(&mut hasher);
            match entry {
                DirectoryEntry::Dir(dir) => {
                    dir.as_fingerprinted_dyn().fingerprint().hash(&mut hasher);
                }
                DirectoryEntry::Leaf(leaf) => {
                    leaf.metadata.hash(&mut hasher);
                }
            }
        }
        BundledDirectoryDigest(hasher.finalize())
    }

    fn leaf_size(&self, leaf: &ContentsAndMetadata) -> u64 {
        leaf.contents.len() as u64
    }
}

#[derive(allocative::Allocative)]
pub(crate) struct BundledFileOpsDelegate {
    cell_name: CellName,
    digest_config: DigestConfig,
    dir: ImmutableDirectory<ContentsAndMetadata, BundledDirectoryDigest>,
}

impl PagableSerialize for BundledFileOpsDelegate {
    fn pagable_serialize(&self, serializer: &mut dyn PagableSerializer) -> pagable::Result<()> {
        self.cell_name.pagable_serialize(serializer)?;
        self.digest_config.pagable_serialize(serializer)
    }
}

impl<'de> PagableDeserialize<'de> for BundledFileOpsDelegate {
    fn pagable_deserialize<D: PagableDeserializer<'de> + ?Sized>(
        deserializer: &mut D,
    ) -> pagable::Result<Self> {
        let cell_name = CellName::pagable_deserialize(deserializer)?;
        let digest_config = DigestConfig::pagable_deserialize(deserializer)?;
        get_file_ops_delegate_impl(cell_name, digest_config)
            .map_err(|e| pagable::Error::msg(e.to_string()))
    }
}

#[derive(yak_error::Error, Debug)]
#[yak(tag = Environment)]
enum BundledPathSearchError {
    #[error("Expected a directory at `{0}` but found a file")]
    ExpectedDirectory(String),
    #[error("Path not found: `{0}`")]
    MissingFile(CellRelativePathBuf),
    #[error("Expected file at `{0}` but found a directory")]
    ExpectedFile(CellRelativePathBuf),
}

impl BundledFileOpsDelegate {
    fn get_entry_at_path_if_exists(
        &self,
        path: &CellRelativePath,
    ) -> yak_error::Result<
        Option<
            DirectoryEntry<
                impl DirectoryRef<
                    '_,
                    Leaf = ContentsAndMetadata,
                    DirectoryDigest = BundledDirectoryDigest,
                > + use<'_>,
                &ContentsAndMetadata,
            >,
        >,
    > {
        match find(self.dir.as_ref(), path.iter()) {
            Ok(entry) => Ok(entry),
            Err(DirectoryFindError::CannotTraverseLeaf { path }) => {
                Err(BundledPathSearchError::ExpectedDirectory(path.to_string()).into())
            }
        }
    }

    fn get_entry_at_path(
        &self,
        path: &CellRelativePath,
    ) -> yak_error::Result<
        DirectoryEntry<
            impl DirectoryRef<'_, Leaf = ContentsAndMetadata, DirectoryDigest = BundledDirectoryDigest>
            + use<'_>,
            &ContentsAndMetadata,
        >,
    > {
        self.get_entry_at_path_if_exists(path)?
            .ok_or_else(|| BundledPathSearchError::MissingFile(path.to_owned()).into())
    }

    /// Return the list of file outputs, sorted.
    async fn read_dir(&self, path: &CellRelativePath) -> yak_error::Result<Arc<[RawDirEntry]>> {
        let dir = match self.get_entry_at_path(path)? {
            DirectoryEntry::Dir(dir) => dir,
            DirectoryEntry::Leaf(_) => {
                return Err(BundledPathSearchError::ExpectedDirectory(path.to_string()).into());
            }
        };

        let entries = dir
            .entries()
            .map(|(name, entry)| RawDirEntry {
                file_name: name.to_owned().into_inner(),
                file_type: match entry {
                    DirectoryEntry::Leaf(_) => FileType::File,
                    DirectoryEntry::Dir(_) => FileType::Directory,
                },
            })
            .collect();

        Ok(entries)
    }

    fn read_file_if_exists(
        &self,
        path: &CellRelativePath,
    ) -> yak_error::Result<Option<&'static str>> {
        match self.get_entry_at_path_if_exists(path)? {
            Some(DirectoryEntry::Leaf(leaf)) => Ok(Some(str::from_utf8(leaf.contents)?)),
            Some(DirectoryEntry::Dir(_)) => {
                Err(BundledPathSearchError::ExpectedFile(path.to_owned()).into())
            }
            None => Ok(None),
        }
    }

    fn read_path_metadata_if_exists(
        &self,
        path: &CellRelativePath,
    ) -> yak_error::Result<Option<RawPathMetadata>> {
        match self.get_entry_at_path_if_exists(path)? {
            Some(DirectoryEntry::Leaf(leaf)) => {
                Ok(Some(RawPathMetadata::File(leaf.metadata.clone())))
            }
            Some(DirectoryEntry::Dir(_)) => Ok(Some(RawPathMetadata::Directory)),
            None => Ok(None),
        }
    }
}

#[pagable_typetag]
#[async_trait::async_trait]
impl FileOpsDelegate for BundledFileOpsDelegate {
    async fn read_file_if_exists(
        &self,
        _ctx: &mut DiceComputations<'_>,
        path: &'async_trait CellRelativePath,
    ) -> yak_error::Result<Option<String>> {
        Ok(self.read_file_if_exists(path)?.map(|s| s.to_owned()))
    }

    /// Return the list of file outputs, sorted.
    async fn read_dir(
        &self,
        _ctx: &mut DiceComputations<'_>,
        path: &'async_trait CellRelativePath,
    ) -> yak_error::Result<Arc<[RawDirEntry]>> {
        self.read_dir(path).await
    }

    async fn read_path_metadata_if_exists(
        &self,
        _ctx: &mut DiceComputations<'_>,
        path: &'async_trait CellRelativePath,
    ) -> yak_error::Result<Option<RawPathMetadata>> {
        self.read_path_metadata_if_exists(path)
    }

    fn eq_token(&self) -> PartialEqAny<'_> {
        PartialEqAny::always_false()
    }
}

fn get_file_ops_delegate_impl(
    cell_name: CellName,
    digest_config: DigestConfig,
) -> yak_error::Result<BundledFileOpsDelegate> {
    let data = find_bundled_data(cell_name)?;
    let mut builder: DirectoryBuilder<ContentsAndMetadata, BundledDirectoryDigest> =
        DirectoryBuilder::empty_non_exhaustive();
    let source_digest_config = digest_config.cas_digest_config().source_files_config();
    for file in data.files {
        let path = ForwardRelativePath::new(file.path)
            .internal_error("non-forward relative bundled path")?;
        let metadata = FileMetadata {
            digest: TrackedFileDigest::from_content(file.contents, source_digest_config),
            is_executable: file.is_executable,
        };

        builder
            .insert(
                path,
                DirectoryEntry::Leaf(ContentsAndMetadata {
                    contents: file.contents,
                    metadata,
                }),
            )
            .internal_error("conflicting bundled source paths")?;
    }
    let builder = builder.fingerprint(&BundledDirectoryDigester);
    Ok(BundledFileOpsDelegate {
        cell_name,
        digest_config,
        dir: builder,
    })
}

async fn declare_all_source_artifacts(
    ctx: &mut DiceComputations<'_>,
    cell_name: CellName,
    ops: &BundledFileOpsDelegate,
) -> yak_error::Result<()> {
    let mut requests = Vec::new();
    let artifact_fs = ctx.get_artifact_fs().await?;
    let yak_out_resolver = artifact_fs.yak_out_path_resolver();

    for (path, entry) in ops.dir.unordered_walk_leaves().with_paths() {
        let path = yak_out_resolver.resolve_external_cell_source(
            CellRelativePath::new(path.as_ref()),
            ExternalCellOrigin::Bundled(cell_name),
        );
        requests.push(WriteRequest {
            path,
            content: entry.contents.to_vec(),
            is_executable: entry.metadata.is_executable,
            path_kind: YakOutPathKind::Configuration,
        });
    }

    let materializer = ctx.per_transaction_data().get_materializer();
    materializer
        .declare_write(Box::new(move || Ok(requests)))
        .await
        .map(|_| ())
}

pub(crate) async fn get_file_ops_delegate(
    ctx: &mut DiceComputations<'_>,
    cell_name: CellName,
) -> yak_error::Result<Arc<BundledFileOpsDelegate>> {
    #[derive(
        dupe::Dupe,
        Clone,
        Copy,
        Debug,
        derive_more::Display,
        PartialEq,
        Eq,
        Hash,
        allocative::Allocative,
        Pagable
    )]
    #[pagable_typetag(dice::DiceKeyDyn)]
    struct BundledFileOpsDelegateKey(CellName);

    #[async_trait::async_trait]
    impl Key for BundledFileOpsDelegateKey {
        type Value = yak_error::Result<Arc<BundledFileOpsDelegate>>;

        async fn compute(
            &self,
            ctx: &mut DiceComputations,
            _cancellations: &CancellationContext,
        ) -> Self::Value {
            let ops = get_file_ops_delegate_impl(self.0, ctx.global_data().get_digest_config())?;
            declare_all_source_artifacts(ctx, self.0, &ops).await?;
            Ok(Arc::new(ops))
        }

        fn equality_behavior() -> EqualityBehavior<Self::Value> {
            EqualityBehavior::Compare(|_x, _y| {
                // No need for non-trivial equality, because this has no deps and is never recomputed
                false
            })
        }

        fn value_serialize() -> impl ValueSerialize<Value = Self::Value> {
            OkPagableValueSerialize::<Self::Value>::new()
        }
    }

    ctx.compute(&BundledFileOpsDelegateKey(cell_name))
        .await?
        .dupe()
}

pub(crate) async fn materialize_all(
    ctx: &mut DiceComputations<'_>,
    cell: CellName,
) -> yak_error::Result<ProjectRelativePathBuf> {
    let artifact_fs = ctx.get_artifact_fs().await?;
    let yak_out_resolver = artifact_fs.yak_out_path_resolver();

    let ops = get_file_ops_delegate(ctx, cell).await?;
    let mut artifacts = Vec::new();
    for (path, entry) in ops.dir.unordered_walk_leaves().with_paths() {
        let path = yak_out_resolver.resolve_external_cell_source(
            CellRelativePath::new(path.as_ref()),
            ExternalCellOrigin::Bundled(cell),
        );
        // `entry.metadata` was digested with the source-files config, while the files were
        // written through `declare_write` under the CAS config. The two are identical today; a
        // materializer that checks values against disk should be sent the `declare_write`
        // results instead.
        artifacts.push((path, ArtifactValue::file(entry.metadata.dupe())));
    }

    let re_use_case = invocation_re_use_case(ctx);
    let response = ctx
        .per_transaction_data()
        .get_materializer()
        .materialize(MaterializeRequest {
            artifacts,
            outputs: Vec::new(),
            purpose: MaterializationPurpose::IntermediateOnly,
            re_use_case,
        })
        .await?;
    for result in response.results {
        result?;
    }
    // FIXME(materializer): The command reads the cell's sources for as long as it runs, so the
    // lease should live as long, held by whatever dice value this becomes. Nothing here can
    // hold it that long, so it goes with the result.
    drop(response.lease);
    Ok(yak_out_resolver.resolve_external_cell_source(
        CellRelativePath::unchecked_new(""),
        ExternalCellOrigin::Bundled(cell),
    ))
}

#[cfg(test)]
mod tests {
    use std::assert_matches;

    use pagable::testing::TestingDeserializer;
    use pagable::testing::TestingSerializer;

    use super::*;

    fn testing_ops() -> BundledFileOpsDelegate {
        get_file_ops_delegate_impl(
            CellName::testing_new("test_bundled_cell"),
            DigestConfig::testing_default(),
        )
        .unwrap()
    }

    #[tokio::test]
    async fn test_smoke_read() {
        let ops = testing_ops();
        let content = ops
            .read_file_if_exists(CellRelativePath::unchecked_new("dir/src.txt"))
            .unwrap()
            .unwrap();
        let content = if cfg!(windows) {
            // Git may check out files on Windows with \r\n as line separator.
            // We could configure git, but it's more reliable to handle it in the test.
            content.replace("\r\n", "\n")
        } else {
            content.to_owned()
        };
        assert_eq!(content, "foobar\n");
        assert!(
            ops.read_file_if_exists(CellRelativePath::unchecked_new("dir/does_not_exist.txt"))
                .unwrap()
                .is_none()
        );
    }

    #[tokio::test]
    async fn test_executable_bit() {
        let ops = testing_ops();
        assert_matches!(
            ops.read_path_metadata_if_exists(CellRelativePath::unchecked_new("dir/src.txt"))
                .unwrap()
                .unwrap(),
            RawPathMetadata::File(FileMetadata {
                digest: _,
                is_executable: false,
            }),
        );
        assert_matches!(
            ops.read_path_metadata_if_exists(CellRelativePath::unchecked_new("dir/src2.txt"))
                .unwrap()
                .unwrap(),
            RawPathMetadata::File(FileMetadata {
                digest: _,
                is_executable: true,
            }),
        );
    }

    #[tokio::test]
    async fn test_dir_listing() {
        let ops = testing_ops();

        let root = CellRelativePath::unchecked_new("");
        let root_metadata = ops.read_path_metadata_if_exists(root).unwrap().unwrap();
        assert_matches!(root_metadata, RawPathMetadata::Directory);
        let root_entries = ops.read_dir(root).await.unwrap();
        assert!(root_entries.is_sorted());
        assert_eq!(
            &*root_entries,
            &[
                RawDirEntry {
                    file_name: ".yakconfig".into(),
                    file_type: FileType::File
                },
                RawDirEntry {
                    file_name: "YAK_TREE".into(),
                    file_type: FileType::File
                },
                RawDirEntry {
                    file_name: "dir".into(),
                    file_type: FileType::Directory
                },
            ],
        );

        let dir = CellRelativePath::unchecked_new("dir");
        let dir_metadata = ops.read_path_metadata_if_exists(dir).unwrap().unwrap();
        assert_matches!(dir_metadata, RawPathMetadata::Directory);
        let dir_entries = ops.read_dir(dir).await.unwrap();
        assert!(dir_entries.is_sorted());
        assert_eq!(dir_entries.len(), 5);
    }

    #[test]
    fn test_load_all_bundled_cells() {
        for c in get_bundled_data() {
            get_file_ops_delegate_impl(
                CellName::testing_new(c.name),
                DigestConfig::testing_default(),
            )
            .unwrap();
        }
    }

    #[test]
    fn test_pagable_round_trip() {
        let ops = testing_ops();
        let mut serializer = TestingSerializer::new();
        ops.pagable_serialize(&mut serializer).unwrap();
        let bytes = serializer.finish();

        let mut deserializer = TestingDeserializer::new(&bytes);
        let restored = BundledFileOpsDelegate::pagable_deserialize(&mut deserializer).unwrap();

        let path = CellRelativePath::unchecked_new("dir/src.txt");
        assert_eq!(
            ops.read_file_if_exists(path).unwrap(),
            restored.read_file_if_exists(path).unwrap()
        );
    }
}
