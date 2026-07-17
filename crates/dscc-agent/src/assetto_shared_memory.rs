use super::*;

#[cfg(any(target_os = "windows", test))]
pub(crate) const ASSETTO_PHYSICS_MIN_LEN: usize = 120;
#[cfg(any(target_os = "windows", test))]
pub(crate) const ASSETTO_GRAPHICS_MIN_LEN: usize = 12;
#[cfg(any(target_os = "windows", test))]
pub(crate) const ASSETTO_STATIC_MAX_RPM_OFFSET: usize = 412;
#[cfg(any(target_os = "windows", test))]
pub(crate) const ASSETTO_STATIC_MIN_LEN: usize = ASSETTO_STATIC_MAX_RPM_OFFSET + 4;
#[cfg(any(target_os = "windows", test))]
pub(crate) const ASSETTO_AC_LIVE: i32 = 2;
#[cfg(any(target_os = "windows", test))]
const ASSETTO_AC_PAUSE: i32 = 3;
#[cfg(any(target_os = "windows", test))]
const ASSETTO_DEFAULT_MAX_RPM: f64 = 8_000.0;
#[cfg(any(target_os = "windows", test))]
const STANDARD_GRAVITY_MS2: f64 = 9.80665;

#[cfg(any(target_os = "windows", test))]
#[derive(Clone, Copy)]
pub(crate) struct AssettoSharedMemoryPages<'a> {
    pub(crate) physics: &'a [u8],
    pub(crate) graphics: Option<&'a [u8]>,
    pub(crate) static_page: Option<&'a [u8]>,
}

#[cfg(any(target_os = "windows", test))]
pub(crate) fn parse_assetto_shared_memory_pages(
    pages: AssettoSharedMemoryPages<'_>,
    sequence: u64,
) -> Option<(usize, Vec<SignalUpdate>)> {
    if pages.physics.len() < ASSETTO_PHYSICS_MIN_LEN {
        return None;
    }

    let packet_id = read_le_i32(pages.physics, 0)?;
    let throttle = finite_unit(read_le_f32(pages.physics, 4)?);
    let brake = finite_unit(read_le_f32(pages.physics, 8)?);
    let raw_gear = read_le_i32(pages.physics, 16)?;
    let rpm = finite_non_negative(f64::from(read_le_i32(pages.physics, 20)?));
    let steer_angle = finite_f64(f64::from(read_le_f32(pages.physics, 24)?));
    let speed_kmh = finite_non_negative(read_le_f32_f64(pages.physics, 28)?);
    let acceleration_x = finite_f64(read_le_f32_f64(pages.physics, 44)? * STANDARD_GRAVITY_MS2);
    let acceleration_y = finite_f64(read_le_f32_f64(pages.physics, 48)? * STANDARD_GRAVITY_MS2);
    let acceleration_z = finite_f64(read_le_f32_f64(pages.physics, 52)? * STANDARD_GRAVITY_MS2);
    let acceleration_magnitude = finite_f64(
        acceleration_x
            .mul_add(
                acceleration_x,
                acceleration_y.mul_add(acceleration_y, acceleration_z * acceleration_z),
            )
            .sqrt(),
    );
    let wheel_slip = read_f32_array_abs(pages.physics, 56, 4)?;
    let front_slip = wheel_slip[0].max(wheel_slip[1]);
    let rear_slip = wheel_slip[2].max(wheel_slip[3]);
    let wheel_slip_max = front_slip.max(rear_slip);
    let suspension_signal = signal_scaled_value(acceleration_magnitude, 2.0, 16.0);
    let surface_grip = read_le_f32(pages.physics, 116)
        .map(finite_unit)
        .filter(|value| *value > 0.0);
    let loose_surface = surface_grip.map_or(0.0, |grip| (1.0 - grip).clamp(0.0, 1.0));
    let surface_rumble = loose_surface
        .max(signal_scaled_value(acceleration_magnitude, 3.0, 22.0) * 0.55)
        .max((wheel_slip_max - 0.12).clamp(0.0, 1.0) * 0.35)
        .clamp(0.0, 1.0);
    let max_rpm = pages
        .static_page
        .and_then(|static_page| read_le_i32(static_page, ASSETTO_STATIC_MAX_RPM_OFFSET))
        .map(f64::from)
        .filter(|value| value.is_finite() && *value >= 1_000.0)
        .unwrap_or(ASSETTO_DEFAULT_MAX_RPM);
    let rpm_ratio = if max_rpm > 0.0 {
        (rpm / max_rpm).clamp(0.0, 1.25)
    } else {
        0.0
    };
    let graphics_status = pages.graphics.and_then(|graphics| read_le_i32(graphics, 4));
    let game_state =
        assetto_game_state(graphics_status, speed_kmh, rpm, throttle, brake, packet_id);

    let updates = vec![
        sequenced_signal_update("source.id", ASSETTO_SHARED_MEMORY_ADAPTER_ID, sequence),
        sequenced_signal_update("source.connected", true, sequence),
        sequenced_signal_update("source.packet_size", pages.physics.len() as f64, sequence),
        sequenced_signal_update("game.state", game_state, sequence),
        sequenced_signal_update("vehicle.max_rpm", max_rpm, sequence),
        sequenced_signal_update("vehicle.rpm", rpm, sequence),
        sequenced_signal_update("vehicle.rpm_ratio", rpm_ratio, sequence),
        sequenced_signal_update("vehicle.speed_kmh", speed_kmh, sequence),
        sequenced_signal_update("vehicle.acceleration.x", acceleration_x, sequence),
        sequenced_signal_update("vehicle.acceleration.y", acceleration_y, sequence),
        sequenced_signal_update("vehicle.acceleration.z", acceleration_z, sequence),
        sequenced_signal_update(
            "vehicle.acceleration.magnitude",
            acceleration_magnitude,
            sequence,
        ),
        sequenced_signal_update("input.throttle", throttle, sequence),
        sequenced_signal_update("input.brake", brake, sequence),
        sequenced_signal_update("input.clutch", 0.0, sequence),
        sequenced_signal_update("input.handbrake", 0.0, sequence),
        sequenced_signal_update("input.steer", assetto_steer_unit(steer_angle), sequence),
        sequenced_signal_update("drivetrain.gear", assetto_display_gear(raw_gear), sequence),
        sequenced_signal_update("wheel.slip.front_left", wheel_slip[0], sequence),
        sequenced_signal_update("wheel.slip.front_right", wheel_slip[1], sequence),
        sequenced_signal_update("wheel.slip.rear_left", wheel_slip[2], sequence),
        sequenced_signal_update("wheel.slip.rear_right", wheel_slip[3], sequence),
        sequenced_signal_update("wheel.slip.front_max", front_slip, sequence),
        sequenced_signal_update("wheel.slip.rear_max", rear_slip, sequence),
        sequenced_signal_update("wheel.slip.max", wheel_slip_max, sequence),
        sequenced_signal_update("tire.slip_ratio.max", wheel_slip_max, sequence),
        sequenced_signal_update("tire.slip_angle.max", wheel_slip_max * 0.65, sequence),
        sequenced_signal_update("surface.rumble.max", surface_rumble, sequence),
        sequenced_signal_update("surface.rumble_strip.max", surface_rumble * 0.35, sequence),
        sequenced_signal_update("surface.puddle.max", 0.0, sequence),
        sequenced_signal_update("suspension.travel.max", suspension_signal, sequence),
    ];

    Some((pages.physics.len(), updates))
}

#[cfg(any(target_os = "windows", test))]
fn assetto_game_state(
    graphics_status: Option<i32>,
    speed_kmh: f64,
    rpm: f64,
    throttle: f64,
    brake: f64,
    packet_id: i32,
) -> &'static str {
    match graphics_status {
        Some(ASSETTO_AC_LIVE) => "driving",
        Some(ASSETTO_AC_PAUSE) => "paused",
        Some(_) => "menu",
        None if speed_kmh > 1.0 || rpm > 500.0 || throttle > 0.01 || brake > 0.01 => "driving",
        None if packet_id > 0 => "menu",
        None => "menu",
    }
}

#[cfg(any(target_os = "windows", test))]
fn assetto_display_gear(raw_gear: i32) -> f64 {
    f64::from(raw_gear.saturating_sub(1).max(0))
}

#[cfg(any(target_os = "windows", test))]
fn assetto_steer_unit(steer_angle: f64) -> f64 {
    (steer_angle / 0.75).clamp(-1.0, 1.0)
}

#[cfg(any(target_os = "windows", test))]
fn read_f32_array_abs(packet: &[u8], offset: usize, count: usize) -> Option<Vec<f64>> {
    (0..count)
        .map(|index| read_le_f32_f64(packet, offset + index * 4).map(|value| value.abs()))
        .collect()
}

#[cfg(any(target_os = "windows", test))]
fn signal_scaled_value(value: f64, input_min: f64, input_max: f64) -> f64 {
    if input_min >= input_max {
        return 0.0;
    }
    ((value - input_min) / (input_max - input_min)).clamp(0.0, 1.0)
}

#[cfg(any(target_os = "windows", test))]
fn finite_unit(value: f32) -> f64 {
    finite_f64(f64::from(value)).clamp(0.0, 1.0)
}

#[cfg(any(target_os = "windows", test))]
fn finite_non_negative(value: f64) -> f64 {
    finite_f64(value).max(0.0)
}

#[cfg(any(target_os = "windows", test))]
fn finite_f64(value: f64) -> f64 {
    if value.is_finite() {
        value
    } else {
        0.0
    }
}

#[cfg(any(target_os = "windows", test))]
fn read_le_bytes<const N: usize>(packet: &[u8], offset: usize) -> Option<[u8; N]> {
    packet.get(offset..offset + N)?.try_into().ok()
}

#[cfg(any(target_os = "windows", test))]
fn read_le_i32(packet: &[u8], offset: usize) -> Option<i32> {
    Some(i32::from_le_bytes(read_le_bytes(packet, offset)?))
}

#[cfg(any(target_os = "windows", test))]
fn read_le_f32(packet: &[u8], offset: usize) -> Option<f32> {
    Some(f32::from_le_bytes(read_le_bytes(packet, offset)?))
}

#[cfg(any(target_os = "windows", test))]
fn read_le_f32_f64(packet: &[u8], offset: usize) -> Option<f64> {
    Some(finite_f64(f64::from(read_le_f32(packet, offset)?)))
}

#[cfg(any(target_os = "windows", test))]
fn sequenced_signal_update(
    name: &str,
    value: impl Into<SignalValue>,
    sequence: u64,
) -> SignalUpdate {
    signal_update(name, value).with_sequence(sequence)
}

#[cfg(target_os = "windows")]
type AssettoSharedMemoryPageBuffers = (Vec<u8>, Option<Vec<u8>>, Option<Vec<u8>>);

#[cfg(target_os = "windows")]
fn read_assetto_shared_memory_snapshot(
    sequence: u64,
) -> io::Result<Option<(usize, Vec<SignalUpdate>)>> {
    let Some((physics, graphics, static_page)) = read_assetto_shared_memory_pages()? else {
        return Ok(None);
    };
    Ok(parse_assetto_shared_memory_pages(
        AssettoSharedMemoryPages {
            physics: &physics,
            graphics: graphics.as_deref(),
            static_page: static_page.as_deref(),
        },
        sequence,
    ))
}

#[cfg(target_os = "windows")]
fn read_assetto_shared_memory_pages() -> io::Result<Option<AssettoSharedMemoryPageBuffers>> {
    let page_sets = [
        (
            "Local\\acpmf_physics",
            "Local\\acpmf_graphics",
            "Local\\acpmf_static",
        ),
        (
            "Local\\acevo_pmf_physics",
            "Local\\acevo_pmf_graphics",
            "Local\\acevo_pmf_static",
        ),
    ];

    for (physics_name, graphics_name, static_name) in page_sets {
        let Some(physics) = read_windows_shared_memory_page(physics_name, ASSETTO_PHYSICS_MIN_LEN)?
        else {
            continue;
        };
        let graphics = read_windows_shared_memory_page(graphics_name, ASSETTO_GRAPHICS_MIN_LEN)?;
        let static_page = read_windows_shared_memory_page(static_name, ASSETTO_STATIC_MIN_LEN)?;
        return Ok(Some((physics, graphics, static_page)));
    }

    Ok(None)
}

#[cfg(target_os = "windows")]
fn read_windows_shared_memory_page(
    name: &str,
    bytes_to_read: usize,
) -> io::Result<Option<Vec<u8>>> {
    use windows_sys::Win32::{
        Foundation::{CloseHandle, GetLastError, ERROR_FILE_NOT_FOUND, HANDLE},
        System::Memory::{
            MapViewOfFile, OpenFileMappingW, UnmapViewOfFile, FILE_MAP_READ,
            MEMORY_MAPPED_VIEW_ADDRESS,
        },
    };

    struct MappingHandle(HANDLE);

    impl Drop for MappingHandle {
        fn drop(&mut self) {
            unsafe {
                let _ = CloseHandle(self.0);
            }
        }
    }

    struct MappingView(MEMORY_MAPPED_VIEW_ADDRESS);

    impl Drop for MappingView {
        fn drop(&mut self) {
            unsafe {
                let _ = UnmapViewOfFile(self.0);
            }
        }
    }

    let mut wide = name.encode_utf16().collect::<Vec<_>>();
    wide.push(0);

    let handle = unsafe { OpenFileMappingW(FILE_MAP_READ, 0, wide.as_ptr()) };
    if handle.is_null() {
        let error = unsafe { GetLastError() };
        if error == ERROR_FILE_NOT_FOUND {
            return Ok(None);
        }
        return Err(io::Error::from_raw_os_error(error as i32));
    }
    let handle = MappingHandle(handle);

    let view = unsafe { MapViewOfFile(handle.0, FILE_MAP_READ, 0, 0, bytes_to_read) };
    if view.Value.is_null() {
        let error = unsafe { GetLastError() };
        return Err(io::Error::from_raw_os_error(error as i32));
    }
    let view = MappingView(view);

    let bytes = unsafe { std::slice::from_raw_parts(view.0.Value.cast::<u8>(), bytes_to_read) };
    let mut owned = vec![0_u8; bytes_to_read];
    owned.copy_from_slice(bytes);
    Ok(Some(owned))
}

#[cfg(target_os = "windows")]
pub(crate) async fn assetto_shared_memory_adapter_loop(state: AgentState) {
    {
        let mut inner = state.inner.write().await;
        inner
            .adapter_runtime_mut(ASSETTO_SHARED_MEMORY_ADAPTER_ID)
            .mark_ready();
        inner.push_log(LogEntry {
            level: "info".to_string(),
            message: "Assetto shared-memory reader ready".to_string(),
            timestamp: current_timestamp(),
        });
    }

    let mut interval = tokio::time::interval(SHARED_MEMORY_TELEMETRY_PROCESS_INTERVAL);
    interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
    let mut sequence = 0_u64;
    loop {
        interval.tick().await;
        sequence = sequence.saturating_add(1);
        let result =
            tokio::task::spawn_blocking(move || read_assetto_shared_memory_snapshot(sequence))
                .await;
        match result {
            Ok(Ok(Some((packet_len, updates)))) => {
                state
                    .apply_adapter_packet(
                        ASSETTO_SHARED_MEMORY_ADAPTER_ID,
                        packet_len,
                        sequence,
                        updates,
                    )
                    .await;
            }
            Ok(Ok(None)) => {}
            Ok(Err(error)) => {
                let mut inner = state.inner.write().await;
                inner
                    .adapter_runtime_mut(ASSETTO_SHARED_MEMORY_ADAPTER_ID)
                    .last_error = Some(error.to_string());
            }
            Err(error) => {
                let mut inner = state.inner.write().await;
                inner
                    .adapter_runtime_mut(ASSETTO_SHARED_MEMORY_ADAPTER_ID)
                    .last_error = Some(error.to_string());
            }
        }
    }
}

#[cfg(not(target_os = "windows"))]
pub(crate) async fn mark_assetto_shared_memory_unavailable(state: &AgentState) {
    let mut inner = state.inner.write().await;
    inner
        .adapter_runtime_mut(ASSETTO_SHARED_MEMORY_ADAPTER_ID)
        .mark_bind_error(
            SocketAddr::from(([127, 0, 0, 1], 0)),
            "Assetto shared-memory telemetry is currently available on Windows only.",
        );
}
