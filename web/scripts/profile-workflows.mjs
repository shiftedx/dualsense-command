import assert from 'node:assert/strict';
import { createServer } from 'vite';
import { fileURLToPath } from 'node:url';
import { mkdtempSync, rmSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { dirname, join } from 'node:path';

const cacheDir = mkdtempSync(join(tmpdir(), 'dscc-profile-workflows-'));
const server = await createServer({ cacheDir, root: fileURLToPath(new URL('..', import.meta.url)), appType: 'custom', logLevel: 'silent', server: { middlewareMode: true } });
const failures = [];
async function test(name, run) { try { await run(); console.log(`PASS ${name}`); } catch (error) { failures.push(name); console.error(`FAIL ${name}: ${error.message.slice(0,700)}`); } }
const originalFetch = globalThis.fetch;
try {
  const { createButtonMappingSession, createButtonMappingSessionState } = await server.ssrLoadModule('/src/app/buttonMappingSession.ts');
  const { mockAppSnapshot, mockControllerConfig } = await server.ssrLoadModule('/src/lib/mock/fixture.ts');
  await test('mapping session drops edits when game changes with identical slot identity', () => {
    let state = createButtonMappingSessionState();
    const store = { get: () => state, set: value => state = value, update: fn => state = fn(state) };
    const game = mockAppSnapshot.gameDetection.supportedGames?.[0] ?? { gameId: 'a', appId: 'a' };
    const binding = { input: 'Cross', inputId: 'button_a', rawBinding: 'key_press A, , A', binding: 'A', groupId: '1', source: 'button_diamond', sourceMode: 'four_buttons', activator: 'Full Press', kind: 'Key' };
    const context = { active: true, controller: mockAppSnapshot.controllers[0], controllerHeaderName: 'Test', selectedTuningScope: 'game', steamContextGame: { ...game, gameId: 'a', appId: 'a' }, steamInputStatus: { available: true, running: true, layouts: [{ source: 'layout-a', appId: 'a', controllerType: 'controller_ps5_edge', bindings: [binding] }] }, refresh: async () => {}, notify: () => {} };
    createButtonMappingSession({ ...context, state, store });
    state.bindingDraft = 'key_press X, , Unsaved';
    context.steamContextGame = { ...game, gameId: 'b', appId: 'b' };
    context.steamInputStatus.layouts = [{ ...context.steamInputStatus.layouts[0], source: 'layout-b', appId: 'b', bindings: [{ ...binding, rawBinding: 'key_press B, , B' }] }];
    createButtonMappingSession({ ...context, state, store });
    assert.equal(state.bindingDraft, 'key_press B, , B');
    for (const change of [
      () => context.controller = { ...context.controller, id: 'other-controller' },
      () => context.steamInputStatus.layouts[0].title = 'Replacement layout',
      () => context.selectedTuningScope = 'global',
      () => context.active = false
    ]) {
      state.bindingDraft = 'key_press X, , Unsaved';
      change();
      createButtonMappingSession({ ...context, state, store });
      assert.notEqual(state.bindingDraft, 'key_press X, , Unsaved');
    }
  });
  const { profileImportPayload } = await server.ssrLoadModule('/src/lib/features/profiles/profileSelection.ts');
  for (const game_id of ['forza-horizon-6', undefined]) await test(`import preserves ${game_id ?? 'global'} identity`, () => {
    const exported = { schema: 'dev.dscc.profile.v1', id: 'copy', name: 'Copy', game_id, config: mockControllerConfig };
    assert.equal(profileImportPayload(exported, []).game_id, game_id);
  });
  const { createProfileManagement } = await server.ssrLoadModule('/src/app/profileManagement.ts');
  for (const mode of ['custom', 'stock', 'save-as']) for (const transition of ['switch', 'return', 'teardown']) await test(`${mode} completion cannot follow ${transition} context`, async () => {
    let state = { saveBusy: false, saveAsBusy: false, saveAsName: 'Copy' };
    let context = { controller: 'A', game: 'game-a', selected: 'profile-a', generation: 0 };
    let baseline = 'A-baseline';
    const effects = [];
    let unblock;
    const pending = new Promise(resolve => unblock = resolve);
    globalThis.fetch = async (url, init) => {
      if (url === '/api/profiles' || mode === 'custom') await pending;
      return Response.json(url === '/api/profiles' ? {id:'copy',name:'Copy',built_in:false} : {accepted:true,message:'saved'});
    };
    const saveLive = async () => effects.push(`live:${context.controller}`);
    const override = async (id, game) => {effects.push(`override:${context.controller}:${game}:${id}`);return null;};
    const workflow = createProfileManagement({
      store:{get:()=>state,set:value=>state=value}, getSnapshot:()=>null,setSnapshot:()=>{},
      getProfiles:()=>[],getSelectedActionProfile:()=>({id:context.selected,name:'Profile',builtIn:mode==='stock'}),
      getProfileContextGame:()=>null,getProfileContextGameId:()=>context.game,getSelectedTuningScope:()=> 'game',
      getControllerId:()=>context.controller,getSelectedOverrideProfileId:()=>context.selected,
      buildControllerConfig:()=>structuredClone(mockControllerConfig),profileConfigSignature:()=> 'A-config',
      setProfileSaveBaseline:value=>baseline=value,setSelectedOverrideProfileId:id=>context.selected=id,
      saveControllerConfigForProfileTargets:saveLive,setProfileOverrideForTargets:override,
      captureSaveContext:()=>{const captured={...context};return {gameId:captured.game,isCurrent:()=>captured.generation===context.generation,saveControllerConfig:async()=>effects.push(`live:${captured.controller}`),setProfileOverride:async id=>{effects.push(`override:${captured.controller}:${captured.game}:${id}`);return null;}};},
      refresh:async()=>effects.push('refresh'),notify:()=>effects.push('notify')
    });
    const saving=mode==='save-as'?workflow.submitSaveAsProfile():workflow.saveActiveProfile();
    await Promise.resolve();await Promise.resolve();
    context={controller:'B',game:'game-b',selected:'profile-b',generation:1};baseline='B-baseline';
    if(transition==='return')context={controller:'A',game:'game-a',selected:'profile-a',generation:2};
    if(transition==='teardown')context.generation=3;
    const selected=context.selected;
    unblock();await saving;
    assert.equal(baseline,'B-baseline');assert.equal(context.selected,selected);
    assert.ok(!effects.some(effect=>effect.includes(':B')||effect.startsWith('override')||effect==='notify'||effect==='refresh'),effects.join(', '));
  });
  for (const mode of ['custom', 'stock', 'save-as']) await test(`save captures acknowledged config before awaits (${mode})`, async () => {
    const builtIn = mode === 'stock';
    let state = { saveBusy: false, saveAsName: 'Copy', saveAsBusy: false };
    let draft = structuredClone(mockControllerConfig);
    const original = structuredClone(draft);
    let unblock;
    const pending = new Promise(resolve => unblock = resolve);
    const requests = [];
    globalThis.fetch = async (url, init) => {
      requests.push({ url, body: JSON.parse(init?.body ?? '{}') });
      if (url === '/api/profiles') { await pending; return Response.json({ id: 'copy', name: 'Copy', built_in: false }); }
      if (!builtIn) await pending;
      return Response.json({ accepted: true, message: 'saved' });
    };
    let baseline;
    const profile = { id: 'selected', name: 'Selected', builtIn };
    const workflow = createProfileManagement({ store: { get: () => state, set: next => state = next }, getSnapshot: () => null, setSnapshot: () => {}, getProfiles: () => [profile], getSelectedActionProfile: () => profile, getProfileContextGame: () => null, getProfileContextGameId: () => null, getSelectedTuningScope: () => 'global', captureSaveContext: () => ({ gameId: null, isCurrent: () => true, saveControllerConfig: async () => {}, setProfileOverride: async () => null }), buildControllerConfig: () => structuredClone(draft), profileConfigSignature: JSON.stringify, setProfileSaveBaseline: (signature, config) => baseline = { signature, config }, saveControllerConfigForProfileTargets: async () => {}, setProfileOverrideForTargets: async () => null, setSelectedOverrideProfileId: () => {}, refresh: async () => {}, notify: () => {} });
    const saving = mode === 'save-as' ? workflow.submitSaveAsProfile() : workflow.saveActiveProfile();
    await Promise.resolve();
    draft.lightbar.brightness = 17;
    unblock();
    await saving;
    assert.deepEqual(requests.find(request => request.url.endsWith('/config')).body, original);
    assert.deepEqual(baseline.config, original);
    assert.equal(baseline.signature, JSON.stringify(original));
  });
} finally {
  globalThis.fetch = originalFetch;
  await server.close();
  assert.equal(dirname(cacheDir), tmpdir());
  rmSync(cacheDir, {recursive:true,force:true});
}
if (failures.length) throw Error(`${failures.length} workflow regressions: ${failures.join(', ')}`);
