use super::*;

pub(crate) async fn output_watchdog_loop(state: AgentState, interval_duration: Duration) {
    let mut interval = tokio::time::interval(interval_duration);
    interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);

    loop {
        interval.tick().await;
        if !state.hardware_output_enabled()
            || state.manual_output_override_active()
            || !state.has_non_neutral_output_frames()
        {
            continue;
        }

        let game_detection = state.cached_hardware_game_detection().await;
        let should_neutralize = {
            let inner = state.inner.read().await;
            !hardware_output_any_allowed(&inner, Some(&game_detection))
        };

        if should_neutralize {
            state
                .neutralize_active_output_and_release("the supported-game telemetry gate closed")
                .await;
        }
    }
}
