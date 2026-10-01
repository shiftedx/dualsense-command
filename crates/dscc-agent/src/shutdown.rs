use super::*;
use std::io::Read;

const CLEANUP_BUDGET: Duration = Duration::from_secs(2);

#[derive(Default)]
pub(crate) struct ShutdownGate {
    stopping: AtomicBool,
    writes: Mutex<()>,
}

impl ShutdownGate {
    pub(crate) fn begin(&self) {
        self.stopping.store(true, Ordering::SeqCst);
    }
    pub(crate) fn is_stopping(&self) -> bool {
        self.stopping.load(Ordering::SeqCst)
    }
    pub(crate) fn write<T>(&self, write: impl FnOnce() -> T) -> Result<T, &'static str> {
        let _guard = self.writes.lock().unwrap_or_else(|e| e.into_inner());
        if self.is_stopping() {
            return Err("agent is shutting down");
        }
        Ok(write())
    }
}

impl AgentState {
    pub(crate) async fn shutdown_outputs(&self) {
        self.shutdown.begin();
        self.input_bridge.begin_shutdown();
        let hardware = async {
            let targets = {
                let inner = self.inner.read().await;
                inner
                    .controllers
                    .summaries()
                    .iter()
                    .filter_map(|controller| {
                        controller_output_target_or_reason(&inner, &controller.id).ok()
                    })
                    .collect::<Vec<_>>()
            };
            let state = self.clone();
            tokio::task::spawn_blocking(move || {
                // The gate is held inside blocking tasks, so cancellation cannot let
                // an already queued effect write overtake shutdown neutralization.
                let _guard = state
                    .shutdown
                    .writes
                    .lock()
                    .unwrap_or_else(|e| e.into_inner());
                if let Some(manager) = &state.output_manager {
                    for target in targets {
                        if manager
                            .write_frame(&target, &ControllerOutputFrame::default())
                            .is_err()
                        {
                            tracing::warn!("Shutdown controller neutralization failed");
                        }
                    }
                    manager.release_all();
                }
                state.clear_recorded_output_frames();
            })
            .await
        };
        let bridge = self.input_bridge.run_blocking(|bridge| bridge.shutdown());
        if tokio::time::timeout(CLEANUP_BUDGET, async {
            let _ = tokio::join!(hardware, bridge);
        })
        .await
        .is_err()
        {
            tracing::warn!(
                "Output shutdown exceeded its cleanup budget; neutralization is unconfirmed"
            );
        }
    }
}

pub(crate) async fn requested() {
    let (tx, mut rx) = tokio::sync::oneshot::channel();
    let mut sender = Some(tx);
    // Only a tray-owned inherited pipe opts into this channel. EOF also handles
    // an abruptly exited tray without exposing a network shutdown endpoint.
    if std::env::var("DSCC_TRAY_STDIN_SHUTDOWN").as_deref() == Ok("1") {
        let tx = sender.take().expect("shutdown sender");
        std::thread::spawn(move || {
            let _ = read_stop(std::io::stdin().lock());
            let _ = tx.send(());
        });
    }
    #[cfg(unix)]
    {
        let mut terminate =
            tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate())
                .expect("install SIGTERM handler");
        tokio::select! { _ = tokio::signal::ctrl_c() => {}, _ = terminate.recv() => {}, _ = &mut rx => {} }
    }
    #[cfg(not(unix))]
    tokio::select! { _ = tokio::signal::ctrl_c() => {}, _ = &mut rx => {} }
}

fn read_stop(mut input: impl Read) -> io::Result<()> {
    let mut byte = [0];
    loop {
        match input.read(&mut byte)? {
            0 => return Ok(()),
            _ if byte[0] == b'\n' => return Ok(()),
            _ => {}
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn stop_pipe_accepts_request_and_eof() {
        assert!(read_stop(&b"shutdown\n"[..]).is_ok());
        assert!(read_stop(&b""[..]).is_ok());
    }
    #[test]
    fn queued_output_cannot_follow_shutdown_neutralization() {
        let gate = Arc::new(ShutdownGate::default());
        let held = gate.writes.lock().unwrap();
        let queued = gate.clone();
        let worker = std::thread::spawn(move || queued.write(|| panic!("late effect write")));
        gate.begin();
        drop(held);
        assert!(worker.join().unwrap().is_err());
    }
    #[tokio::test]
    async fn shutdown_stops_virtual_sessions_and_rejects_restarts() {
        let bridge = InputBridgeService::mock();
        bridge
            .start_session("test", VirtualOutputKind::Xbox360, 0)
            .unwrap();
        bridge.begin_shutdown();
        bridge.run_blocking(|b| b.shutdown()).await;
        assert!(!bridge.is_active("test"));
        assert!(bridge
            .start_session("test", VirtualOutputKind::Xbox360, 1)
            .is_err());
    }
}
