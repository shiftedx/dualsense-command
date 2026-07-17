use super::*;

pub(crate) fn udp_adapter_bind_addr(adapter: &UdpTelemetryAdapter) -> SocketAddr {
    match adapter.id {
        FORZA_DATA_OUT_ADAPTER_ID => resolve_forza_bind_addr(),
        _ => SocketAddr::from(([127, 0, 0, 1], adapter.default_port)),
    }
}

pub(crate) async fn udp_telemetry_adapter_loop(
    state: AgentState,
    adapter: UdpTelemetryAdapter,
    bind_addr: SocketAddr,
) {
    let socket = match UdpSocket::bind(bind_addr).await {
        Ok(socket) => socket,
        Err(error) => {
            let mut inner = state.inner.write().await;
            inner
                .adapter_runtime_mut(adapter.id)
                .mark_bind_error(bind_addr, error.to_string());
            inner.push_log(LogEntry {
                level: "warn".to_string(),
                message: format!(
                    "{} listener could not bind {bind_addr}: {error}",
                    adapter.display_name
                ),
                timestamp: current_timestamp(),
            });
            return;
        }
    };

    {
        let mut inner = state.inner.write().await;
        inner.adapter_runtime_mut(adapter.id).mark_bound(bind_addr);
        inner.push_log(LogEntry {
            level: "info".to_string(),
            message: format!("{} listener ready on {bind_addr}", adapter.display_name),
            timestamp: current_timestamp(),
        });
    }

    let mut sequence = 0_u64;
    let mut buffer = [0_u8; 512];
    let mut last_processed_at: Option<Instant> = None;
    loop {
        match socket.recv_from(&mut buffer).await {
            Ok((len, _source)) => {
                sequence = sequence.saturating_add(1);
                let now = Instant::now();
                if last_processed_at
                    .is_some_and(|last| now.duration_since(last) < UDP_TELEMETRY_PROCESS_INTERVAL)
                {
                    continue;
                }
                last_processed_at = Some(now);
                if let Some(parsed) =
                    parse_udp_telemetry_packet(adapter.id, &buffer[..len], sequence)
                {
                    state
                        .apply_adapter_packet(
                            parsed.adapter_id,
                            parsed.packet_len,
                            sequence,
                            parsed.updates,
                        )
                        .await;
                } else {
                    let mut inner = state.inner.write().await;
                    inner
                        .adapter_runtime_mut(adapter.id)
                        .mark_parse_error(len, sequence);
                }
            }
            Err(error) => {
                let mut inner = state.inner.write().await;
                inner.adapter_runtime_mut(adapter.id).last_error = Some(error.to_string());
                inner.push_log(LogEntry {
                    level: "warn".to_string(),
                    message: format!("{} listener read failed: {error}", adapter.display_name),
                    timestamp: current_timestamp(),
                });
            }
        }
    }
}
