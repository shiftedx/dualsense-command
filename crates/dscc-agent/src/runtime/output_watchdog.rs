use super::*;

pub(crate) async fn output_watchdog_loop(state: AgentState, interval_duration: Duration) {
    let mut interval = tokio::time::interval(interval_duration);
    interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);

    loop {
        interval.tick().await;
        if !state.hardware_output_enabled() {
            continue;
        }

        let game_detection = state.hardware_game_detection_snapshot();
        output_watchdog_tick(&state, game_detection.as_ref()).await;
    }
}

async fn output_watchdog_tick(state: &AgentState, game_detection: Option<&GameDetectionResponse>) {
    state.retry_expired_manual_outputs().await;
    if !state.has_non_neutral_output_frames() {
        return;
    }
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
        !hardware_output_any_allowed(&inner, game_detection)
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
        output_watchdog_tick(&state, None).await;
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
        tokio::time::timeout(HARDWARE_OUTPUT_INTERVAL, output_watchdog_tick(&state, None))
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

    #[tokio::test]
    async fn audit_failed_manual_expiry_retries_while_another_controller_is_live() {
        let mut state = AgentState::mock();
        state.output_recorder = Some(Arc::new(Mutex::new(TestOutputRecorder::default())));
        let detection = detect_running_game_from_processes(["ForzaHorizon6.exe"]);
        let (controller_a, controller_b) = {
            let mut inner = state.inner.write().await;
            inner
                .adapter_runtime_mut(FORZA_DATA_OUT_ADAPTER_ID)
                .mark_packet(324, 1);
            inner.telemetry = SignalSnapshot::from_updates([
                signal_update("source.id", FORZA_DATA_OUT_ADAPTER_ID),
                signal_update("game.state", "driving"),
                signal_update("input.throttle", 0.6),
                signal_update("input.brake", 0.5),
            ]);
            let resolution = profile_resolution(&inner, Some(&detection));
            assert!(hardware_output_runtime_allowed_for_resolution(
                &inner,
                Some(&detection),
                &resolution
            ));
            let controller_a = resolution.controller_id.unwrap();
            let controller_b = inner
                .controllers
                .summaries()
                .into_iter()
                .find(|controller| controller.id != controller_a)
                .unwrap()
                .id;
            (controller_a, controller_b)
        };
        state
            .write_current_output_frame_if_due(Some(&detection))
            .await
            .unwrap();
        let frame_a = state.lock_output_runtime().last_output_frames[&controller_a]
            .frame
            .clone();
        let request = serde_json::from_value(
            serde_json::json!({"target":"r2", "mode":"wall", "intensity":70, "durationMs":100}),
        )
        .unwrap();
        assert!(
            crate::api::run_effect_test_for_controller(
                controller_b.clone(),
                state.clone(),
                request
            )
            .await
            .unwrap()
            .1
             .0
            .accepted
        );
        state
            .output_recorder
            .as_ref()
            .unwrap()
            .lock()
            .unwrap()
            .fail_next = true;
        tokio::time::sleep(Duration::from_millis(160)).await;
        assert!(
            !state
                .output_recorder
                .as_ref()
                .unwrap()
                .lock()
                .unwrap()
                .fail_next,
            "expiry attempted the failing neutral write"
        );
        assert!(state
            .lock_output_runtime()
            .manual_overrides
            .contains_key(&controller_b));
        assert_ne!(
            state.lock_output_runtime().last_output_frames[&controller_b].frame,
            ControllerOutputFrame::default()
        );
        assert!(hardware_output_any_allowed(
            &*state.inner.read().await,
            Some(&detection)
        ));
        output_watchdog_tick(&state, Some(&detection)).await;
        let writes = state
            .output_recorder
            .as_ref()
            .unwrap()
            .lock()
            .unwrap()
            .writes
            .clone();
        assert!(
            writes.iter().any(
                |(id, frame)| id == &controller_b && *frame == ControllerOutputFrame::default()
            ),
            "expired B must retry even though A remains live"
        );
        let runtime = state.lock_output_runtime();
        assert!(!runtime.manual_overrides.contains_key(&controller_b));
        assert!(!runtime.last_output_frames.contains_key(&controller_b));
        assert_eq!(runtime.last_output_frames[&controller_a].frame, frame_a);
        assert_eq!(
            writes.iter().filter(|(id, _)| id == &controller_a).count(),
            1
        );
    }
}
