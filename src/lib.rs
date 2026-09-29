#[global_allocator]
static ALLOC: rusty_alloc_api::RustyAlloc = rusty_alloc_api::RustyAlloc;

/// Enable rusty_alloc's span purge at process start; call first in `main`.
///
/// The crate ships `purge_delay = -1` (purging opt-in), which on our
/// workloads holds ~1 GiB commit / 236 MiB resident after a 220 MiB decode
/// session where the CRT heap returns to 6 MiB / 13 MiB. Setting the option
/// to 0 makes every coalesced free span purge on release; measured parity
/// with the CRT's resident footprint at no speed cost (export and table
/// A/B, 2026-09-30 — docs/allocator-rusty-alloc.md). The activated path is
/// the audited `span_free` purge with recommit marking; the M8 arena-recycle
/// defect is unreachable because no arena regions are registered.
pub fn init_allocator() {
    rusty_alloc::options::set_default(15, 0); // purge_delay (ABI index)
}

pub mod archive;
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
