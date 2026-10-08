// Run the real Go browser adapter against the official JS SDK on a local DB.
// This emulates browser storage with a test-only in-memory map; no real player's
// key or connection credentials are loaded or saved.
const fs = require('node:fs');
const path = require('node:path');
const vm = require('node:vm');
const storage = new Map();
globalThis.localStorage = { getItem: key => storage.get(key) ?? null, setItem: (key,value) => storage.set(key,String(value)), removeItem: key => storage.delete(key) };
const host = new URL(process.env.EARTH_TWO_TEST_STDB_HOST);
if (!['localhost','127.0.0.1','[::1]'].includes(host.hostname)) throw new Error('WASM integration tests require a disposable local database');
vm.runInThisContext(fs.readFileSync(path.join(process.cwd(),'build/spacetime/account.js'),'utf8'));
require(path.join(process.env.GOROOT,'lib/wasm/wasm_exec.js'));
const go = new Go();
let exitCode = 0;
go.exit = code => { exitCode = code; };
go.argv = process.argv.slice(2);
go.env = process.env;
WebAssembly.instantiate(fs.readFileSync(go.argv[0]),go.importObject).then(async result => { await go.run(result.instance); process.exit(exitCode); }).catch(error => { console.error(error); process.exit(1); });
