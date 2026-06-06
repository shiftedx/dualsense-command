import assert from 'node:assert/strict';

import { classifySnapshotFrame, mapSnapshotDto } from '../src/lib/api/snapshotMapping.ts';

// A minimal-but-complete agent snapshot DTO. Complete enough that
// isCompleteSnapshotPayload accepts it, so it doubles as the `type: snapshot`
// socket-frame fixture below.
const baseDto = () => ({
  status: {
    version: '1.2.3',
    healthy: true,
    uptime_seconds: 3661,
    active_profile_id: 'global',
    active_adapter_id: null
  },
  appSettings: { effectiveBindAddress: '127.0.0.1:43473' },
  controllers: [
    {
      id: 'ctrl-1',
      name: 'Pad',
      model: 'DualSense Edge',
      transport: 'usb',
      connected: true,
      battery_percent: 80,
      battery_state: 'discharging',
      permission: 'granted'
    }
  ],
  profiles: [{ id: 'global', name: 'Global', built_in: true, active: true }],
  adapters: [{ id: 'forza', name: 'Forza', enabled: true, state: 'connected', packet_rate_hz: 60, protocol: 'udp' }],
  modules: [],
  steamInput: { running: false, available: false, steamPath: null, layouts: [], warnings: [] },
  gameDetection: { activeGameId: null, source: 'unknown', confidence: 0, candidates: [] },
  profileResolution: {
    controllerId: 'ctrl-1',
    selectedProfileId: 'global',
    detectedGameId: null,
    reason: 'global_default',
    validation: 'valid'
  },
  effectState: { reason: 'ok' },
  telemetry: [{ name: 'speed', value: 100, unit: 'kph', updated_ms_ago: 10 }],
  logs: Array.from({ length: 12 }, (_, i) => ({
    level: 'info',
    message: `log ${i}`,
    timestamp: '2020-01-01T00:00:00Z'
  })),
  diagnostics: { checks: [{ name: 'hid', status: 'ok', detail: 'fine' }] },
  partialErrors: []
});

// mapSnapshotDto: agent DTO -> UI snapshot.
const snap = mapSnapshotDto(baseDto());
assert.equal(snap.status.version, '1.2.3');
assert.equal(snap.status.uptime, '1h 01m');
assert.equal(snap.controllers[0].family, 'DualSense Edge');
assert.equal(snap.controllers[0].transport, 'USB');
assert.equal(snap.profiles[0].scope, 'Global');
assert.equal(snap.logs.length, 8); // capped at 8
assert.ok(Array.isArray(snap.partialErrors));
assert.equal(snap.controllerProfileAssignments.length, 1);
assert.equal(snap.controllerProfileAssignments[0].profileId, 'global');

// Dual snake_case / camelCase leaf fields normalize to one shape.
const snakeDto = baseDto();
snakeDto.controllers[0].power_diagnostics = { written_reports: 5 };
assert.equal(mapSnapshotDto(snakeDto).controllers[0].powerDiagnostics.writtenReports, 5);

const camelDto = baseDto();
camelDto.controllers[0].powerDiagnostics = { writtenReports: 7 };
assert.equal(mapSnapshotDto(camelDto).controllers[0].powerDiagnostics.writtenReports, 7);

// classifySnapshotFrame: every routing outcome, including the previously-silent
// malformed/invalidate split.
assert.equal(classifySnapshotFrame(42).kind, 'malformed'); // not a string
assert.equal(classifySnapshotFrame('{bad json').kind, 'malformed'); // unparseable
assert.equal(classifySnapshotFrame('"a string"').kind, 'malformed'); // parses to a non-object
assert.equal(classifySnapshotFrame(JSON.stringify({ type: 'ping' })).kind, 'ignore');
assert.equal(classifySnapshotFrame(JSON.stringify({ type: 'pong' })).kind, 'ignore');
assert.equal(classifySnapshotFrame(JSON.stringify({ type: 'controllers_changed' })).kind, 'invalidate');
assert.equal(
  classifySnapshotFrame(JSON.stringify({ type: 'snapshot', snapshot: { incomplete: true } })).kind,
  'invalidate'
);

const full = classifySnapshotFrame(JSON.stringify({ type: 'snapshot', snapshot: baseDto() }));
assert.equal(full.kind, 'snapshot');
assert.equal(full.snapshot.status.version, '1.2.3');
assert.equal(full.snapshot.controllers[0].family, 'DualSense Edge');

console.log('snapshot mapping: mapSnapshotDto + classifySnapshotFrame fixtures pass');
