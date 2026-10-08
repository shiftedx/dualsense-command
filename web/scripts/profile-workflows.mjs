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
  const { steamBindingSlots } = await server.ssrLoadModule('/src/lib/features/buttonMapping/buttonMapping.ts');
  const flush = () => new Promise(resolve => setImmediate(resolve));
  const deferred = () => { let resolve; let reject; const promise = new Promise((yes,no) => {resolve=yes;reject=no;}); return {promise,resolve,reject}; };
  const mappingFixture = kind => {
    let state = createButtonMappingSessionState();
    const requests = [], notifications = [], refreshes = [];
    const store = { get:()=>state, set:next=>state=next, update:fn=>state=fn(state) };
    const binding = (key,inputId='button_a') => ({input:'Cross',inputId,rawBinding:`key_press ${key}, , ${key}`,binding:key,groupId:'1',source:inputId==='button_a'?'button_diamond':'switches',sourceMode:'four_buttons',activator:'Full Press',kind:'Key'});
    const contextFor = key => ({active:true,controller:{...mockAppSnapshot.controllers[0],family:'DualSense Edge'},controllerHeaderName:'Test',selectedTuningScope:'game',steamContextGame:{gameId:`game-${key}`,appId:key,name:key,inputProvider:kind==='bridge'?'dscc_input_bridge':'steam_input'},steamInputStatus:{available:true,running:true,layouts:[{source:`layout-${key}`,appId:key,controllerType:'controller_ps5_edge',bindings:[binding(key),binding(key,'button_back_left'),binding(key,'button_back_right')]}]},inputBridgeStatus:{available:true},activeProfileName:key,profileContextGameName:key,bridgeProfileId:`profile-${key}`,refresh:()=>{const pending=deferred();refreshes.push(pending);return pending.promise;},notify:message=>notifications.push(message)});
    let context = contextFor('A');
    globalThis.fetch = (url,init) => {
      const pending = deferred(), body = JSON.parse(init?.body ?? '{}');
      const response = {accepted:true,message:`saved ${body.rawBinding ?? body.target ?? body.leftKey}`,warnings:[],binding:{...binding('A'),rawBinding:body.rawBinding},paddles:[{binding:binding(body.leftKey,'button_back_left')},{binding:binding(body.rightKey,'button_back_right')}]};
      const request = {...pending,url,body,response};requests.push(request);
      return pending.promise;
    };
    const render = () => createButtonMappingSession({...context,state,store});
    const start = (view,key='X') => {
      if(kind==='paddle'){view.onPaddlePresetLeftKeyChange(key);return view.onApplyPaddlePreset();}
      view.onRawDraftChange(`key_press ${key}, , ${key}`);return view.onSaveBinding();
    };
    const settle = (request,outcome='resolve') => request.resolve(outcome==='reject'?Response.json({message:'obsolete mapping failure'},{status:500}):Response.json(request.response));
    const transition = type => {
      if(type==='switch'||type==='return'){context=contextFor('B');render();if(type==='return')context=contextFor('A');}
      else if(type==='inactive')context={...context,active:false};
      else if(type==='controller')context={...context,controller:{...context.controller,id:'controller-B'}};
      else if(type==='profile')context={...context,bridgeProfileId:'profile-B'};
      else if(type==='provider')context={...context,steamContextGame:{...context.steamContextGame,inputProvider:kind==='bridge'?'steam_input':'dscc_input_bridge'}};
      return render();
    };
    const close = async () => {for(const request of requests)settle(request);for(const pending of refreshes)pending.resolve();await flush();};
    return {get state(){return state;},store,requests,notifications,refreshes,render,start,settle,transition,close};
  };
  for(const kind of ['steam','bridge','paddle']) for(const outcome of ['resolve','reject']) for(const transition of ['switch','return','inactive','controller','provider','profile']) await test(`${kind} pending ${outcome} ignores ${transition} mapping context`,async()=>{
    const fixture=mappingFixture(kind);
    try {
      const saving=fixture.start(fixture.render());
      await flush();assert.equal(fixture.requests.length,1);
      fixture.transition(transition);
      const resetBusy=fixture.state.bindingBusy;
      // A replacement operation owns busy/finally; it must not be released by A.
      fixture.store.update(state=>({...state,bindingBusy:true,bindingMessage:'Current context',bindingDraft:'key_press B, , B',optimisticBindings:[{inputId:'button_a',rawBinding:'key_press B, , B'}]}));
      const before=structuredClone(fixture.state), notifications=fixture.notifications.length;
      fixture.settle(fixture.requests[0],outcome);await saving;await flush();
      assert.deepEqual(fixture.state,before,'old response changed the replacement editor');
      assert.equal(fixture.notifications.length,notifications,'old response notified the new context');
      assert.equal(fixture.refreshes.length,0,'obsolete success started a refresh');
      assert.equal(resetBusy,false,'context reset must release the old busy flag');
    } finally {await fixture.close();}
  });
  for(const kind of ['steam','bridge','paddle']) for(const outcome of ['resolve','reject']) await test(`${kind} older request cannot release or replace its newer mapping save (${outcome})`,async()=>{
    const fixture=mappingFixture(kind);
    try {
      const first=fixture.start(fixture.render(),'X');await flush();
      const second=fixture.start(fixture.render(),'Y');await flush();
      const before=structuredClone(fixture.state),notifications=fixture.notifications.length;
      fixture.settle(fixture.requests[0],outcome);await first;
      assert.deepEqual(fixture.state,before);assert.equal(fixture.notifications.length,notifications);assert.equal(fixture.refreshes.length,0);
      fixture.settle(fixture.requests[1]);await second;
      assert.equal(fixture.state.bindingBusy,false);assert.match(fixture.state.bindingMessage,/Y/);
    } finally {await fixture.close();}
  });
  for(const kind of ['steam','paddle']) for(const transition of ['switch','new-save']) for(const outcome of ['resolve','reject']) await test(`${kind} old ${outcome} refresh cannot clear ${transition} optimistic bindings`,async()=>{
    const fixture=mappingFixture(kind);
    try {
      const first=fixture.start(fixture.render(),'X');await flush();fixture.settle(fixture.requests[0]);await first;
      assert.equal(fixture.refreshes.length,1);
      if(transition==='switch')fixture.transition('switch');
      const second=fixture.start(fixture.render(),'Y');await flush();fixture.settle(fixture.requests[1]);await second;
      const optimistic=structuredClone(fixture.state.optimisticBindings);
      assert.ok(optimistic?.some(binding=>binding.rawBinding.includes('Y')));
      fixture.refreshes[0][outcome](Error('obsolete refresh failure'));await flush();
      assert.deepEqual(fixture.state.optimisticBindings,optimistic);
      fixture.refreshes[1][outcome](Error('current refresh failure'));await flush();assert.equal(fixture.state.optimisticBindings,null);
    } finally {await fixture.close();}
  });
  for(const kind of ['steam','bridge','paddle']) await test(`${kind} obsolete mapping callbacks cannot edit or start writes after leaving and returning`,async()=>{
    const fixture=mappingFixture(kind);
    try {
      const old=fixture.render();fixture.transition('return');const before=structuredClone(fixture.state),notifications=fixture.notifications.length;
      old.onRawDraftChange('key_press Z, , Stale');old.onLabelChange('Stale');old.onTargetChange('key_press Z, , Stale');old.onResetDraft();
      old.onPaddlePresetLeftKeyChange('Z');old.onPaddlePresetRightKeyChange('Z');old.onHoverSlot(steamBindingSlots[1]);old.onSelectSlot(steamBindingSlots[1]);
      void old.onSaveBinding();void old.onApplyPaddlePreset();await flush();
      assert.equal(fixture.requests.length,0);assert.deepEqual(fixture.state,before);assert.equal(fixture.notifications.length,notifications);
    } finally {await fixture.close();}
  });
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
