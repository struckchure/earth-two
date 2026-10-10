const {test} = require('node:test');
const assert = require('node:assert/strict');
const fs = require('node:fs');
const vm = require('node:vm');
const source = fs.readFileSync(__dirname + '/pointer.js', 'utf8');
function setup({supported = true, reject = false, touch = false} = {}) {
  const events = {};
  const canvasEvents = {};
  const hint = {hidden: true};
  let requests = 0;
  const canvas = {addEventListener(name, fn) {canvasEvents[name] = fn;}};
  if (supported) canvas.requestPointerLock = () => {requests++; if (reject) return Promise.reject(new Error('Gesture required'));};
  const document = {
    pointerLockElement: null,
    getElementById: id => id === 'canvas' ? canvas : hint,
    addEventListener(name, fn) {events[name] = fn;},
    exitPointerLock() {document.pointerLockElement = null; events.pointerlockchange();}
  };
  const context = {document, earthTwoTouch: {active: () => touch}};
  vm.runInNewContext(source, context);
  return {pointer: context.earthTwoPointer, hint, document, requests: () => requests,
    setTouch(value) {touch = value;},
    move: (movementX, movementY) => events.mousemove({movementX, movementY}),
    lock() {document.pointerLockElement = canvas; events.pointerlockchange();},
    click() {canvasEvents.mousedown({button: 0, preventDefault() {}, stopImmediatePropagation() {}});}
  };
}

test('capture only in play, accumulate movement without buttons, consume it once', () => {
  const p = setup();
  p.click();
  assert.equal(p.requests(), 0);
  p.pointer.setPlaying(true);
  assert.equal(p.requests(), 1);
  p.lock();
  p.move(12, -4); p.move(3, 2);
  assert.deepEqual(Array.from(p.pointer.takeDelta()), [15, -2]);
  assert.deepEqual(Array.from(p.pointer.takeDelta()), [0, 0]);
  assert.equal(p.hint.hidden, true);
  p.pointer.setPlaying(false);
  assert.equal(p.pointer.locked(), false);
  assert.equal(p.pointer.consumeUnlock(), false);
});

test('touch play skips pointer lock and switching to touch does not pause', () => {
  const p = setup({touch: true});
  p.pointer.setPlaying(true); p.click();
  assert.equal(p.requests(), 0);
  assert.equal(p.hint.hidden, true);
  p.setTouch(false); p.click(); p.lock();
  p.setTouch(true); p.document.exitPointerLock();
  assert.equal(p.pointer.consumeUnlock(), false);
  assert.equal(p.hint.hidden, true);
});

test('Escape signals pause once and never recaptures every frame', () => {
  const p = setup();
  p.pointer.setPlaying(true); p.lock(); p.move(10, 10);
  p.document.exitPointerLock();
  assert.equal(p.pointer.consumeUnlock(), true);
  assert.equal(p.pointer.consumeUnlock(), false);
  assert.deepEqual(Array.from(p.pointer.takeDelta()), [0, 0]);
  p.pointer.setPlaying(true);
  assert.equal(p.requests(), 1);
  p.pointer.setPlaying(false);
  p.pointer.setPlaying(true);
  assert.equal(p.requests(), 2);
});

test('late capture cannot trap the pointer over an open menu', () => {
  const p = setup();
  p.pointer.setPlaying(true);
  p.pointer.setPlaying(false);
  p.lock();
  assert.equal(p.pointer.locked(), false);
  assert.equal(p.pointer.consumeUnlock(), false);
});

test('denied requests are handled and a fresh click can retry', async () => {
  const p = setup({reject: true});
  p.pointer.setPlaying(true);
  await Promise.resolve();
  assert.equal(p.hint.hidden, false);
  p.click();
  assert.equal(p.requests(), 2);
  await Promise.resolve();
});

test('unsupported browsers retain drag controls', () => {
  const p = setup({supported: false});
  p.pointer.setPlaying(true); p.click(); p.move(4, 5);
  assert.equal(p.requests(), 0);
  assert.match(p.hint.textContent, /right-click/);
  assert.deepEqual(Array.from(p.pointer.takeDelta()), [0, 0]);
});
