use super::*;

pub(crate) async fn device_scan_loop<T>(
    state: AgentState,
    mut manager: DeviceManager<T>,
    scan_interval: Duration,
) where
    T: DeviceTransport,
{
    let mut interval = tokio::time::interval(scan_interval);
    interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
    loop {
        interval.tick().await;
        let scan = tokio::task::spawn_blocking(move || {
            let events = controller_events_from_device_manager(&mut manager);
            (manager, events)
        })
        .await;
        let events = match scan {
            Ok((returned_manager, events)) => {
                manager = returned_manager;
                events
            }
            Err(error) => {
                state
                    .log_warn(format!("HID scan task failed: {error}"))
                    .await;
                return;
            }
        };
        match events {
            Ok(events) => {
                for event in events {
                    state.apply_controller_event(event).await;
                }
            }
            Err(error) => {
                state
                    .apply_controller_event(ControllerDiscoveryEvent::Faulted {
                        id: None,
                        message: format!("HID scan failed: {error}"),
                    })
                    .await;
            }
        }
    }
}
