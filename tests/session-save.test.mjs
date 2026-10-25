import test from 'node:test';
import assert from 'node:assert/strict';
import {loadTs} from './load-ts.mjs';
const {SessionSave} = await loadTs('../src/sessionSave.ts');
const runs = text => [{text, color: 'black'}];
const deferred = () => {let resolve, reject; const promise = new Promise((ok, fail) => {resolve = ok; reject = fail;}); return {promise, resolve, reject};};
const receipt = revision => ({revision, savedAt: 'now', wordCount: 1});

test('leaving/closing waits for in-flight writes even when no further edits occurred', async () => {
  const request = deferred(); const saver = new SessionSave(() => request.promise, () => {}, () => {});
  saver.reset('essay-a', 3, []); saver.update(runs('First'));
  const autosave = saver.flush(); let quit = false;
  const close = saver.flush().then(saved => { if (saved) quit = true; });
  await Promise.resolve(); assert.equal(quit, false);
  assert.throws(() => saver.reset('essay-b', 0, []));
  request.resolve(receipt(4)); await Promise.all([autosave, close]);
  assert.equal(quit, true); assert.equal(saver.hasUnsaved, false);
});

test('flush drains edits made during a write and keeps revision numbers in their session', async () => {
  const first = deferred(); const calls = [];
  const saver = new SessionSave((id, revision, value) => {
    calls.push({id, revision, value});
    return calls.length === 1 ? first.promise : Promise.resolve(receipt(revision + 1));
  }, () => {}, () => {});
  saver.reset('essay-a', 2, []); saver.update(runs('First')); const flush = saver.flush();
  saver.update(runs('First and second')); first.resolve(receipt(3)); assert.equal(await flush, true);
  assert.deepEqual(calls.map(c => [c.id, c.revision, c.value[0].text]), [
    ['essay-a', 2, 'First'], ['essay-a', 3, 'First and second'],
  ]);
  saver.reset('essay-b', 7, []); saver.update(runs('Other')); await saver.flush();
  assert.equal(calls[2].revision, 7); assert.equal(calls[2].id, 'essay-b');
});

test('failed saves block exit and session reset, preserving the newest edit for retry', async () => {
  const request = deferred(); const calls = []; const errors = [];
  const saver = new SessionSave((id, revision, value) => {
    calls.push({id, revision, value});
    return calls.length === 1 ? request.promise : Promise.resolve(receipt(revision + 1));
  }, () => {}, error => errors.push(error));
  saver.reset('essay-a', 0, []); saver.update(runs('Before')); const first = saver.flush();
  saver.update(runs('Latest')); const closing = saver.flush(); request.reject(Error('disk failure'));
  assert.equal(await first, false); assert.equal(await closing, false);
  assert.equal(errors.length, 1); assert.throws(() => saver.reset(null, 0, []));
  assert.equal(await saver.flush(), true);
  assert.equal(calls[1].value[0].text, 'Latest'); assert.equal(calls[1].revision, 0);
});

test('first quick-writing edit survives delayed document creation', async () => {
  const calls = []; const saver = new SessionSave((id, revision, value) => {
    calls.push({id, revision, value}); return Promise.resolve(receipt(1));
  }, () => {}, () => {});
  saver.reset(null, 0, []); saver.update(runs('First typed text')); assert.equal(await saver.flush(), false);
  saver.attach('essay-new', 0); assert.equal(await saver.flush(), true);
  assert.equal(calls[0].value[0].text, 'First typed text');
});
