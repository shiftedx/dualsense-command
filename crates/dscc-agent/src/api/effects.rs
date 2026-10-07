use super::*;

pub(crate) async fn test_effect(
    Path(id): Path<String>,
    State(state): State<AgentState>,
    Json(request): Json<EffectTestRequest>,
) -> Result<(StatusCode, Json<EffectTestResponse>), StatusCode> {
    run_effect_test_for_controller(id, state, request).await
}

pub(crate) async fn test_current_effect(
    State(state): State<AgentState>,
    Json(request): Json<EffectTestRequest>,
) -> Result<(StatusCode, Json<EffectTestResponse>), StatusCode> {
    let id = {
        let inner = state.inner.read().await;
        inner
            .controllers
            .summaries()
            .into_iter()
            .find(|controller| controller.connected)
            .map(|controller| controller.id)
            .ok_or(StatusCode::NOT_FOUND)?
    };

    run_effect_test_for_controller(id, state, request).await
}

pub(crate) async fn run_effect_test_for_controller(
    id: String,
    state: AgentState,
    request: EffectTestRequest,
) -> Result<(StatusCode, Json<EffectTestResponse>), StatusCode> {
    {
        let inner = state.inner.read().await;
        let detail = inner.controllers.detail(&id).ok_or(StatusCode::NOT_FOUND)?;

        if detail.permission == ControllerPermissionState::Denied {
            return Ok((
                StatusCode::CONFLICT,
                Json(EffectTestResponse {
                    accepted: false,
                    message: format!(
                        "Controller {id} requires device permission before effect tests"
                    ),
                    dry_run: true,
                    duration_ms: 0,
                    output: ControllerOutputFrame::default(),
                }),
            ));
        }
    }

    let target = request.target.as_deref().unwrap_or("r2").to_string();
    let mode = request
        .mode
        .as_deref()
        .unwrap_or("adaptive_resistance")
        .to_string();
    let stop_manual_override = target == "base_feel" && mode == "off";
    let duration_ms = if stop_manual_override {
        0
    } else if target == "base_feel" {
        request
            .duration_ms
            .unwrap_or(DEFAULT_BASE_FEEL_TEST_DURATION_MS)
            .clamp(500, MAX_BASE_FEEL_TEST_DURATION_MS)
    } else {
        request
            .duration_ms
            .unwrap_or(DEFAULT_EFFECT_TEST_DURATION_MS)
            .clamp(100, MAX_EFFECT_TEST_DURATION_MS)
    };
    let output = if stop_manual_override {
        ControllerOutputFrame::default()
    } else {
        effect_test_output_frame(&request)
    };
    let base_feel_trigger = if target == "base_feel" && !stop_manual_override {
        Some(request.trigger.clone().unwrap_or_default())
    } else {
        None
    };
    let hardware_output_enabled = state.hardware_output_enabled();
    let mut accepted = true;
    let mut status = StatusCode::ACCEPTED;
    let mut message = if hardware_output_enabled {
        let (generation, deadline) = state
            .begin_manual_output_override(&id, Duration::from_millis(duration_ms))
            .await;
        match state
            .write_output_frame_with_owner(
                &id,
                &output,
                if stop_manual_override {
                    OutputWriteOwner::FinishManual(generation)
                } else {
                    OutputWriteOwner::Manual(generation)
                },
            )
            .await
        {
            Ok(None) => {
                let _ = state
                    .write_output_frame_with_owner(
                        &id,
                        &ControllerOutputFrame::default(),
                        OutputWriteOwner::FinishManual(generation),
                    )
                    .await;
                accepted = false;
                status = StatusCode::CONFLICT;
                format!("Hardware effect test for controller {id} was superseded")
            }
            Ok(Some(write)) => {
                if !stop_manual_override {
                    let state_for_reset = state.clone();
                    let id_for_reset = id.clone();
                    let output_for_refresh = output.clone();
                    let base_feel_trigger = base_feel_trigger.clone();
                    tokio::spawn(async move {
                        let refresh_interval = if base_feel_trigger.is_some() {
                            BASE_FEEL_OUTPUT_REFRESH_INTERVAL
                        } else {
                            MANUAL_OUTPUT_REFRESH_INTERVAL
                        };
                        loop {
                            let now = Instant::now();
                            if now >= deadline {
                                break;
                            }
                            let sleep_for =
                                refresh_interval.min(deadline.saturating_duration_since(now));
                            tokio::time::sleep(sleep_for).await;
                            if !state_for_reset
                                .manual_output_override_active_for(&id_for_reset, generation)
                            {
                                if !state_for_reset.manual_output_override_generation_matches(
                                    &id_for_reset,
                                    generation,
                                ) {
                                    return;
                                }
                                break;
                            }
                            if Instant::now() >= deadline {
                                break;
                            }
                            let output_for_refresh = if let Some(trigger_config) =
                                base_feel_trigger.as_ref()
                            {
                                match state_for_reset
                                    .read_input_state_for_controller(&id_for_reset)
                                    .await
                                {
                                    Ok(Some(input)) => base_feel_test_output_frame(
                                        trigger_config.clone(),
                                        Some(input.l2),
                                        Some(input.r2),
                                    ),
                                    Ok(None) => base_feel_test_output_frame(
                                        trigger_config.clone(),
                                        None,
                                        None,
                                    ),
                                    Err(error) => {
                                        state_for_reset
                                                .note_hardware_output_error(format!(
                                                    "Hardware effect test input read for controller {id_for_reset} failed: {error}"
                                                ))
                                                .await;
                                        output_for_refresh.clone()
                                    }
                                }
                            } else {
                                output_for_refresh.clone()
                            };
                            if let Err(error) = state_for_reset
                                .write_output_frame_with_owner(
                                    &id_for_reset,
                                    &output_for_refresh,
                                    OutputWriteOwner::Manual(generation),
                                )
                                .await
                            {
                                state_for_reset
                                    .note_hardware_output_error(format!(
                                        "Hardware effect test refresh for controller {id_for_reset} failed: {error}"
                                    ))
                                    .await;
                                break;
                            }
                        }

                        if state_for_reset
                            .manual_output_override_generation_matches(&id_for_reset, generation)
                        {
                            let _ = state_for_reset
                                .write_output_frame_with_owner(
                                    &id_for_reset,
                                    &ControllerOutputFrame::default(),
                                    OutputWriteOwner::FinishManual(generation),
                                )
                                .await;
                        }
                    });
                    format!(
                        "Queued hardware effect test for controller {id} ({} byte {:?} report)",
                        write.bytes, write.report_kind
                    )
                } else {
                    format!(
                        "Stopped hardware effect test for controller {id} ({} byte {:?} report)",
                        write.bytes, write.report_kind
                    )
                }
            }
            Err(error) => {
                let _ = state
                    .write_output_frame_with_owner(
                        &id,
                        &ControllerOutputFrame::default(),
                        OutputWriteOwner::FinishManual(generation),
                    )
                    .await;
                accepted = false;
                status = StatusCode::CONFLICT;
                format!("Hardware effect test for controller {id} was blocked: {error}")
            }
        }
    } else {
        format!("Queued effect test preview for controller {id}")
    };

    {
        let mut inner = state.inner.write().await;
        inner.push_log(LogEntry {
            level: if accepted { "info" } else { "warn" }.to_string(),
            message: format!("{}: target={} mode={}", message, target, mode),
            timestamp: current_timestamp(),
        });
    }

    if !accepted && message.is_empty() {
        message = format!("Hardware effect test for controller {id} was blocked");
    }

    Ok((
        status,
        Json(EffectTestResponse {
            accepted,
            message,
            dry_run: !hardware_output_enabled,
            duration_ms,
            output,
        }),
    ))
}
