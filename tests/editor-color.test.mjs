import {test} from 'node:test';
import assert from 'node:assert/strict';
import {loadTs} from './load-ts.mjs';
const {colorCommittedInput}=await loadTs('../src/editor.ts');
const run=(text,color='black')=>({text,color});

test('IME commit colours only inserted text and retains surrounding ink',()=>{
 const before=[run('Old '),run('ending','blue')];
 const after=[run('Old 中文'),run('ending','blue')];
 assert.deepEqual(colorCommittedInput(before,after,'red',{start:4,end:4}),[run('Old '),run('中文','red'),run('ending','blue')]);
 assert.deepEqual(before,[run('Old '),run('ending','blue')]);
});

test('repeated text uses the actual insertion location, not the last matching character',()=>{
 assert.deepEqual(colorCommittedInput([run('aaa')],[run('aaaa')],'green',{start:1,end:1}),[run('a'),run('a','green'),run('aa')]);
});

test('replacing a selection including identical text uses the selected ink',()=>{
 assert.deepEqual(colorCommittedInput([run('same text')],[run('same text')],'blue',{start:0,end:4}),[run('same','blue'),run(' text')]);
 assert.deepEqual(colorCommittedInput([run('old text')],[run('new text')],'green',{start:0,end:3}),[run('new','green'),run(' text')]);
});

test('composition replacement may span several existing colour runs',()=>{
 assert.deepEqual(colorCommittedInput([run('a'),run('b','blue'),run('c','red')],[run('a中'),run('文c','red')],'green',{start:1,end:2}),[run('a'),run('中文','green'),run('c','red')]);
});

test('an IME that recreates its surroundings does not reset their ink',()=>{
 assert.deepEqual(colorCommittedInput([run('old','red'),run('tail','blue')],[run('old中文tail')],'green',{start:3,end:3}),[run('old','red'),run('中文','green'),run('tail','blue')]);
});

test('IME reconversion outside the initial caret falls back to the actual text change',()=>{
 assert.deepEqual(colorCommittedInput([run('word end')],[run('词 end')],'red',{start:4,end:4}),[run('词','red'),run(' end')]);
});

test('cancelled collapsed composition and deletion do not recolour existing text',()=>{
 assert.deepEqual(colorCommittedInput([run('unchanged')],[run('unchanged')],'red',{start:2,end:2}),[run('unchanged')]);
 assert.deepEqual(colorCommittedInput([run('abc')],[run('ac')],'red',{start:1,end:2}),[run('ac')]);
});
