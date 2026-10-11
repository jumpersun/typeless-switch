import test from 'node:test';
import assert from 'node:assert/strict';
import fs from 'node:fs';
import ts from 'typescript';
import { readPreferences, resolvedTheme } from '../src/preferences-model.ts';
import { english } from '../src/translations.ts';
test('missing or damaged preferences fall back without affecting the vault',()=>{
 assert.deepEqual(readPreferences(null,'zh-CN'),{language:'zh',theme:'system'});
 assert.equal(readPreferences('{bad','en-US').language,'en');
 for(const raw of ['null','42','[]','{"theme":"invalid","appearance":"bad","language":"fr"}'])assert.deepEqual(readPreferences(raw,'zh'),{language:'zh',theme:'system'});
});
test('legacy appearances are discarded while language and theme survive',()=>{
 for(const appearance of ['glass','panel'])for(const language of ['zh','en'])for(const theme of ['system','light','dark']){
  const preference={language,theme};assert.deepEqual(readPreferences(JSON.stringify({...preference,appearance})),preference);
 }
});
test('manual theme overrides system, while automatic follows changes',()=>{
 for(const system of [false,true]){assert.equal(resolvedTheme('light',system),'light');assert.equal(resolvedTheme('dark',system),'dark');}
 assert.equal(resolvedTheme('system',false),'light');assert.equal(resolvedTheme('system',true),'dark');
});
test('all statically keyed UI messages have English translations',()=>{
 for(const file of ['src/main.tsx','src/Settings.tsx']){
  const source=ts.createSourceFile(file,fs.readFileSync(new URL('../'+file,import.meta.url),'utf8'),ts.ScriptTarget.Latest,true,ts.ScriptKind.TSX);
  function walk(n){
   if(ts.isCallExpression(n)&&n.expression.getText(source)==='t'&&ts.isStringLiteral(n.arguments[0])){
    const key=n.arguments[0].text.replace(/\s+/g,' ').trim();assert.ok(key in english,`${file}: ${key}`);
   }
   ts.forEachChild(n,walk);
  }walk(source);
 }
 for(const [key,value] of Object.entries(english)){
  assert.equal(/[\u3400-\u9fff]/u.test(value),false,`Untranslated: ${key}`);
  assert.deepEqual([...(key.match(/\{\d+\}/g)??[])].sort(),[...(value.match(/\{\d+\}/g)??[])].sort(),`Placeholders: ${key}`);
 }
});
