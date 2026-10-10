const {test} = require('node:test');
const assert = require('node:assert/strict');
const fs = require('node:fs');
const vm = require('node:vm');
const source = fs.readFileSync(__dirname + '/cache.js', 'utf8');
// Model asynchronous database opens and transaction commits across page loads.
function storage({quota = false, blocked = false} = {}) {
  const entries = new Map();
  return {entries, open() {
    const request = {};
    setImmediate(() => {
      if (blocked) { request.onblocked(); return; }
      request.result = {close() {}, transaction() {
        const tx = {objectStore() {
          return {
            get(key) { return operation(key); },
            put(value, key) { return operation(key, value); }
          };
        }};
        function operation(key, value) {
          const op = {};
          setImmediate(() => {
            if (value && quota) { tx.error = Error('Quota exceeded'); tx.onabort(); return; }
            if (value) entries.set(key, value);
            op.result = entries.get(key);
            op.onsuccess();
            tx.oncomplete();
          });
          return op;
        }
        return tx;
      }};
      request.onsuccess();
    });
    return request;
  }};
}
function page(db, {status = 200, length = 8} = {}) {
  let downloads = 0;
  const states = [];
  const context = {indexedDB: db, Blob, Response, ReadableStream, setTimeout, clearTimeout,
    console: {warn() {}}, fetch: async () => { downloads++; return new Response(new Uint8Array(length), {status}); }};
  vm.runInNewContext(source, context);
  return {states, downloads: () => downloads,
    load: version => context.loadSavedArtifact('raylib.data', `raylib.data?v=${version}`, version, 8, state => states.push(state))};
}
test('a new page on LAN HTTP reads the saved bundle without any fetch', async () => {
  const db = storage();
  const first = page(db);
  assert.equal((await first.load('v1')).byteLength, 8);
  assert.equal(first.downloads(), 1);
  assert.equal(db.entries.size, 1); // The save is committed before startup continues.
  const reload = page(db);
  assert.equal((await reload.load('v1')).byteLength, 8);
  assert.equal(reload.downloads(), 0);
  assert.ok(reload.states.includes('cached'));
  assert.ok(!reload.states.includes('loading'));
});
test('new version replaces old bytes only after a complete successful download', async () => {
  const db = storage();
  await page(db).load('v1');
  await assert.rejects(page(db, {status: 503}).load('v2'), /HTTP 503/);
  await assert.rejects(page(db, {length: 4}).load('v2'), /incomplete asset/);
  assert.equal(db.entries.get('raylib.data').version, 'v1');
  const next = page(db);
  await next.load('v2');
  assert.equal(next.downloads(), 1);
  assert.equal(db.entries.size, 1);
  assert.equal(db.entries.get('raylib.data').version, 'v2');
});
test('missing, blocked, and full storage allow network loading', async () => {
  for (const db of [undefined, storage({blocked: true}), storage({quota: true})]) {
    const p = page(db);
    assert.equal((await p.load('v1')).byteLength, 8);
    assert.equal(p.downloads(), 1);
    assert.ok(p.states.includes('unavailable'));
  }
});
test('a saved bundle with the wrong size is fetched again', async () => {
  const db = storage();
  db.entries.set('raylib.data', {version: 'v1', data: new Blob(['bad'])});
  const p = page(db);
  assert.equal((await p.load('v1')).byteLength, 8);
  assert.equal(p.downloads(), 1);
});
