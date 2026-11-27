#[global_allocator]
static ALLOC: rusty_alloc_api::RustyAlloc = rusty_alloc_api::RustyAlloc;

/// Initialize `rusty_alloc` options on the main thread before worker pools
/// start; call first in `main`.
///
/// Starting with `rusty_alloc` 2.4+, purging is enabled by default with a
/// 10 ms `purge_delay`, arenas reserve address space without committing,
/// and segments commit on demand. Leaving the shipped default preserves
/// short-lived buffer reuse while returning idle memory to the OS
/// (~17 MiB WS / ~15 MiB private commit after a 2,000-scene burst; see
/// `docs/allocator-rusty-alloc.md`). Touching the option table here parses
/// any `RUSTY_ALLOC_*` / `MIMALLOC_*` overrides once, single-threaded.
pub fn init_allocator() {
    let _ = rusty_alloc::options::get(15); // purge_delay (ABI index)
}

/// Actively reclaim idle allocator memory at major teardown boundaries
/// (such as closing an archive tab).
///
/// Calls `rusty_alloc::alloc::collect(true)`, which adopts abandoned
/// segments from exited worker threads, retires empty pages, purges free
/// slots on partially-used medium pages (`MEM_RESET` on Windows), and
/// flushes pending span and arena-chunk purges immediately.
pub fn reclaim_allocator_memory() {
    rusty_alloc::alloc::collect(true);
}

pub mod archive;
pub mod collections;
pub mod compare;
pub mod compat;
pub mod config;
pub mod dev_logger;
pub mod editor;
pub mod file_association;
pub mod i18n;
pub mod inspector;
pub mod parser;
pub mod runtime;
pub mod search;
pub mod session;
pub mod sort;
pub mod tasks;
#[cfg(test)]
pub(crate) mod test_paths;
pub mod ui;
pub mod updater;
pub mod utils;

#[cfg(test)]
mod allocator_tests {
    use super::{init_allocator, reclaim_allocator_memory};

    #[test]
    fn allocator_init_and_reclaim_keep_purging_enabled() {
        init_allocator();
        let delay = rusty_alloc::options::get(15);
        assert!(
            delay >= 0,
            "expected rusty_alloc 2.6 purge_delay >= 0 by default, got {delay}"
        );

        // Exercise a mix of medium-page (32 KiB) and large (1 MiB) buffers
        // with one retained allocation, then force a teardown collect.
        let pinned = vec![0xA5u8; 32 * 1024];
        let mut scratch: Vec<Vec<u8>> = (0..64).map(|_| vec![0x5Au8; 32 * 1024]).collect();
        scratch.push(vec![0x3Cu8; 2 * 1024 * 1024]);
        drop(scratch);

        reclaim_allocator_memory();

        assert_eq!(pinned[0], 0xA5);
        assert_eq!(pinned[pinned.len() - 1], 0xA5);
        assert_eq!(rusty_alloc::stats::commit_failures(), 0);
        let (_, _, _, rss, peak_rss, commit, peak_commit, _) = rusty_alloc::stats::process_info();
        assert!(rss > 0 && peak_rss >= rss);
        assert!(commit > 0 && peak_commit >= commit);
    }
}
