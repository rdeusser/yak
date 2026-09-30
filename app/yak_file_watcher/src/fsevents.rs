/*
 * This source code is dual-licensed under either the MIT license found in the
 * LICENSE-MIT file in the root directory of this source tree or the Apache
 * License, Version 2.0 found in the LICENSE-APACHE file in the root directory
 * of this source tree. You may select, at your option, one of the
 * above-listed licenses.
 */

//! An FSEvents stream of the project root that leaves out directories, such as `yak-out`.
//!
//! `notify` watches through FSEvents too, but it cannot leave out a directory. A build that
//! rewrites many outputs sends hundreds of thousands of events for `yak-out`, which overflow the
//! stream's queue. FSEvents then reports that events were dropped and asks for a rescan of the
//! whole project, and the watcher drops the DICE graph. With `FSEventStreamSetExclusionPaths`,
//! FSEvents never queues the events of the left-out directories. The stream reports its events as
//! `notify` events, translated from the FSEvents flags as `notify`'s own FSEvents watcher does.

use std::ffi::CStr;
use std::ffi::c_void;
use std::path::Path;
use std::path::PathBuf;
use std::ptr;
use std::sync::mpsc;
use std::thread;

use fsevent_sys as fs;
use fsevent_sys::core_foundation as cf;
use notify::Event;
use notify::EventKind;
use notify::event::CreateKind;
use notify::event::DataChange;
use notify::event::Flag;
use notify::event::MetadataKind;
use notify::event::ModifyKind;
use notify::event::RemoveKind;
use notify::event::RenameMode;
use yak_error::yak_error;

unsafe extern "C" {
    /// Whether the run loop is waiting for an event, which means it is running.
    fn CFRunLoopIsWaiting(runloop: cf::CFRunLoopRef) -> cf::Boolean;
}

/// FSEventStreamSetExclusionPaths accepts at most this many directories.
const MAX_EXCLUDED: usize = 8;

type Handler = Box<dyn FnMut(notify::Result<Event>) + Send>;

/// A CoreFoundation reference that is moved to the thread that owns it.
struct SendRef(cf::CFRef);

// SAFETY: CoreFoundation references may be moved across threads, and the run loop reference is
// only used to stop the run loop, which `CFRunLoopStop` allows from any thread.
unsafe impl Send for SendRef {}
unsafe impl Sync for SendRef {}

/// FsEventsWatcher delivers the events of a directory tree to a handler until it is dropped.
pub(crate) struct FsEventsWatcher {
    runloop: SendRef,
    thread: Option<thread::JoinHandle<()>>,
}

impl FsEventsWatcher {
    /// Watches `root` recursively, apart from the directories in `excluded`, which must be inside
    /// `root`. The handler runs on the watcher's thread.
    pub(crate) fn new(
        root: &Path,
        excluded: &[PathBuf],
        handler: Handler,
    ) -> yak_error::Result<FsEventsWatcher> {
        if excluded.len() > MAX_EXCLUDED {
            return Err(yak_error!(
                yak_error::ErrorTag::NotifyWatcher,
                "FSEvents can leave out at most {MAX_EXCLUDED} directories, not {}",
                excluded.len()
            ));
        }
        let root = path_str(root)?.to_owned();
        let excluded = excluded
            .iter()
            .map(|p| Ok(path_str(p)?.to_owned()))
            .collect::<yak_error::Result<Vec<_>>>()?;

        let (started_tx, started_rx) = mpsc::channel();
        let thread = thread::Builder::new()
            .name("yak-fsevents".to_owned())
            .spawn(move || run_stream(&root, &excluded, handler, started_tx))
            .map_err(|e| {
                yak_error!(
                    yak_error::ErrorTag::NotifyWatcher,
                    "Could not start the FSEvents thread: {e}"
                )
            })?;
        match started_rx.recv() {
            Ok(Ok(runloop)) => Ok(FsEventsWatcher {
                runloop,
                thread: Some(thread),
            }),
            Ok(Err(message)) => {
                join(thread);
                Err(yak_error!(yak_error::ErrorTag::NotifyWatcher, "{message}"))
            }
            Err(_) => {
                join(thread);
                Err(yak_error!(
                    yak_error::ErrorTag::NotifyWatcher,
                    "The FSEvents thread exited before it started the stream"
                ))
            }
        }
    }
}

impl Drop for FsEventsWatcher {
    fn drop(&mut self) {
        // SAFETY: the run loop belongs to the watcher's thread, which runs it until it stops. A
        // stop before the loop runs would be lost, so wait until it runs.
        unsafe {
            while CFRunLoopIsWaiting(self.runloop.0) == 0 {
                thread::yield_now();
            }
            cf::CFRunLoopStop(self.runloop.0);
        }
        if let Some(thread) = self.thread.take() {
            join(thread);
        }
    }
}

/// Waits for the watcher's thread. A panic of the thread has already been reported by the panic
/// hook, and the watcher has nothing left to clean up.
fn join(thread: thread::JoinHandle<()>) {
    thread.join().unwrap_or(());
}

fn path_str(path: &Path) -> yak_error::Result<&str> {
    path.to_str().ok_or_else(|| {
        yak_error!(
            yak_error::ErrorTag::NotifyWatcher,
            "FSEvents cannot watch `{}`, which is not UTF-8",
            path.display()
        )
    })
}

/// A CFArray of the paths as CFStrings, or `None` when a path cannot be converted.
unsafe fn path_array(paths: &[String]) -> Option<cf::CFMutableArrayRef> {
    unsafe {
        let array =
            cf::CFArrayCreateMutable(cf::kCFAllocatorDefault, 0, &cf::kCFTypeArrayCallBacks);
        for path in paths {
            let mut err: cf::CFErrorRef = ptr::null_mut();
            let cf_path = cf::str_path_to_cfstring_ref(path, &mut err);
            if cf_path.is_null() {
                if !err.is_null() {
                    cf::CFRelease(err as cf::CFRef);
                }
                cf::CFRelease(array);
                return None;
            }
            cf::CFArrayAppendValue(array, cf_path);
            cf::CFRelease(cf_path);
        }
        Some(array)
    }
}

/// The objects of a started stream, which `run_stream` releases after the run loop stops.
struct Stream {
    stream: fs::FSEventStreamRef,
    paths: cf::CFMutableArrayRef,
    exclusion: cf::CFMutableArrayRef,
}

/// Creates the stream on the current thread's run loop and starts it.
///
/// SAFETY: the caller runs the current thread's run loop and then calls `stop_stream`.
unsafe fn start_stream(
    root: &str,
    excluded: &[String],
    handler: Handler,
) -> Result<Stream, String> {
    unsafe {
        let paths = path_array(&[root.to_owned()])
            .ok_or_else(|| format!("FSEvents cannot watch `{root}`"))?;
        let Some(exclusion) = path_array(excluded) else {
            cf::CFRelease(paths);
            return Err(format!(
                "FSEvents cannot leave out the directories {excluded:?}"
            ));
        };

        let context = Box::into_raw(Box::new(handler));
        let stream_context = fs::FSEventStreamContext {
            version: 0,
            info: context as *mut c_void,
            retain: None,
            release: Some(release_context),
            copy_description: None,
        };
        let stream = fs::FSEventStreamCreate(
            cf::kCFAllocatorDefault,
            callback,
            &stream_context,
            paths,
            fs::kFSEventStreamEventIdSinceNow,
            0.0,
            fs::kFSEventStreamCreateFlagFileEvents | fs::kFSEventStreamCreateFlagNoDefer,
        );
        if stream.is_null() {
            drop(Box::from_raw(context));
            cf::CFRelease(paths);
            cf::CFRelease(exclusion);
            return Err(format!("FSEvents cannot watch `{root}`"));
        }
        let started = Stream {
            stream,
            paths,
            exclusion,
        };
        if !excluded.is_empty() && fs::FSEventStreamSetExclusionPaths(stream, exclusion) == 0 {
            release_stream(started);
            return Err(format!(
                "FSEvents cannot leave out the directories {excluded:?}"
            ));
        }
        fs::FSEventStreamScheduleWithRunLoop(
            stream,
            cf::CFRunLoopGetCurrent(),
            cf::kCFRunLoopDefaultMode,
        );
        if fs::FSEventStreamStart(stream) == 0 {
            release_stream(started);
            return Err(format!("FSEvents cannot start watching `{root}`"));
        }
        Ok(started)
    }
}

/// Stops and releases a stream, which releases its handler.
unsafe fn release_stream(stream: Stream) {
    unsafe {
        fs::FSEventStreamStop(stream.stream);
        fs::FSEventStreamInvalidate(stream.stream);
        fs::FSEventStreamRelease(stream.stream);
        cf::CFRelease(stream.paths);
        cf::CFRelease(stream.exclusion);
    }
}

/// Starts the stream, reports the thread's run loop through `started`, and runs the loop until
/// `FsEventsWatcher` stops it.
fn run_stream(
    root: &str,
    excluded: &[String],
    handler: Handler,
    started: mpsc::Sender<Result<SendRef, String>>,
) {
    // SAFETY: every CoreFoundation object is created, used, and released on this thread.
    unsafe {
        match start_stream(root, excluded, handler) {
            Ok(stream) => {
                if started.send(Ok(SendRef(cf::CFRunLoopGetCurrent()))).is_ok() {
                    cf::CFRunLoopRun();
                }
                release_stream(stream);
            }
            // The receiver waits for this message, so it can only be gone after a panic.
            Err(message) => started.send(Err(message)).unwrap_or(()),
        }
    }
}

extern "C" fn release_context(info: *const c_void) {
    // SAFETY: the stream calls `release` once, when it is released, with the `info` pointer that
    // `run_stream` created from a `Box<Handler>`.
    unsafe { drop(Box::from_raw(info as *mut Handler)) };
}

extern "C" fn callback(
    _stream: fs::FSEventStreamRef,
    info: *mut c_void,
    num_events: usize,
    event_paths: *mut c_void,
    event_flags: *const fs::FSEventStreamEventFlags,
    _event_ids: *const fs::FSEventStreamEventId,
) {
    // SAFETY: FSEvents passes `num_events` C strings and flags, and `info` is the `Box<Handler>`
    // of `run_stream`, which only this thread uses.
    unsafe {
        let handler = &mut *(info as *mut Handler);
        let event_paths = event_paths as *const *const std::ffi::c_char;
        for i in 0..num_events {
            let path = CStr::from_ptr(*event_paths.add(i));
            let path = PathBuf::from(path.to_string_lossy().into_owned());
            for event in translate_flags(*event_flags.add(i)) {
                handler(Ok(event.add_path(path.clone())));
            }
        }
    }
}

fn has(flags: u32, flag: u32) -> bool {
    flags & flag != 0
}

/// The events that FSEvents flags describe, as `notify`'s FSEvents watcher translates them.
fn translate_flags(flags: u32) -> Vec<Event> {
    let mut events = Vec::new();
    if has(flags, fs::kFSEventStreamEventFlagHistoryDone) {
        return events;
    }
    if has(flags, fs::kFSEventStreamEventFlagMustScanSubDirs) {
        // The flags say whether the kernel (0x4) or the stream (0x2) dropped the events.
        tracing::debug!("FSEvents asked for a rescan, with flags {flags:#x}");
        events.push(Event::new(EventKind::Other).set_flag(Flag::Rescan));
    }
    if has(flags, fs::kFSEventStreamEventFlagRootChanged) {
        events.push(Event::new(EventKind::Modify(ModifyKind::Name(
            RenameMode::From,
        ))));
    }
    if has(flags, fs::kFSEventStreamEventFlagMount) {
        events.push(Event::new(EventKind::Create(CreateKind::Other)));
    }
    if has(flags, fs::kFSEventStreamEventFlagUnmount) {
        events.push(Event::new(EventKind::Remove(RemoveKind::Other)));
    }
    let is_dir = has(flags, fs::kFSEventStreamEventFlagItemIsDir);
    let is_file = has(flags, fs::kFSEventStreamEventFlagItemIsFile);
    let is_link = has(flags, fs::kFSEventStreamEventFlagItemIsSymlink)
        || has(flags, fs::kFSEventStreamEventFlagItemIsHardlink)
        || has(flags, fs::kFSEventStreamEventFlagItemCloned);
    if has(flags, fs::kFSEventStreamEventFlagItemCreated) {
        events.push(Event::new(EventKind::Create(if is_dir {
            CreateKind::Folder
        } else if is_file {
            CreateKind::File
        } else if is_link {
            CreateKind::Other
        } else {
            CreateKind::Any
        })));
    }
    if has(flags, fs::kFSEventStreamEventFlagItemRemoved) {
        events.push(Event::new(EventKind::Remove(if is_dir {
            RemoveKind::Folder
        } else if is_file {
            RemoveKind::File
        } else if is_link {
            RemoveKind::Other
        } else {
            RemoveKind::Any
        })));
    }
    if has(flags, fs::kFSEventStreamEventFlagItemRenamed) {
        events.push(Event::new(EventKind::Modify(ModifyKind::Name(
            RenameMode::Any,
        ))));
    }
    if has(flags, fs::kFSEventStreamEventFlagItemInodeMetaMod) {
        events.push(Event::new(EventKind::Modify(ModifyKind::Metadata(
            MetadataKind::Any,
        ))));
    }
    if has(flags, fs::kFSEventStreamEventFlagItemFinderInfoMod) {
        events.push(Event::new(EventKind::Modify(ModifyKind::Metadata(
            MetadataKind::Other,
        ))));
    }
    if has(flags, fs::kFSEventStreamEventFlagItemChangeOwner) {
        events.push(Event::new(EventKind::Modify(ModifyKind::Metadata(
            MetadataKind::Ownership,
        ))));
    }
    if has(flags, fs::kFSEventStreamEventFlagItemXattrMod) {
        events.push(Event::new(EventKind::Modify(ModifyKind::Metadata(
            MetadataKind::Extended,
        ))));
    }
    if has(flags, fs::kFSEventStreamEventFlagItemModified) {
        events.push(Event::new(EventKind::Modify(ModifyKind::Data(
            DataChange::Content,
        ))));
    }
    events
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;
    use std::sync::Mutex;
    use std::time::Duration;
    use std::time::Instant;

    use super::*;

    #[test]
    fn test_translate_flags() {
        let created_file = translate_flags(
            fs::kFSEventStreamEventFlagItemCreated | fs::kFSEventStreamEventFlagItemIsFile,
        );
        assert_eq!(created_file.len(), 1);
        assert_eq!(created_file[0].kind, EventKind::Create(CreateKind::File));

        let rescan = translate_flags(
            fs::kFSEventStreamEventFlagMustScanSubDirs | fs::kFSEventStreamEventFlagUserDropped,
        );
        assert!(rescan[0].need_rescan());

        assert!(translate_flags(fs::kFSEventStreamEventFlagHistoryDone).is_empty());
    }

    /// Waits until `events` holds a path that satisfies `found`.
    fn wait_for(events: &Mutex<Vec<PathBuf>>, found: impl Fn(&Path) -> bool) -> bool {
        let deadline = Instant::now() + Duration::from_secs(10);
        while Instant::now() < deadline {
            if events.lock().unwrap().iter().any(|p| found(p)) {
                return true;
            }
            thread::sleep(Duration::from_millis(20));
        }
        false
    }

    #[test]
    fn test_excluded_directory_sends_no_events() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path().canonicalize().unwrap();
        let excluded = root.join("yak-out");
        std::fs::create_dir(&excluded).unwrap();

        let events = Arc::new(Mutex::new(Vec::new()));
        let events2 = events.clone();
        let watcher = FsEventsWatcher::new(
            &root,
            std::slice::from_ref(&excluded),
            Box::new(move |event| {
                events2.lock().unwrap().extend(event.unwrap().paths);
            }),
        )
        .unwrap();

        std::fs::write(excluded.join("output"), "out").unwrap();
        std::fs::write(root.join("source"), "src").unwrap();
        // Events arrive in order, so the event of `source` follows any event of `output`.
        assert!(wait_for(&events, |p| p.ends_with("source")));
        drop(watcher);
        assert!(
            !events
                .lock()
                .unwrap()
                .iter()
                .any(|p| p.starts_with(&excluded)),
            "{:?}",
            events.lock().unwrap()
        );
    }
}
