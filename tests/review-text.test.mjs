import assert from 'node:assert/strict';
import test from 'node:test';
import {loadTs} from './load-ts.mjs';
const {textDiff, sourceParts} = await loadTs('../src/reviewText.ts');

test('diff preserves deletion, insertion, replacement, punctuation and spaces exactly', () => {
  for (const [a,b] of [['a bus','an independent bus'],['word,word','word, word'],['a bus.','bus.'],['He go.','He goes.'],['','Add.'],['Delete.',''],['🙂 A  bus\n','🙂 A bus\n']]) {
    const diff = textDiff(a,b);
    assert.equal(diff.before.map(p => p.text).join(''),a);
    assert.equal(diff.after.map(p => p.text).join(''),b);
    assert.equal(diff.before.filter(p=>!p.changed).map(p=>p.text).join(''),diff.after.filter(p=>!p.changed).map(p=>p.text).join(''));
  }
  assert.equal(textDiff('word,word','word, word').after.filter(p=>p.changed).map(p=>p.text).join(''),' ');
  assert.equal(textDiff('He go.','He goes.').before.filter(p=>p.changed).map(p=>p.text).join(''),'go');
});
test('overlap splits source without losing text, ink or related annotations', () => {
  const runs=[{text:'He ',color:'black'},{text:'go home.\nHe go home.',color:'blue'}];
  const notes=[{id:'a',start:0,end:11,original:'He go home.',suggestion:'He goes home.',scope:'text'}, {id:'b',start:3,end:10,original:'go home',suggestion:'goes home',scope:'text'}, {id:'bad',start:null,end:null}];
  const parts=sourceParts(runs,notes,'a');
  assert.equal(parts.map(p=>p.text).join(''),runs.map(r=>r.text).join(''));
  assert.ok(parts.some(p=>p.ids.join(',')==='a,b'));
  assert.equal(parts.filter(p=>p.deleted).map(p=>p.text).join(''),'go');
  assert.ok(parts.filter(p=>p.start>=3).every(p=>p.color==='blue'));
  assert.ok(parts.filter(p=>p.start>=11).every(p=>!p.selected&&!p.ids.length));
});
test('long diff remains bounded and reconstructs original and corrected text', () => {
  const a='same '.repeat(1100)+'old', b='same '.repeat(1100)+'new';
  const d=textDiff(a,b);
  assert.equal(d.before.map(p=>p.text).join(''),a);
  assert.equal(d.after.map(p=>p.text).join(''),b);
  assert.equal(d.after.filter(p=>p.changed).map(p=>p.text).join(''),'new');
});
