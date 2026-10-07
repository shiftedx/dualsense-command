import assert from 'node:assert/strict';
import { createServer } from 'vite';
import { chromium } from 'playwright';
import { fileURLToPath } from 'node:url';
import { readFileSync } from 'node:fs';
const stockProfiles = JSON.parse(readFileSync(new URL('./fixtures/stock-profiles.json', import.meta.url), 'utf8'));

// Test-only boundary: the compiled, mounted App and its real workflows run with
// deferred API responses. No test hooks or alternate behavior ship in the app.
const fixtureApi = `
export * from '/src/lib/api.ts';
import { mockAppSnapshot, mockControllerConfig } from '/src/lib/mock/fixture.ts';
const clone = value => structuredClone(value);
const snapshot = clone(mockAppSnapshot);
snapshot.controllers.push({...snapshot.controllers[0], id: 'edge-b', name: 'Edge B'});
snapshot.controllers.push({...snapshot.controllers[0], id: 'standard', name: 'Standard', family:'DualSense'});
const stockProfiles = ${JSON.stringify(stockProfiles)};
for (const stock of stockProfiles) if (!snapshot.profiles.some(p=>p.id===stock.id)) snapshot.profiles.push({id:stock.id,name:stock.name,builtIn:true,scope:'Built-in',gameId:stock.game_id??'all',active:false,rules:0,updatedAt:''});
const profile = id => ({id, name:id, builtIn:false, scope:'Global', gameId:'all', active:false, rules:0, updatedAt:''});
snapshot.profiles.push(profile('profile-a'), profile('profile-b'));
const fixture = window.fixture = { snapshot, pending:[], requests:[], deferExport:false, deferEdge:false, deferController:false, deferSave:false, deferCreate:false, failLive:false, stockProfiles };
const defer = (kind,id) => new Promise((resolve,reject) => fixture.pending.push({kind,id,resolve,reject}));
fixture.resolve = (kind,id,value) => { const i=fixture.pending.findIndex(p=>p.kind===kind&&p.id===id); if(i<0)throw Error('missing '+kind+' '+id); fixture.pending.splice(i,1)[0].resolve(value); };
fixture.resolveLast = (kind,id,value) => { const i=fixture.pending.findLastIndex(p=>p.kind===kind&&p.id===id); if(i<0)throw Error('missing '+kind+' '+id); fixture.pending.splice(i,1)[0].resolve(value); };
fixture.reject = (kind,id) => { const i=fixture.pending.findIndex(p=>p.kind===kind&&p.id===id); if(i<0)throw Error('missing '+kind+' '+id); fixture.pending.splice(i,1)[0].reject(Error('obsolete failure')); };
fixture.config = brightness => ({...clone(mockControllerConfig), lightbar:{...mockControllerConfig.lightbar,brightness}});
fixture.exported = (id,brightness) => ({schema:'dev.dscc.profile.v1',id,name:id,config:fixture.config(brightness)});
export const getAppSnapshot = async () => clone(snapshot);
export const connectAppSnapshotSocket = () => () => {};
export const getAppUpdateCheck = async () => ({state:'up_to_date',message:'Fixture'});
export const getControllerConfig = async id => fixture.deferController ? defer('controller',id) : ({...clone(mockControllerConfig),controllerId:id,model:id==='standard'?'DualSense':'DualSense Edge'});
export const getControllerInput = async () => ({available:false,source:'fixture',message:'No hardware input',buttons:[],l2:0,r2:0});
export const exportProfile = async id => { fixture.requests.push({kind:'export',id}); return fixture.deferExport ? defer('export',id) : clone(stockProfiles.find(p=>p.id===id) ?? fixture.exported(id, id==='profile-b'?42:31)); };
export const getEdgeProfiles = async id => fixture.deferEdge ? defer('edge',id) : ({controllerId:id,slots:[],supportState:'unavailable'});
export const setProfileOverride = async () => null;
export const saveControllerConfig = async (id,config) => { fixture.requests.push({kind:'live',config:clone(config)}); if(fixture.failLive) throw Error('live PUT failed'); return {...clone(config),controllerId:id,model:id==='standard'?'DualSense':'DualSense Edge'}; };
export const saveProfileConfig = async (id,config) => { fixture.requests.push({kind:'save',id,config:clone(config)}); if(fixture.deferSave) await defer('save',id); return {accepted:true,message:'saved'}; };
export const runEffectTest = async (request,id) => { fixture.requests.push({kind:'effect',id,request:clone(request)}); return {accepted:true,dryRun:true}; };
export const createProfile = async (name,options) => {if(fixture.deferCreate)await defer('create','copy');const p=profile('copy-'+snapshot.profiles.length);p.name=name;snapshot.profiles.push(p);return p;};
`;
const hook = `
  dismissOnboarding();
  if (import.meta.env.DEV) window.appFixture = {
    select: selectProfileForScope, readEdge: loadEdgeProfiles,
    reloadController: loadControllerConfig,
    controller: (id) => { selectedControllerId = id; },
    scope: (value) => { selectedTuningScope = value; },
    edit: setLightbarBrightness, save: saveActiveProfile, discard: discardDraftChanges,
    stop: stopAppRuntime, manual: toggleBaseFeelTest, reset: restoreDefaults,
    resetCurves: resetTriggerCurvesToProfileDefaults,
    copy: () => {beginSaveAsProfile();return submitSaveAsProfile();},
    state: () => ({config:buildControllerConfig(), baseline:profileSaveBaselineConfig, dirty:profileConfigDirty, edge:edgeProfiles, edgeLoading:edgeProfilesLoading, edgeError:edgeProfilesError, profileError:profileOverrideMessage, selected:selectedOverrideProfileId})
  };
`;
const server = await createServer({ root: fileURLToPath(new URL('..', import.meta.url)), logLevel:'error', server:{host:'127.0.0.1',port:0}, plugins:[{
  name:'profile-regression-boundaries', enforce:'pre',
  resolveId(source, importer) { if (source.endsWith('/lib/api') && importer?.replaceAll('\\','/').includes('/src/')) return '\0profile-fixture-api'; },
  load(id) { if(id==='\0profile-fixture-api') return fixtureApi; },
  transform(code,id) { if(id.replaceAll('\\','/').endsWith('/src/App.svelte')) return code.replace('</script>',hook+'</script>'); }
}] });
const failures=[];
await server.listen();
const browser = await chromium.launch({headless:true});
async function test(name, run) {
  const page=await browser.newPage({viewport:{width:1440,height:1000}});
  const unhandled=[];
  await page.route(/^http:\/\/127\.0\.0\.1:\d+\/api\//, route => {unhandled.push(route.request().method()+' '+new URL(route.request().url()).pathname);return route.abort('blockedbyclient');});
  try { await page.goto(server.resolvedUrls.local[0]+'#/tuning'); await page.waitForFunction(()=>window.appFixture && window.fixture); await page.waitForTimeout(200); await run(page); assert.deepEqual(unhandled,[], 'Every API call must use the controlled boundary');console.log('PASS '+name); }
  catch(error) { failures.push(name+': '+error.message); console.error('FAIL '+name+': '+error.message); }
  finally { await page.close(); }
}
try {
  await test('profile late success cannot replace selected editor', async page => {
    await page.evaluate(()=>{fixture.deferExport=true;void appFixture.select('profile-a');});
    await page.waitForFunction(()=>fixture.pending.length===1);
    await page.evaluate(()=>{void appFixture.select('profile-b');});
    await page.waitForFunction(()=>fixture.pending.length===2);
    await page.evaluate(()=>fixture.resolve('export','profile-b',fixture.exported('profile-b',42)));
    await page.waitForFunction(()=>appFixture.state().config.lightbar.brightness===42);
    await page.evaluate(()=>fixture.resolve('export','profile-a',fixture.exported('profile-a',31)));
    await page.waitForTimeout(100);
    assert.equal(await page.evaluate(()=>appFixture.state().config.lightbar.brightness),42);
  });
  await test('profile late errors and scope changes are ignored', async page => {
    await page.evaluate(()=>{fixture.deferExport=true;void appFixture.select('profile-a');});
    await page.waitForFunction(()=>fixture.pending.length===1);
    await page.evaluate(()=>{appFixture.scope('none');fixture.reject('export','profile-a');});
    await page.waitForTimeout(100);
    assert.doesNotMatch(await page.evaluate(()=>appFixture.state().profileError),/obsolete failure/);
  });
  for (const change of ['controller','scope']) await test('profile success after '+change+' change is ignored', async page => {
    await page.evaluate(()=>{fixture.deferExport=true;void appFixture.select('profile-a');});
    await page.waitForFunction(()=>fixture.pending.length===1);
    await page.evaluate(change=>{if(change==='controller') appFixture.controller('edge-b');else appFixture.scope('none');},change);
    await page.waitForTimeout(50);
    const before=await page.evaluate(()=>appFixture.state().config.lightbar.brightness);
    await page.evaluate(()=>fixture.resolve('export','profile-a',fixture.exported('profile-a',13)));
    await page.waitForTimeout(50);
    assert.equal(await page.evaluate(()=>appFixture.state().config.lightbar.brightness),before);
  });
  await test('same profile repeated request keeps newest result and ignores late failure', async page => {
    await page.evaluate(()=>{fixture.deferExport=true;void appFixture.select('profile-a');});
    await page.waitForFunction(()=>fixture.pending.length===1);
    await page.evaluate(()=>{void appFixture.select('profile-a');});
    await page.waitForFunction(()=>fixture.pending.length===2);
    await page.evaluate(()=>fixture.resolveLast('export','profile-a',fixture.exported('profile-a',42)));
    await page.evaluate(()=>fixture.reject('export','profile-a'));
    await page.waitForTimeout(50);
    assert.equal(await page.evaluate(()=>appFixture.state().config.lightbar.brightness),42);
    assert.doesNotMatch(await page.evaluate(()=>appFixture.state().profileError),/obsolete failure/);
  });
  await test('controller config read is invalidated by leaving and returning to its scope', async page=>{
    await page.evaluate(()=>{fixture.deferController=true;void appFixture.reloadController(fixture.snapshot.controllers[0].id);});
    await page.waitForFunction(()=>fixture.pending.length===1);
    await page.evaluate(()=>appFixture.scope('none'));
    await page.waitForTimeout(50);
    await page.evaluate(()=>appFixture.scope('global'));
    await page.waitForTimeout(50);
    const before=await page.evaluate(()=>appFixture.state().config.lightbar.brightness);
    await page.evaluate(()=>fixture.resolve('controller',fixture.snapshot.controllers[0].id,fixture.config(13)));
    await page.waitForTimeout(50);
    assert.equal(await page.evaluate(()=>appFixture.state().config.lightbar.brightness),before);
  });
  await test('Edge controller ownership guards success error and finally', async page => {
    await page.evaluate(()=>{fixture.deferEdge=true;void appFixture.readEdge(fixture.snapshot.controllers[0].id,true);appFixture.controller('edge-b');});
    await page.waitForFunction(()=>fixture.pending.filter(p=>p.kind==='edge').length===2);
    await page.evaluate(()=>fixture.resolve('edge','edge-b',{controllerId:'edge-b',slots:[],supportState:'unavailable'}));
    await page.evaluate(()=>fixture.reject('edge',fixture.snapshot.controllers[0].id));
    await page.waitForTimeout(100);
    assert.equal(await page.evaluate(()=>appFixture.state().edge?.controllerId),'edge-b');
    assert.equal(await page.evaluate(()=>appFixture.state().edgeError),'');
  });
  await test('Edge obsolete finally cannot clear a newer read loading state', async page => {
    await page.evaluate(()=>{fixture.deferEdge=true;void appFixture.readEdge(fixture.snapshot.controllers[0].id,true);});
    await page.evaluate(()=>{void appFixture.readEdge(fixture.snapshot.controllers[0].id,true);});
    await page.waitForFunction(()=>fixture.pending.length===2);
    await page.evaluate(()=>fixture.resolve('edge',fixture.snapshot.controllers[0].id,{controllerId:'obsolete',slots:[]}));
    await page.waitForTimeout(50);
    assert.equal(await page.evaluate(()=>appFixture.state().edgeLoading),true);
    assert.notEqual(await page.evaluate(()=>appFixture.state().edge?.controllerId),'obsolete');
    await page.evaluate(()=>fixture.resolve('edge',fixture.snapshot.controllers[0].id,{controllerId:fixture.snapshot.controllers[0].id,slots:[]}));
    await page.waitForTimeout(50);
    assert.equal(await page.evaluate(()=>appFixture.state().edgeLoading),false);
  });
  await test('failed live PUT still enables Save and keyboard save; pending save keeps exact baseline', async page => {
    await page.evaluate(()=>appFixture.select('profile-a'));
    await page.evaluate(()=>{fixture.failLive=true;fixture.deferSave=true;appFixture.edit(51);});
    await page.waitForTimeout(250);
    assert.equal(await page.evaluate(()=>appFixture.state().dirty),true);
    assert.equal(await page.locator('.saved-save-button').first().isEnabled(),true);
    await page.evaluate(()=>{fixture.failLive=false;});
    await page.keyboard.press('Control+s');
    await page.waitForFunction(()=>fixture.pending.some(p=>p.kind==='save'));
    await page.evaluate(()=>{appFixture.edit(64);fixture.resolve('save','profile-a');});
    await page.waitForTimeout(100);
    assert.equal(await page.evaluate(()=>appFixture.state().baseline.lightbar.brightness),51);
    await page.evaluate(()=>appFixture.discard());
    assert.equal(await page.evaluate(()=>appFixture.state().config.lightbar.brightness),51);
  });
  await test('saved rail requests 3000ms and expires after three seconds', async page => {
    await page.locator('.saved-preview-button').first().click();
    assert.equal(await page.evaluate(()=>fixture.requests.find(r=>r.kind==='effect').request.durationMs),3000);
    await page.waitForTimeout(1600);
    await page.evaluate(()=>appFixture.edit(52));
    await page.waitForTimeout(1600);
    assert.equal(await page.locator('.saved-preview-button').first().getAttribute('aria-pressed'),'false');
  });
  await test('stock save keeps newer edits after CREATE and custom selection complete',async page=>{
    await page.evaluate(()=>appFixture.select('global'));
    await page.evaluate(()=>{appFixture.edit(51);fixture.deferCreate=true;void appFixture.save();});
    await page.waitForFunction(()=>fixture.pending.some(p=>p.kind==='create'));
    await page.evaluate(()=>{appFixture.edit(64);fixture.resolve('create','copy');});
    await page.waitForFunction(()=>appFixture.state().selected.startsWith('copy-'));
    await page.waitForTimeout(200);
    assert.equal(await page.evaluate(()=>fixture.requests.find(r=>r.kind==='save').config.lightbar.brightness),51);
    assert.equal(await page.evaluate(()=>appFixture.state().baseline.lightbar.brightness),51);
    assert.equal(await page.evaluate(()=>appFixture.state().config.lightbar.brightness),64);
    assert.equal(await page.evaluate(()=>appFixture.state().dirty),true);
    assert.equal(await page.locator('.saved-save-button').first().isEnabled(),true);
    await page.evaluate(()=>appFixture.discard());
    assert.equal(await page.evaluate(()=>appFixture.state().config.lightbar.brightness),51);
  });
  await test('manual test retains 30s while rail preview replaces it with 3s',async page=>{
    await page.evaluate(()=>appFixture.manual());
    await page.locator('.saved-preview-button').first().click();
    const effects=await page.evaluate(()=>fixture.requests.filter(r=>r.kind==='effect'));
    assert.equal(effects[0].request.durationMs,30000);
    assert.equal(effects[1].request.mode,'off');
    assert.equal(effects[2].request.durationMs,3000);
    await page.evaluate(()=>appFixture.stop());
    const count=await page.evaluate(()=>fixture.requests.filter(r=>r.kind==='effect').length);
    await page.waitForTimeout(3100);
    assert.equal(await page.evaluate(()=>fixture.requests.filter(r=>r.kind==='effect').length),count,'teardown clears preview timers');
  });
  for (const ending of ['click','teardown','controller']) await test('preview stops on '+ending+' using its owning controller',async page=>{
    await page.locator('.saved-preview-button').first().click();
    if(ending==='click') await page.locator('.saved-preview-button').first().click();
    else await page.evaluate(ending=>ending==='teardown'?appFixture.stop():appFixture.controller('edge-b'),ending);
    await page.waitForTimeout(50);
    const effects=await page.evaluate(()=>fixture.requests.filter(r=>r.kind==='effect'));
    assert.equal(effects.at(-1).request.mode,'off');
    assert.equal(effects.at(-1).id,effects[0].id);
  });
  for(const stock of stockProfiles) await test('complete canonical select/reset/copy '+stock.id,async page=>{
    await page.evaluate(()=>appFixture.controller('standard'));
    await page.waitForTimeout(50);
    await page.evaluate(id=>appFixture.select(id),stock.id);
    const project=config=>{const {profileAssignments,...saved}=config;return saved;};
    assert.deepEqual(project(await page.evaluate(()=>appFixture.state().config)),stock.config);
    await page.evaluate(()=>appFixture.edit(19));
    await page.evaluate(()=>appFixture.reset());
    assert.deepEqual(project(await page.evaluate(()=>appFixture.state().config)),stock.config);
    await page.evaluate(()=>appFixture.copy());
    assert.deepEqual(project(await page.evaluate(()=>fixture.requests.findLast(r=>r.kind==='save').config)),stock.config);
  });
} finally { await browser.close(); await server.close(); }
if(failures.length) throw Error(failures.join('\n'));
