use std::sync::OnceLock;

use tokio::runtime::{Handle, Runtime};

static FALLBACK_RUNTIME: OnceLock<Runtime> = OnceLock::new();

/// Returns a lazily initialized fallback Tokio runtime for callers outside an
/// existing runtime (such as CLI or standalone library use).
///
/// Inside the GUI application, Iced already owns the multi-threaded Tokio
/// runtime; prefer [`spawn_blocking`], which reuses the ambient runtime via
/// [`Handle::try_current`] without spinning up a second thread pool.
pub fn runtime() -> &'static Runtime {
    FALLBACK_RUNTIME.get_or_init(|| {
        tokio::runtime::Builder::new_multi_thread()
            .enable_all()
            .worker_threads(2)
            .thread_name("imgeditor-tokio")
            .build()
            .expect("failed to build tokio runtime")
    })
}

/// Runs a blocking closure on the current Tokio runtime's blocking pool when
/// called from within an active runtime (such as Iced's executor), falling back
/// to [`runtime`] only when no Tokio runtime is entered.
pub fn spawn_blocking<F, R>(function: F) -> tokio::task::JoinHandle<R>
where
    F: FnOnce() -> R + Send + 'static,
    R: Send + 'static,
{
    match Handle::try_current() {
        Ok(handle) => handle.spawn_blocking(function),
        Err(_) => runtime().spawn_blocking(function),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn spawn_blocking_reuses_ambient_tokio_runtime_without_building_fallback() {
        let rt = tokio::runtime::Builder::new_current_thread()
            .enable_time()
            .build()
            .unwrap();
        let ambient_id = rt.handle().id();

        let (value, ran_on_id) = rt.block_on(async {
            spawn_blocking(move || (40 + 2, Handle::current().id()))
                .await
                .unwrap()
        });

        assert_eq!(value, 42);
        assert_eq!(ran_on_id, ambient_id);
        assert!(
            FALLBACK_RUNTIME.get().is_none(),
            "ambient Tokio runtime should be reused without starting a second runtime"
        );
    }
}
