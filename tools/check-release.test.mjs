import assert from 'node:assert/strict';
import { test } from 'node:test';
import { parseTag, releaseNotes, checkDistribution } from './check-release.mjs';

test('release tags retain prerelease status and require a patch', () => {
  assert.deepEqual(parseTag('v0.5.0-beta.1'), { version: '0.5.0', release: '0.5.0-beta.1', prerelease: true });
  assert.equal(parseTag('v0.5.0').prerelease, false);
  for (const tag of ['v0.5', 'v0.5.0/unsafe', 'v0.5.0-']) assert.throws(() => parseTag(tag));
});

test('distribution requires complete verified shipped asset entries', () => {
  const verified = { path: 'shipped', status: 'verified', source: 'source', license: 'license' };
  assert.doesNotThrow(() => checkDistribution([verified], ['shipped']));
  for (const ledger of [[], {}, null, [verified, null]]) assert.throws(() => checkDistribution(ledger, ['shipped']));
  assert.throws(() => checkDistribution([verified], ['missing']));
  assert.throws(() => checkDistribution([{ ...verified, status: 'unverified' }], ['shipped']));
  assert.throws(() => checkDistribution([{ ...verified, source: '' }], ['shipped']));
});

test('release notes select the requested section and reject missing sections', () => {
  const changelog = '# DualSense Command Center 0.6.0\nnew\n# DualSense Command Center 0.5.0\nold\n';
  assert.equal(releaseNotes(changelog, '0.5.0'), '# DualSense Command Center 0.5.0\nold\n');
  assert.throws(() => releaseNotes(changelog, '0.7.0'));
});
