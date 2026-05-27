use super::*;

#[test]
fn idle_forza_listener_is_a_clear_diagnostic() {
    let mut runtime = test_udp_adapter_runtime();
    runtime.mark_bound("127.0.0.1:5300".parse().unwrap());

    let health = adapter_runtime_health_check(&runtime, Some(&no_game_detection("none")));

    assert_eq!(health.name, "forza-data-out");
    assert_eq!(health.status, "ok");
    assert!(health.detail.contains("telemetry will activate"));
    assert!(!health.detail.contains("waiting"));
}

#[test]
fn detected_forza_without_packets_warns_in_diagnostics() {
    let mut runtime = test_udp_adapter_runtime();
    runtime.mark_bound("127.0.0.1:5300".parse().unwrap());
    let detection = detect_running_game_from_processes(["ForzaHorizon6.exe"]);

    let health = adapter_runtime_health_check(&runtime, Some(&detection));

    assert_eq!(health.name, "forza-data-out");
    assert_eq!(health.status, "warning");
    assert!(health.detail.contains("Forza Horizon 6 is running"));
    assert!(health.detail.contains("no live Data Out packets"));
}
