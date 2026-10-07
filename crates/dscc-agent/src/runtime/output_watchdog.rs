use super::*;

pub(crate) async fn output_watchdog_loop(state: AgentState, interval_duration: Duration) {
    let mut interval = tokio::time::interval(interval_duration);
    interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);

    loop {
        interval.tick().await;
        if !state.hardware_output_enabled() || !state.has_non_neutral_output_frames() {
            continue;
        }

        output_watchdog_tick(&state).await;
    }
}

async fn output_watchdog_tick(state: &AgentState) {
    let game_detection = state.hardware_game_detection_snapshot();
    let should_neutralize = if game_detection.is_none() {
        // Retain Global Profile lighting while dropping game actuators on
        // expired detection, even if a catalog scan is still blocked.
        state
            .lock_output_runtime()
            .last_output_frames
            .values()
            .any(|last| {
                last.frame.l2 != TriggerOutput::Off
                    || last.frame.r2 != TriggerOutput::Off
                    || last.frame.rumble != ControllerOutputFrame::default().rumble
            })
    } else {
        let inner = state.inner.read().await;
        !hardware_output_any_allowed(&inner, game_detection.as_ref())
    };

    if should_neutralize {
        state
            .neutralize_active_output_and_release("the supported-game telemetry gate closed")
            .await;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn unavailable_discovery_preserves_global_lighting() {
        let mut state = AgentState::from_controller_events([]);
        state.output_recorder = Some(Arc::new(Mutex::new(TestOutputRecorder::default())));
        let _busy = state.discovery_cache.game_detection.lock().await;
        state.record_output_frame_write(
            "mock-controller",
            &ControllerOutputFrame {
                lightbar: Some(LightbarOutput {
                    color: RgbColor {
                        red: 10,
                        green: 20,
                        blue: 30,
                    },
                    brightness: 0.5,
                }),
                ..ControllerOutputFrame::default()
            },
            DeviceTransportKind::Usb,
            Instant::now(),
        );
        output_watchdog_tick(&state).await;
        assert!(state.has_non_neutral_output_frames());
    }

    #[tokio::test]
    async fn slow_discovery_does_not_block_watchdog_neutralization() {
        let mut state = AgentState::from_controller_events([]);
        state.output_recorder = Some(Arc::new(Mutex::new(TestOutputRecorder::default())));
        // Simulate a discovery worker holding both shared discovery locks.
        let detection_lock = state.discovery_cache.game_detection.lock().await;
        let catalog_lock = state.discovery_cache.steam_game_catalog.lock().await;
        state.record_output_frame_write(
            "mock-controller",
            &ControllerOutputFrame {
                rumble: Some(RumbleOutput {
                    low_frequency: 0.5,
                    high_frequency: 0.5,
                }),
                ..ControllerOutputFrame::default()
            },
            DeviceTransportKind::Usb,
            Instant::now(),
        );
        assert!(state.has_non_neutral_output_frames());
        tokio::time::timeout(HARDWARE_OUTPUT_INTERVAL, output_watchdog_tick(&state))
            .await
            .expect("watchdog must not wait for discovery");
        assert!(!state.has_non_neutral_output_frames());
        for _ in 0..10 {
            assert!(state.hardware_game_detection_snapshot().is_none());
            tokio::task::yield_now().await;
        }
        drop(catalog_lock);
        drop(detection_lock);
    }
}
