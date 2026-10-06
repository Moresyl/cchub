use std::sync::atomic::{AtomicU8, Ordering};
use std::time::Duration;

// ExitRequested is repeated by app.exit after the accounting worker finishes.
static EXIT: AtomicU8 = AtomicU8::new(0);

pub(crate) fn handle_exit<R: tauri::Runtime>(app: &tauri::AppHandle<R>, event: tauri::RunEvent) {
    let tauri::RunEvent::ExitRequested { api, code, .. } = event else {
        return;
    };
    if EXIT.load(Ordering::Acquire) == 2 {
        return;
    }
    api.prevent_exit();
    if EXIT
        .compare_exchange(0, 1, Ordering::AcqRel, Ordering::Acquire)
        .is_err()
    {
        return;
    }
    super::dispatch::close_requests();
    super::lifecycle::lifecycle().close();
    let app = app.clone();
    let started = std::thread::Builder::new()
        .name("proxy-accounting-exit".into())
        .spawn(move || {
            loop {
                match super::drain_accounting(Duration::from_secs(5)) {
                    Ok(()) if super::dispatch::active_requests() == 0 => break,
                    Ok(()) => std::thread::sleep(Duration::from_millis(25)),
                    Err(_) => crate::utils::append_runtime_log(
                        "warn",
                        "provider_proxy",
                        "Waiting for proxy accounting before exiting",
                    ),
                }
            }
            EXIT.store(2, Ordering::Release);
            app.exit(code.unwrap_or(0));
        });
    if started.is_err() {
        EXIT.store(0, Ordering::Release);
        crate::utils::append_runtime_log(
            "warn",
            "provider_proxy",
            "Could not start the accounting exit worker; exit can be retried",
        );
    }
}
