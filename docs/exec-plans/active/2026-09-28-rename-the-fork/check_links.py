#!/usr/bin/env python3
"""Checks the links of the documentation site in a tree.

Usage: check_links.py <tree root>

Prints one line per link that does not resolve: relative links to pages and
files, heading anchors, sidebar ids, redirect targets, and images. Pages that
website/gen_docs.py generates are not in the tree, so links into them are
skipped.
"""

import os
import posixpath
import re
import sys
import unicodedata

GENERATED = ("api/", "prelude/", "users/commands/", "users/query/")

LINK = re.compile(r"(?<!!)\[(?:[^\]\[]|\[[^\]]*\])*\]\(([^)\s]+)(?:\s+\"[^\"]*\")?\)")
IMG_MD = re.compile(r"!\[[^\]]*\]\(([^)\s]+)\)")
USE_BASE_URL = re.compile(r"useBaseUrl\(\s*['\"]([^'\"]+)['\"]")
HREF = re.compile(r"href=[{]?['\"]([^'\"]+)['\"]")
HEADING = re.compile(r"^(#{1,6})\s+(.*?)\s*(?:\{#([\w-]+)\})?\s*$")
FENCE = re.compile(r"^\s*(```+|~~~+)")
FRONT = re.compile(r"\A---\n(.*?)\n---\n", re.S)


def slug(text):
    """Returns the anchor that github-slugger gives a heading."""
    text = re.sub(r"`([^`]*)`", r"\1", text)
    text = re.sub(r"\[([^\]]*)\]\([^)]*\)", r"\1", text)
    text = re.sub(r"<[^>]+>", "", text)
    text = text.strip().lower()
    out = []
    for ch in text:
        cat = unicodedata.category(ch)
        if ch in "-_" or ch == " " or cat[0] in "LN" or cat == "Mn":
            out.append(ch)
    return "".join(out).replace(" ", "-")


def read_docs(root):
    docs_root = os.path.join(root, "website", "docs")
    pages = {}  # doc id -> (path, anchors)
    by_path = {}
    for dirpath, _, files in os.walk(docs_root):
        for name in files:
            if not name.endswith((".md", ".mdx")):
                continue
            path = os.path.join(dirpath, name)
            rel = os.path.relpath(path, docs_root).replace(os.sep, "/")
            with open(path, encoding="utf-8") as f:
                text = f.read()
            front = FRONT.match(text)
            doc_id = posixpath.splitext(rel)[0]
            if front:
                m = re.search(r"^id:\s*(\S+)\s*$", front.group(1), re.M)
                if m:
                    doc_id = posixpath.join(posixpath.dirname(rel), m.group(1))
            anchors = set()
            counts = {}
            in_fence = False
            for line in text.split("\n"):
                if FENCE.match(line):
                    in_fence = not in_fence
                    continue
                if in_fence:
                    continue
                m = HEADING.match(line)
                if m:
                    anchor = m.group(3) or slug(m.group(2))
                    n = counts.get(anchor, 0)
                    counts[anchor] = n + 1
                    anchors.add(anchor if n == 0 else f"{anchor}-{n}")
                for m in re.finditer(r"<a (?:name|id)=['\"]([\w-]+)['\"]", line):
                    anchors.add(m.group(1))
            pages[doc_id] = (rel, anchors)
            by_path[rel] = doc_id
    return docs_root, pages, by_path


def url_of(doc_id):
    if doc_id.endswith("/index"):
        return doc_id[: -len("index")]
    return doc_id


def main():
    root = sys.argv[1]
    docs_root, pages, by_path = read_docs(root)
    urls = {}
    for doc_id in pages:
        urls[url_of(doc_id).rstrip("/")] = doc_id
    problems = []

    def check_doc_url(source, target_path, anchor, link):
        target_path = target_path.rstrip("/")
        if target_path.startswith(GENERATED) or target_path + "/" in GENERATED:
            return
        doc_id = urls.get(target_path)
        if doc_id is None:
            problems.append(f"{source}: no page for {link}")
            return
        if anchor and anchor not in pages[doc_id][1]:
            problems.append(f"{source}: no anchor #{anchor} for {link}")

    for doc_id, (rel, anchors) in sorted(pages.items()):
        path = os.path.join(docs_root, rel)
        with open(path, encoding="utf-8") as f:
            text = f.read()
        in_fence = False
        page_url = url_of(doc_id)
        page_dir = page_url if page_url.endswith("/") else page_url + "/"
        for line in text.split("\n"):
            if FENCE.match(line):
                in_fence = not in_fence
                continue
            if in_fence:
                continue
            line_nocode = re.sub(r"`[^`]*`", "", line)
            for m in LINK.finditer(line_nocode):
                link = m.group(1)
                if re.match(r"^[a-z]+:", link) or link.startswith("mailto:"):
                    continue
                target, _, anchor = link.partition("#")
                if not target:
                    if anchor and anchor not in anchors:
                        problems.append(f"website/docs/{rel}: no anchor #{anchor}")
                    continue
                if target.startswith("/"):
                    if target.startswith("/docs/"):
                        check_doc_url(f"website/docs/{rel}", target[len("/docs/"):], anchor, link)
                    continue
                if target.endswith((".md", ".mdx")):
                    target_rel = posixpath.normpath(posixpath.join(posixpath.dirname(rel), target))
                    if target_rel.startswith(GENERATED):
                        continue
                    if target_rel.startswith("../"):
                        repo_path = os.path.normpath(os.path.join(os.path.dirname(path), target))
                        if not os.path.exists(repo_path):
                            problems.append(f"website/docs/{rel}: no file for {link}")
                        continue
                    target_id = by_path.get(target_rel)
                    if target_id is None:
                        problems.append(f"website/docs/{rel}: no file for {link}")
                    elif anchor and anchor not in pages[target_id][1]:
                        problems.append(f"website/docs/{rel}: no anchor #{anchor} for {link}")
                    continue
                if re.search(r"\.(png|jpg|jpeg|svg|gif|pdf|json|txt|bzl|bxl|rs|py)$", target) or "YAK" in target:
                    repo_path = os.path.normpath(os.path.join(os.path.dirname(path), target))
                    if not os.path.exists(repo_path):
                        problems.append(f"website/docs/{rel}: no file for {link}")
                    continue
                resolved = posixpath.normpath(posixpath.join(page_dir, target))
                check_doc_url(f"website/docs/{rel}", resolved, anchor, link)
            for m in list(USE_BASE_URL.finditer(line)) + list(IMG_MD.finditer(line_nocode)):
                target = m.group(1)
                if re.match(r"^[a-z]+:", target):
                    continue
                if target.startswith("/"):
                    static = os.path.join(root, "website", "static", target.lstrip("/"))
                else:
                    static = os.path.normpath(os.path.join(os.path.dirname(path), target))
                if not os.path.exists(static):
                    problems.append(f"website/docs/{rel}: no file for {target}")

    for name in ("sidebars.ts", "redirects.ts"):
        with open(os.path.join(root, "website", name), encoding="utf-8") as f:
            text = f.read()
        if name == "sidebars.ts":
            for m in re.finditer(r"(?:^\s*|id:\s*)'([\w/.-]+)'", text, re.M):
                doc_id = m.group(1)
                if doc_id.startswith(GENERATED) or doc_id in ("main",):
                    continue
                if doc_id not in pages:
                    problems.append(f"website/sidebars.ts: no page {doc_id}")
        else:
            for m in re.finditer(r"to:\s*'(/docs/[^']+)'", text):
                target = m.group(1)[len("/docs/"):]
                if not target.startswith(GENERATED) and target.rstrip("/") not in urls:
                    problems.append(f"website/redirects.ts: no page {m.group(1)}")

    for line in sorted(set(problems)):
        print(line)


if __name__ == "__main__":
    main()
