const {test} = require('node:test');
const assert = require('node:assert/strict');
const fs = require('node:fs');
const vm = require('node:vm');
function setup({quota = false, status = 200} = {}) {
  const handlers = {}, entries = new Map(); let downloads = 0;
  const cache = {
    async match(r) { return entries.get(r.url)?.clone(); },
    async put(r, response) { if (quota) throw Error('Quota exceeded'); entries.set(r.url, response); },
    async keys() { return [...entries.keys()].map(url => new Request(url)); },
    async delete(r) { return entries.delete(r.url); }
  };
  const context = {URL, Promise, Set, caches: {open: async () => cache},
    fetch: async () => { downloads++; return new Response('artifact', {status}); },
    self: {registration: {scope: 'https://game.test/'}, addEventListener: (name, fn) => handlers[name] = fn}};
  vm.runInNewContext(fs.readFileSync(__dirname+'/service-worker.js', 'utf8'), context);
  return {entries, downloads: () => downloads, async request(url, options) {
    let result; const waits = [];
    handlers.fetch({request: new Request(url, options), respondWith: p => result=p, waitUntil: p => waits.push(p)});
    const response = await result; await Promise.all(waits); return response;
  }};
}
const artifact = (v, name='game.wasm') => `https://game.test/${name}?v=${v.repeat(64)}`;
test('repeat visits reuse artifacts; new versions download once; old versions are bounded', async () => {
 const p = setup();
 await p.request(artifact('a')); await p.request(artifact('a'));
 assert.equal(p.downloads(), 1);
 await p.request(artifact('b')); await p.request(artifact('c'));
 assert.equal(p.downloads(), 3);
 assert.equal(p.entries.size, 2);
 assert.equal(p.entries.has(artifact('a')), false);
 assert.equal((await p.request(artifact('c'))).status, 200);
 assert.equal(p.downloads(), 3);
});
test('navigation, accounts, cross-origin, ranges and unversioned requests bypass the cache', async () => {
 const p = setup();
 for (const url of ['https://game.test/', 'https://game.test/account?v='+ 'a'.repeat(64), 'https://other.test/game.wasm?v='+'a'.repeat(64), 'https://game.test/game.wasm', artifact('a')+'&token=x']) {
  assert.equal(await p.request(url), undefined);
 }
 assert.equal(await p.request(artifact('a'), {headers:{Range:'bytes=0-10'}}), undefined);
 assert.equal(p.downloads(), 0);
});
test('failed downloads are never cached', async () => {
 const p = setup({status:409});
 await p.request(artifact('a')); await p.request(artifact('a'));
 assert.equal(p.downloads(), 2); assert.equal(p.entries.size, 0);
});
test('storage quota failures do not stop a successful download', async () => {
 const p = setup({quota:true});
 assert.equal(await (await p.request(artifact('a'))).text(), 'artifact');
 assert.equal(p.entries.size, 0);
});
