import {test} from 'node:test';
import assert from 'node:assert/strict';
import {loadTs} from './load-ts.mjs';
const {prepareReviewSource} = await loadTs('../src/reviewWorkflow.ts');
const version = (id, fixed = true) => ({id, sessionId: 'essay-a', fixed});
const session = (status, versions = []) => ({session: {id: 'essay-a', status}, versions});

test('review fixes the typed original once and returns that exact source', async () => {
  const calls = [];
  for (const status of ['draft', 'editing_original']) {
    const source = await prepareReviewSource(session(status, [version('old')]), {
      submitOriginal: async id => {calls.push(['submit', id]); return version('new');},
      finishRevision: async () => assert.fail('must not finish an original as a revision'),
    });
    assert.deepEqual(source, {sessionId: 'essay-a', versionId: 'new'});
  }
  assert.deepEqual(calls, [['submit', 'essay-a'], ['submit', 'essay-a']]);
});

test('re-review fixes the working revision, preserving the already reviewed original', async () => {
  const original = version('original');
  const working = version('revision', false);
  const source = await prepareReviewSource(session('revising', [original, working]), {
    submitOriginal: async () => assert.fail('must not overwrite the original'),
    finishRevision: async id => {assert.equal(id, 'essay-a'); return version('revision');},
  });
  assert.equal(source.versionId, 'revision');
  assert.equal(original.id, 'original');
  assert.equal(working.fixed, false);
});

test('already fixed and recovered submissions reuse the latest fixed source without mutation', async () => {
  const source = await prepareReviewSource(session('original_submitted', [version('original'), version('latest'), version('working', false)]), {
    submitOriginal: async () => assert.fail('must not submit twice'),
    finishRevision: async () => assert.fail('must not fix twice'),
  });
  assert.equal(source.versionId, 'latest');
});

test('save/fix errors and invalid sources stop submission instead of reviewing stale text', async () => {
  const actions = {submitOriginal: async () => {throw Error('disk failure');}, finishRevision: async () => version('unused')};
  await assert.rejects(prepareReviewSource(session('draft'), actions), /disk failure/);
  await assert.rejects(prepareReviewSource(session('original_submitted'), actions), /固定稿件/);
  await assert.rejects(prepareReviewSource(session('draft'), {...actions, submitOriginal: async () => version('unfinished', false)}), /固定稿件/);
  await assert.rejects(prepareReviewSource(session('draft'), {...actions, submitOriginal: async () => ({...version('wrong'), sessionId: 'another-essay'})}), /固定稿件/);
});
