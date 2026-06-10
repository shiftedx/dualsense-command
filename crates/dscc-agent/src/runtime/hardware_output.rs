use super::*;

pub(crate) async fn hardware_output_loop(state: AgentState, interval_duration: Duration) {
    let mut interval = tokio::time::interval(interval_duration);
    interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
    let mut game_detection = state.cached_hardware_game_detection().await;
    let mut next_detection_refresh = Instant::now() + HARDWARE_GAME_DETECTION_INTERVAL;
    loop {
        interval.tick().await;
        if !state.hardware_output_enabled() || state.manual_output_override_active() {
            continue;
        }

        let now = Instant::now();
        if now >= next_detection_refresh {
            game_detection = state.cached_hardware_game_detection().await;
            next_detection_refresh = now + HARDWARE_GAME_DETECTION_INTERVAL;
        }

        if let Err(error) = state
            .write_current_output_frame_if_due(Some(&game_detection))
            .await
        {
            state
                .note_hardware_output_error(format!(
                    "Hardware trigger output write failed: {error}"
                ))
                .await;
        }
    }
}
