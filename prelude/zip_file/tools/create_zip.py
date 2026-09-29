# Copyright (c) Meta Platforms, Inc. and affiliates.
#
# This source code is dual-licensed under either the MIT license found in the
# LICENSE-MIT file in the root directory of this source tree or the Apache
# License, Version 2.0 found in the LICENSE-APACHE file in the root directory
# of this source tree. You may select, at your option, one of the
# above-listed licenses.

"""Creates the archive of a `zip_file` target.

The archive holds the entries of the `--zip-source` archives, in order, and
then the files that `--entries-file` lists. Entries are sorted by name. Every
entry has the same timestamp, so the archive depends only on its inputs.
"""

from __future__ import annotations

import argparse
import os
import re
import shutil
import stat
import sys
import zipfile
from dataclasses import dataclass
from pathlib import Path, PurePosixPath
from typing import Dict, Iterator, List, Optional, Pattern, Union

# The DOS date and time of every entry. The zip format has no dates before 1980.
FIXED_DATE_TIME = (1985, 2, 1, 0, 0, 0)

# The permissions that `--hardcode-permissions` gives every file.
HARDCODED_FILE_MODE = 0o644
DIRECTORY_MODE = 0o755
MSDOS_DIRECTORY_FLAG = 0x10
UNIX_HOST = 3

DUPLICATE_ACTIONS = ("overwrite", "append", "fail")


@dataclass(frozen=True)
class FileSource:
    """A file on disk that becomes the entry `name`."""

    name: str
    path: Path

    def describe(self) -> str:
        return f"`{self.path}`"


@dataclass(frozen=True)
class ZipSource:
    """The entry `info` of the archive at `archive`."""

    name: str
    archive: Path
    info: zipfile.ZipInfo

    def describe(self) -> str:
        return f"`{self.archive}`"


@dataclass(frozen=True)
class ParentDirectory:
    """A directory entry for a parent of another entry that no input provides."""

    name: str


Source = Union[FileSource, ZipSource]


class ZipFileError(Exception):
    pass


def _parse_args(argv: List[str]) -> argparse.Namespace:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--output", required=True, type=Path)
    parser.add_argument(
        "--on-duplicate-entry",
        choices=DUPLICATE_ACTIONS,
        required=True,
        help="What to do when two inputs produce an entry with the same name.",
    )
    parser.add_argument(
        "--entries-file",
        type=Path,
        help=(
            "A file with three lines per source: its path, its path relative to "
            "its package, and `True` if it is a source file or `False` if a rule "
            "built it."
        ),
    )
    parser.add_argument(
        "--zip-source",
        action="append",
        default=[],
        type=Path,
        help="An archive whose entries to copy. Repeat it to copy several.",
    )
    parser.add_argument(
        "--exclude",
        action="append",
        default=[],
        help="A regular expression. Entries whose whole name matches it are left out.",
    )
    parser.add_argument(
        "--hardcode-permissions",
        action="store_true",
        help="Give every file 0644 permissions instead of the permissions on disk.",
    )
    return parser.parse_args(argv)


def _walk_files(directory: Path) -> Iterator[Path]:
    """Yields the files under `directory`, following symlinks to files."""
    for root, dirnames, filenames in os.walk(directory):
        root_path = Path(root)
        for dirname in dirnames:
            if (root_path / dirname).is_symlink():
                raise ZipFileError(
                    f"`{root_path / dirname}` is a symlink to a directory, which zip_file does not follow"
                )
        dirnames.sort()
        for filename in sorted(filenames):
            yield root_path / filename


def _file_sources(entries_file: Path) -> List[FileSource]:
    """Reads the files that `--entries-file` lists and names their entries.

    A source file keeps its path relative to its package. A file that a rule
    built keeps only its file name. The files inside a directory that a rule
    built keep their path relative to the directory's parent.
    """
    lines = entries_file.read_text(encoding="utf-8").splitlines()
    if len(lines) % 3 != 0:
        raise ZipFileError(
            f"`{entries_file}` has {len(lines)} lines, which is not a multiple of 3"
        )

    sources: Dict[str, FileSource] = {}

    def add(name: str, path: Path) -> None:
        previous = sources.get(name)
        if previous is not None:
            raise ZipFileError(
                f"Entry `{name}` comes from both `{previous.path}` and `{path}`"
            )
        sources[name] = FileSource(name, path)

    for index in range(0, len(lines), 3):
        path = Path(lines[index])
        short_path = lines[index + 1]
        is_source = lines[index + 2]
        if is_source == "True":
            add(short_path, path)
        elif is_source == "False":
            if path.is_dir():
                for file in _walk_files(path):
                    add(file.relative_to(path.parent).as_posix(), file)
            else:
                add(path.name, path)
        else:
            raise ZipFileError(
                f"Line {index + 3} of `{entries_file}` is `{is_source}`, expected `True` or `False`"
            )
    return list(sources.values())


class EntryCollection:
    """Collects the sources of each entry name and applies the duplicate action."""

    def __init__(self, exclude: List[Pattern[str]], on_duplicate_entry: str) -> None:
        self._exclude = exclude
        self._on_duplicate_entry = on_duplicate_entry
        self._by_name: Dict[str, List[Source]] = {}

    def add(self, source: Source) -> None:
        if any(pattern.fullmatch(source.name) for pattern in self._exclude):
            return
        existing = self._by_name.get(source.name)
        if existing is None:
            self._by_name[source.name] = [source]
        elif self._on_duplicate_entry == "append":
            existing.append(source)
        elif self._on_duplicate_entry == "overwrite":
            self._by_name[source.name] = [source]
        else:
            raise ZipFileError(
                f"Duplicate entry `{source.name}` comes from {existing[0].describe()} and {source.describe()}"
            )

    def entries(self) -> List[Union[Source, ParentDirectory]]:
        """Returns the entries sorted by name.

        Each parent directory of a file from disk gets an entry, unless an input
        already provides it. Entries copied from archives keep their layout.
        """
        by_name: Dict[str, List[Union[Source, ParentDirectory]]] = dict(
            self._by_name
        )
        for name, sources in self._by_name.items():
            if not any(isinstance(source, FileSource) for source in sources):
                continue
            for parent in PurePosixPath(name).parents:
                directory = f"{parent}/"
                if parent != PurePosixPath(".") and directory not in by_name:
                    by_name[directory] = [ParentDirectory(directory)]
        return [entry for name in sorted(by_name) for entry in by_name[name]]


def _new_info(name: str) -> zipfile.ZipInfo:
    info = zipfile.ZipInfo(name, date_time=FIXED_DATE_TIME)
    info.create_system = UNIX_HOST
    return info


def _write_parent_directory(archive: zipfile.ZipFile, entry: ParentDirectory) -> None:
    info = _new_info(entry.name)
    info.external_attr = (
        (stat.S_IFDIR | DIRECTORY_MODE) << 16
    ) | MSDOS_DIRECTORY_FLAG
    archive.writestr(info, b"")


def _write_file(
    archive: zipfile.ZipFile, source: FileSource, hardcode_permissions: bool
) -> None:
    status = source.path.stat()
    mode = HARDCODED_FILE_MODE if hardcode_permissions else status.st_mode & 0o777
    info = _new_info(source.name)
    info.compress_type = zipfile.ZIP_DEFLATED
    info.external_attr = (stat.S_IFREG | mode) << 16
    info.file_size = status.st_size
    with source.path.open("rb") as src, archive.open(info, "w") as dst:
        shutil.copyfileobj(src, dst)


def _copy_zip_entry(
    archive: zipfile.ZipFile, source: ZipSource, readers: Dict[Path, zipfile.ZipFile]
) -> None:
    # A new info drops the extra fields of the original entry, which can hold
    # timestamps, and keeps its compression and permissions.
    info = _new_info(source.name)
    info.external_attr = source.info.external_attr
    info.create_system = source.info.create_system
    if source.info.is_dir():
        archive.writestr(info, b"")
        return
    info.compress_type = source.info.compress_type
    info.file_size = source.info.file_size
    reader = readers.get(source.archive)
    if reader is None:
        reader = zipfile.ZipFile(source.archive)
        readers[source.archive] = reader
    with reader.open(source.info) as src, archive.open(info, "w") as dst:
        shutil.copyfileobj(src, dst)


def _compile_exclude(pattern: str) -> Pattern[str]:
    try:
        return re.compile(pattern)
    except re.error as e:
        raise ZipFileError(
            f"`entries_to_exclude` pattern `{pattern}` is not a valid regular expression: {e}"
        ) from e


def create_zip(
    output: Path,
    on_duplicate_entry: str,
    entries_file: Optional[Path],
    zip_sources: List[Path],
    exclude: List[str],
    hardcode_permissions: bool,
) -> None:
    collection = EntryCollection(
        [_compile_exclude(pattern) for pattern in exclude], on_duplicate_entry
    )
    for zip_source in zip_sources:
        with zipfile.ZipFile(zip_source) as reader:
            for info in reader.infolist():
                collection.add(ZipSource(info.filename, zip_source, info))
    if entries_file is not None:
        for file_source in _file_sources(entries_file):
            collection.add(file_source)

    readers: Dict[Path, zipfile.ZipFile] = {}
    try:
        with zipfile.ZipFile(output, "w") as archive:
            for entry in collection.entries():
                if isinstance(entry, ZipSource):
                    _copy_zip_entry(archive, entry, readers)
                elif isinstance(entry, FileSource):
                    _write_file(archive, entry, hardcode_permissions)
                else:
                    _write_parent_directory(archive, entry)
    finally:
        for reader in readers.values():
            reader.close()


def main(argv: List[str]) -> int:
    args = _parse_args(argv)
    try:
        create_zip(
            output=args.output,
            on_duplicate_entry=args.on_duplicate_entry,
            entries_file=args.entries_file,
            zip_sources=args.zip_source,
            exclude=args.exclude,
            hardcode_permissions=args.hardcode_permissions,
        )
    except ZipFileError as e:
        print(f"zip_file: {e}", file=sys.stderr)
        return 1
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
