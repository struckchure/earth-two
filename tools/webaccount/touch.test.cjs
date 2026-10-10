const {test} = require('node:test');
const assert = require('node:assert/strict');
const fs = require('node:fs');
const vm = require('node:vm');
function setup(coarse = true, guided = true) {
  const handlers = {}, windowHandlers = {}, elements = {};
  function element(id, parent, dataset = {}) {
    const e = {id, parent, dataset, hidden: false, style: {setProperty() {}}, classList: {add() {}, remove() {}},
      setAttribute() {}, setPointerCapture() {},
      getBoundingClientRect: () => id === 'canvas' ? {left: 20, top: 10, width: 800, height: 400} : {left: 0, top: 0, width: 100, height: 100},
      contains(target) {return target === this || !!target.parent && this.contains(target.parent);},
      closest(selector) {return (selector === '[data-key]' && this.dataset.key || selector === '[data-wheel]' && this.dataset.wheel) ? this : this.parent?.closest(selector);},
      querySelectorAll: () => Object.values(elements).filter(e => e.dataset.key)};
    elements[id] = e; return e;
  }
  element('touch-guide'); element('touch-guide-done'); element('touch-map', undefined, {key:'77'});
  const canvas = element('canvas'), controls = element('touch-controls');
  const stick = element('touch-stick', controls);
  element('touch-knob', stick);
  element('touch-crouch', controls, {key:'67'}); element('touch-roll', controls, {key:'82'});
  element('touch-pickup', controls, {key:'81'}); element('touch-lights', controls, {key:'72'});
  element('jump', controls, {key: '32'}); element('touch-interact', controls, {key: '69'}).hidden = true; element('touch-interact-label', controls);
  const document = {getElementById: id => elements[id], pointerLockElement: null, addEventListener: (name, fn) => handlers[name] = fn};
  const context = {document, localStorage: {getItem: () => guided ? '1' : null, setItem: () => {}}, matchMedia: () => ({matches: coarse}), addEventListener: (name, fn) => windowHandlers[name] = fn};
  vm.runInNewContext(fs.readFileSync(__dirname + '/touch.js', 'utf8'), context);
  const touch = context.earthTwoTouch;
  const event = (name, target, id, x = 50, y = 50, pointerType = 'touch') => handlers[name]({target: elements[target], pointerId: id, clientX: x, clientY: y, pointerType, preventDefault() {}, stopPropagation() {}});
  return {touch, controls, elements, event, windowHandlers, handlers, document, sample: () => JSON.parse(JSON.stringify(touch.sample()))};
}

test('move, look, and jump work with independent simultaneous touches', () => {
  const p = setup(); p.touch.ready(); p.touch.setPlaying(true);
  p.event('pointerdown', 'touch-stick', 1, 50, 10);
  p.event('pointerdown', 'canvas', 2, 500, 100);
  p.event('pointermove', 'canvas', 2, 530, 112);
  p.event('pointerdown', 'jump', 3);
  assert.deepEqual(p.sample().keys.sort((a,b)=>a-b), [32,87,340]);
  assert.deepEqual(Array.from(p.touch.takeLook()), [30,12]);
  assert.deepEqual(Array.from(p.touch.takeLook()), [0,0]);
  p.event('pointerup', 'jump', 3);
  assert.deepEqual(p.sample().keys.sort((a,b)=>a-b), [87,340]);
  p.event('pointercancel', 'touch-stick', 1);
  assert.deepEqual(p.sample().keys, []);
});

test('quick action taps survive until sampled and do not stick', () => {
  const p = setup(); p.touch.ready(); p.touch.setPlaying(true);
  p.touch.setInteract(true, 'Sit down');
  p.event('pointerdown', 'touch-interact', 1); p.event('pointerup', 'touch-interact', 1);
  assert.deepEqual(p.sample().keys, [69]);
  assert.deepEqual(p.sample().keys, []);
});

test('menu taps retain press and release with canvas-relative coordinates', () => {
  const p = setup(); p.touch.ready();
  p.event('pointerdown', 'canvas', 1, 420, 210); p.event('pointerup', 'canvas', 1, 420, 210);
  const down = p.sample();
  assert.deepEqual(down.mouse, {x:.5,y:.5,down:true,dx:.5,dy:.5});
  assert.equal(p.sample().mouse.down, false);
});

test('pause, blur, rotation, and visibility changes clear held gestures', () => {
  for (const reset of [p => p.touch.setPlaying(false), p => p.windowHandlers.blur(), p => p.windowHandlers.resize(), p => {p.document.hidden=true; p.handlers.visibilitychange();}]) {
    const p = setup(); p.touch.ready(); p.touch.setPlaying(true);
    p.event('pointerdown', 'jump', 1); p.event('pointerdown', 'touch-stick', 2, 90, 50);
    reset(p);
    assert.deepEqual(p.sample().keys, []);
  }
});

test('desktop stays uncluttered; first touch enables controls; mouse restores desktop', () => {
  const p = setup(false); p.touch.ready();
  assert.equal(p.controls.hidden, true);
  p.event('pointerdown', 'canvas', 1);
  assert.equal(p.touch.active(), true);
  assert.equal(p.controls.hidden, false);
  p.event('pointerdown', 'canvas', 2, 50, 50, 'mouse');
  assert.equal(p.touch.active(), false);
  assert.equal(p.controls.hidden, true);
});

 test('Interact follows the available action and releases input when it disappears', () => {
  const p = setup(); p.touch.ready(); p.touch.setPlaying(true);
  assert.equal(p.elements['touch-interact'].hidden, true);
  p.touch.setInteract(true, 'Drive the buggy');
  assert.equal(p.elements['touch-interact'].hidden, false);
  assert.equal(p.elements['touch-interact-label'].textContent, 'Drive the buggy');
  p.event('pointerdown', 'touch-interact', 1);
  assert.deepEqual(p.sample().keys, [69]);
  p.touch.setInteract(false, '');
  assert.equal(p.elements['touch-interact'].hidden, true);
  assert.deepEqual(p.sample().keys, []);
  p.event('pointerdown', 'touch-interact', 2);
  assert.deepEqual(p.sample().keys, []);
});

test('context switches crouch to slide, gates roll/pickup, and clears held actions on entering a vehicle', () => {
  const p = setup(); p.touch.ready(); p.touch.setPlaying(true);
  const e = p.elements;
  p.touch.setContext(false, false, false, false, false);
  assert.equal(e['touch-crouch'].dataset.key, '67');
  assert.equal(e['touch-roll'].hidden, true);
  assert.equal(e['touch-pickup'].hidden, true);
  assert.equal(e['touch-lights'].hidden, true);
  p.touch.setContext(false, true, true, true, false);
  assert.equal(e['touch-crouch'].dataset.key, '341');
  assert.equal(e['touch-roll'].hidden, false);
  assert.equal(e['touch-pickup'].hidden, false);
  p.event('pointerdown', 'touch-crouch', 1);
  assert.deepEqual(p.sample().keys, [341]);
  p.touch.setContext(false, false, false, false, false);
  assert.deepEqual(p.sample().keys, []);
  p.event('pointerdown', 'jump', 2);
  p.touch.setContext(true, false, false, false, true);
  assert.deepEqual(p.sample().keys, []);
  assert.equal(e['touch-lights'].hidden, false);
  assert.equal(p.controls.dataset.driving, 'true');
  p.event('pointerdown', 'jump', 3);
  assert.deepEqual(p.sample().keys, []);
  p.event('pointerdown', 'touch-lights', 4);
  assert.deepEqual(p.sample().keys, [72]);
  p.touch.setContext(false, false, false, false, false);
  assert.deepEqual(p.sample().keys, []);
});

test('first touch guide blocks game input until dismissed', () => {
 const p = setup(true, false); p.touch.ready();
 assert.equal(p.elements['touch-guide'].hidden, false);
 p.event('pointerdown', 'jump', 1);
 assert.deepEqual(p.sample().keys, []);
 p.elements['touch-guide-done'].onclick();
 p.touch.setPlaying(true);
 assert.equal(p.elements['touch-guide'].hidden, true);
 assert.equal(p.controls.hidden, false);
 p.event('pointerdown', 'jump', 2);
 assert.deepEqual(p.sample().keys, [32]);
});
test('minimap touch target follows canvas size and queues the map action', () => {
 const p = setup(); p.touch.ready(); p.touch.setPlaying(true);
 p.touch.setHUD(.02,.04,.15,.3,.2);
 assert.equal(p.elements['touch-map'].style.left, '36px');
 assert.equal(p.elements['touch-map'].style.width, '120px');
 p.elements['touch-map'].parent = p.controls;
 p.event('pointerdown', 'touch-map', 1);
 assert.deepEqual(p.sample().keys, [77]);
});
