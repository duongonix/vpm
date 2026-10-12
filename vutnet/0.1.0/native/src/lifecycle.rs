use server_native::ShutdownHandle;

pub(crate) async fn wait_for_shutdown_signal(shutdown: ShutdownHandle) {
    #[cfg(unix)]
    wait_for_unix_signal(shutdown).await;

    #[cfg(windows)]
    wait_for_windows_signal(shutdown).await;

    #[cfg(not(any(unix, windows)))]
    wait_for_ctrl_c(shutdown).await;
}

#[cfg(unix)]
async fn wait_for_unix_signal(shutdown: ShutdownHandle) {
    let mut terminate =
        match tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate()) {
            Ok(signal) => signal,
            Err(_) => {
                if tokio::signal::ctrl_c().await.is_ok() {
                    shutdown.shutdown();
                }
                return;
            }
        };

    tokio::select! {
        result = tokio::signal::ctrl_c() => {
            if result.is_err() {
                let _ = terminate.recv().await;
            }
        }
        _ = terminate.recv() => {}
    }
    shutdown.shutdown();
}

#[cfg(windows)]
async fn wait_for_windows_signal(shutdown: ShutdownHandle) {
    match tokio::signal::windows::ctrl_break() {
        Ok(mut ctrl_break) => {
            tokio::select! {
                result = tokio::signal::ctrl_c() => {
                    if result.is_ok() || ctrl_break.recv().await.is_some() {
                        shutdown.shutdown();
                    }
                }
                event = ctrl_break.recv() => {
                    if event.is_some() || tokio::signal::ctrl_c().await.is_ok() {
                        shutdown.shutdown();
                    }
                }
            }
        }
        Err(_) => {
            if tokio::signal::ctrl_c().await.is_ok() {
                shutdown.shutdown();
            }
        }
    }
}

#[cfg(not(any(unix, windows)))]
async fn wait_for_ctrl_c(shutdown: ShutdownHandle) {
    if tokio::signal::ctrl_c().await.is_ok() {
        shutdown.shutdown();
    }
}
