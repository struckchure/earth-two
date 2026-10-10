// Run with: node --test tools/webaccount/loader.test.cjs
const {test} = require('node:test');
const assert = require('node:assert/strict');
const fs = require('node:fs');
const vm = require('node:vm');
const source = fs.readFileSync(__dirname + '/shell.html', 'utf8').split('<script>')[1].split('</script>')[0]
  .replace('__POINTER__', '')
  .replace('__MODULES__', '["raylib","jolt"]')
  .replace('__SIZES__', '{"raylib.data":100,"game.wasm":8}');

function boot({httpError = false, scriptError = false} = {}) {
  const elements = new Map();
  const element = id => {
    if (!elements.has(id)) elements.set(id, {hidden: false, textContent: '', value: undefined, classList: {add() {}, toggle() {}}, setAttribute() {}, removeAttribute(name) {delete this[name];}, focus() {this.focused = true;}});
    return elements.get(id);
  };
  let started;
  const running = new Promise(resolve => {started = resolve;});
  let resolveGame;
  let worldPercent;
  let streamBytes = 0;
  const context = {
    console: {log() {}, warn() {}, error() {}},
    document: {getElementById: element, createElement: () => ({}), head: {append(script) {queueMicrotask(() => scriptError ? script.onerror() : script.onload());}}},
    setInterval: () => 1, clearInterval() {}, setTimeout,
    requestAnimationFrame: callback => setTimeout(callback, 0),
    addEventListener() {}, location: {reload() {}},
    ReadableStream, Response,
    fetch: async () => new Response(new Uint8Array(8), {status: httpError ? 503 : 200, headers: {'Content-Length': '2', 'Content-Encoding': 'gzip'}}),
    createRaylib: async options => {
      // Emscripten may report a compressed total. The build manifest is authoritative.
      options.setStatus('Downloading data... (50/10)');
      worldPercent = element('progress').value;
      return {FS: {}};
    },
    createJolt: async () => ({}), installGoFS() {},
    Go: class {importObject = {}; run() {started(); return new Promise(resolve => {resolveGame = resolve;});}},
    WebAssembly: {async instantiateStreaming(responsePromise) {
      const response = await responsePromise;
      const reader = response.body.getReader();
      while (true) {const {done, value} = await reader.read(); if (done) break; streamBytes += value.length;}
      return {instance: {}};
    }}
  };
  vm.runInNewContext(source, context);
  return {element, context, running, finish: () => resolveGame(), worldPercent: () => worldPercent, streamBytes: () => streamBytes};
}

test('decoded download progress and overlay remain until the first game frame', async () => {
  const page = boot();
  await page.running;
  assert.equal(page.worldPercent(), 50);
  assert.equal(page.streamBytes(), 8);
  assert.equal(page.element('status').textContent, 'Preparing your arrival…');
  assert.equal(page.element('loading').hidden, false);
  page.context.earthTwoReady();
  assert.equal(page.element('loading').hidden, true);
  assert.equal(page.element('canvas').focused, true);
  page.finish();
});

for (const options of [{httpError: true}, {scriptError: true}]) {
  test(`failed ${options.httpError ? 'download' : 'script'} offers retry and cannot be dismissed by readiness`, async () => {
    const page = boot(options);
    await new Promise(resolve => setTimeout(resolve, 30));
    assert.equal(page.element('status').textContent, 'Unable to start');
    assert.equal(page.element('retry').hidden, false);
    page.context.earthTwoReady();
    assert.equal(page.element('loading').hidden, false);
  });
}
