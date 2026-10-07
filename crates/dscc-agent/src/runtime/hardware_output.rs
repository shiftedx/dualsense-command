use super::*;

pub(crate) async fn hardware_output_loop(state: AgentState, interval_duration: Duration) {
    let mut interval = tokio::time::interval(interval_duration);
    interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
    loop {
        interval.tick().await;
        if !state.hardware_output_enabled() {
            continue;
        }

        let game_detection = state.hardware_game_detection_snapshot();

        if let Err(error) = state
            .write_current_output_frame_if_due(game_detection.as_ref())
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
