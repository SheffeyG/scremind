mod hooks;
mod render;
mod runtime;
mod types;

use std::sync::atomic::{AtomicBool, Ordering};

pub use types::OverlayParams;

static OVERLAY_ACTIVE: AtomicBool = AtomicBool::new(false);

struct OverlayActivationGuard;

impl OverlayActivationGuard {
    fn try_acquire() -> Option<Self> {
        OVERLAY_ACTIVE
            .compare_exchange(false, true, Ordering::SeqCst, Ordering::SeqCst)
            .ok()
            .map(|_| Self)
    }
}

impl Drop for OverlayActivationGuard {
    fn drop(&mut self) {
        OVERLAY_ACTIVE.store(false, Ordering::SeqCst);
    }
}

pub fn show_overlay_with_params(params: OverlayParams) {
    let Some(activation_guard) = OverlayActivationGuard::try_acquire() else {
        log::warn!("Overlay already active, skipping");
        return;
    };

    log::info!(
        "Showing overlay: time_str={}, fps={}, font_size={}",
        params.time_str,
        params.fps,
        params.font_size
    );

    std::thread::spawn(move || {
        let _activation_guard = activation_guard;
        if let Err(e) = runtime::OverlayRuntime::run(params) {
            log::error!("Overlay error: {}", e);
        }
        log::info!("Overlay closed");
    });
}
