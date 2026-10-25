import test from 'node:test';
import assert from 'node:assert/strict';
import {readFile} from 'node:fs/promises';
import {loadTs} from './load-ts.mjs';
const {wordCount} = await loadTs('../src/wordCount.ts');
const examples = JSON.parse(await readFile(new URL('../contracts/word-count.examples.json', import.meta.url), 'utf8'));
test('editor and browser word count match the shared Rust examples across ink runs', () => {
  for (const {text, count} of examples) {
    const runs = Array.from(text, (char, n) => ({text: char, color: n % 2 === 0 ? 'black' : 'blue'}));
    assert.equal(wordCount(runs), count, text);
  }
});
