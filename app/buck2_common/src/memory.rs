/*
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is dual-licensed under either the MIT license found in the
 * LICENSE-MIT file in the root directory of this source tree or the Apache
 * License, Version 2.0 found in the LICENSE-APACHE file in the root directory
 * of this source tree. You may select, at your option, one of the
 * above-listed licenses.
 */

//! Utilities for interacting with the jemalloc heap used by yak.
//!
//! In order to make use of jemalloc's heap dump or profiling utilities, you must set the MALLOC_CONF environment
//! variable to a suitable value prior to launching the daemon, such as:
//!  `export MALLOC_CONF=prof:true,prof_final:false,prof_prefix:/tmp/jeprof`
//! This turns on the profiler (`prof:true`), tells the profile to not take heap dump on process exit,
//! (`prof_final:false`), and write dumps to `/tmp/jeprof` if a file argument isn't given to `mallctl`
//! (`prof_prefix:/tmp/jeprof`).

mod imp {
    pub fn write_heap_to_file(_filename: &str) -> buck2_error::Result<()> {
        // TODO(swgillespie) the `jemalloc_ctl` crate is probably capable of doing this
        // and we already link against it
        Err(buck2_error::buck2_error!(
            buck2_error::ErrorTag::Unimplemented,
            "not implemented: heap dump for Cargo builds"
        ))
    }

    pub fn allocator_stats(_: &str) -> buck2_error::Result<String> {
        Err(buck2_error::buck2_error!(
            buck2_error::ErrorTag::Unimplemented,
            "not implemented: allocator stats  for Cargo builds"
        ))
    }

    pub fn enable_background_threads() -> buck2_error::Result<()> {
        Ok(())
    }

    pub fn has_jemalloc_stats() -> bool {
        false
    }

    pub fn is_using_jemalloc() -> bool {
        false
    }

    pub fn purge_jemalloc() -> buck2_error::Result<()> {
        Ok(())
    }
}

pub use imp::allocator_stats;
pub use imp::enable_background_threads;
pub use imp::has_jemalloc_stats;
pub use imp::is_using_jemalloc;
pub use imp::purge_jemalloc;
pub use imp::write_heap_to_file;
