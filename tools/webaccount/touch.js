// Touch input is sampled by Go, not dispatched as synthetic keyboard events.
(() => {
  const canvas = document.getElementById('canvas');
  const map = document.getElementById('touch-map');
  const guide = document.getElementById('touch-guide');
  let guided = false;
  try { guided = localStorage.getItem('earth-two-touch-guide-v1') === '1'; } catch (_) {}
  document.getElementById('touch-guide-done').onclick = () => {
    guided = true; guide.hidden = true;
    try { localStorage.setItem('earth-two-touch-guide-v1', '1'); } catch (_) {}
    globalThis.miniaudio?.unlock?.();
  };
  const controls = document.getElementById('touch-controls');
  const stick = document.getElementById('touch-stick');
  const knob = document.getElementById('touch-knob');
  const crouch = document.getElementById('touch-crouch');
  const roll = document.getElementById('touch-roll');
  const pickup = document.getElementById('touch-pickup');
  const lights = document.getElementById('touch-lights');
  let driving = false;
  const interact = document.getElementById('touch-interact');
  const interactLabel = document.getElementById('touch-interact-label');
  let interaction = '';
  let active = matchMedia('(pointer: coarse)').matches;
  let playing = false, started = false;
  let look = [0, 0], movement = [];
  let wheel = 0;
  let mouse = {x: 0, y: 0, down: false};
  const mouseQueue = [];
  const pointers = new Map();
  const pulses = new Set();
  function render() {
    guide.hidden = !active || !started || guided;
    controls.hidden = !active || !started || !guided;
    controls.dataset.active = String(active);
    controls.dataset.playing = String(playing);
  }
  function reset() {
    pointers.clear(); pulses.clear(); movement = []; look = [0, 0];
    mouseQueue.length = 0; mouse.down = false; wheel = 0;
    knob.style.transform = '';
    for (const button of controls.querySelectorAll('[data-key]')) button.classList.remove('held');
  }
  function point(event) {
    const rect = canvas.getBoundingClientRect();
    return {x: (event.clientX - rect.left) / rect.width, y: (event.clientY - rect.top) / rect.height};
  }
  function queueMouse(event, down) {
    const next = {...point(event), down};
    // Preserve down/up edges even when a complete tap occurs between frames.
    if (mouseQueue.length && mouseQueue.at(-1).down === down) mouseQueue[mouseQueue.length - 1] = next;
    else mouseQueue.push(next);
  }
  function moveStick(event) {
    const rect = stick.getBoundingClientRect();
    const radius = rect.width * .35;
    let x = (event.clientX - rect.left - rect.width / 2) / radius;
    let y = (event.clientY - rect.top - rect.height / 2) / radius;
    const length = Math.hypot(x, y);
    if (length > 1) { x /= length; y /= length; }
    knob.style.transform = `translate(${x * radius}px, ${y * radius}px)`;
    movement = [];
    if (Math.hypot(x, y) < .2) return;
    if (x < -.3) movement.push(65);
    if (x > .3) movement.push(68);
    if (y < -.3) movement.push(87);
    if (y > .3) movement.push(83);
    if (!driving && length > .85) movement.push(340); // Shift: run at the outer edge.
  }
  document.addEventListener('pointerdown', event => {
    if (event.target !== canvas && !controls.contains(event.target)) return;
    if (event.pointerType === 'mouse') {
      if (active && event.target === canvas) { active = false; reset(); render(); }
      return;
    }
    active = true;
    render();
    if (document.pointerLockElement === canvas) document.exitPointerLock();
    if (!started || !guided) return;
    let kind, key;
    const button = event.target.closest('[data-key]');
    if (button && (button.hidden || driving && [32,67,341,82,81,70].includes(Number(button.dataset.key)))) return;
    if (button) { kind = 'key'; key = Number(button.dataset.key); pulses.add(key); button.classList.add('held'); }
    else if (event.target.closest('[data-wheel]')) {
      wheel += Number(event.target.closest('[data-wheel]').dataset.wheel); kind = 'extra';
    } else if (playing && stick.contains(event.target)) {
      if ([...pointers.values()].some(p => p.kind === 'stick')) return;
      kind = 'stick'; moveStick(event);
    } else if (event.target === canvas) {
      kind = playing ? 'look' : 'menu';
      if ([...pointers.values()].some(p => p.kind === kind)) return;
      if (kind === 'menu') queueMouse(event, true);
    } else return;
    event.preventDefault();
    event.stopPropagation();
    event.target.setPointerCapture(event.pointerId);
    pointers.set(event.pointerId, {kind, key, button, x: event.clientX, y: event.clientY});
    globalThis.miniaudio?.unlock?.();
  }, {capture: true, passive: false});
  document.addEventListener('pointermove', event => {
    const pointer = pointers.get(event.pointerId);
    if (!pointer) return;
    event.preventDefault();
    if (pointer.kind === 'stick') moveStick(event);
    if (pointer.kind === 'look') {
      look[0] += event.clientX - pointer.x;
      look[1] += event.clientY - pointer.y;
    }
    if (pointer.kind === 'menu') queueMouse(event, true);
    pointer.x = event.clientX; pointer.y = event.clientY;
  }, {passive: false});
  function end(event) {
    const pointer = pointers.get(event.pointerId);
    if (!pointer) return;
    if (pointer.kind === 'stick') { movement = []; knob.style.transform = ''; }
    if (pointer.kind === 'menu') queueMouse(event, false);
    pointer.button?.classList.remove('held');
    pointers.delete(event.pointerId);
  }
  for (const name of ['pointerup', 'pointercancel', 'lostpointercapture']) document.addEventListener(name, end);
  addEventListener('blur', reset);
  addEventListener('resize', reset);
  document.addEventListener('visibilitychange', () => { if (document.hidden) reset(); });
  globalThis.earthTwoTouch = {
    active: () => active,
    setHUD(x, y, width, height, cardBottom) {
      const rect = canvas.getBoundingClientRect();
      map.style.left = `${rect.left + x * rect.width}px`;
      map.style.top = `${rect.top + y * rect.height}px`;
      map.style.width = `${width * rect.width}px`;
      map.style.height = `${height * rect.height}px`;
      controls.style.setProperty('--hud-tools-top', `${rect.top + cardBottom * rect.height + 10}px`);
    },
    setContext(inVehicle, running, sliding, canPickup, headlamps) {
      function release(button) {
        pulses.delete(Number(button.dataset.key));
        for (const [id, pointer] of pointers) if (pointer.button === button) pointers.delete(id);
        button.classList.remove('held');
      }
      function show(button, visible) { if (!visible) release(button); button.hidden = !visible; }
      if (driving !== inVehicle) { reset(); driving = inVehicle; }
      controls.dataset.driving = String(driving);
      show(roll, !driving && running);
      show(pickup, !driving && canPickup);
      show(lights, driving);
      lights.setAttribute('aria-pressed', String(headlamps));
      const key = !driving && sliding ? '341' : '67';
      if (crouch.dataset.key !== key) {
        release(crouch);
        crouch.dataset.key = key;
        const label = key === '341' ? 'Slide' : 'Hold to crouch';
        crouch.setAttribute('aria-label', label);
        crouch.setAttribute('title', label);
      }
    },
    setInteract(available, label) {
      const next = available ? label : '';
      if (interaction === next) return;
      interaction = next;
      interact.hidden = !next;
      interactLabel.textContent = next;
      interact.setAttribute('aria-label', next || 'Interact');
      interact.setAttribute('title', next || 'Interact');
      if (!next) {
        pulses.delete(69);
        for (const [id, pointer] of pointers) if (pointer.button === interact) pointers.delete(id);
        interact.classList.remove('held');
      }
    },
    ready() { started = true; render(); },
    setPlaying(value) {
      if (playing !== value) { reset(); playing = value; }
      render();
    },
    takeLook() { const delta = look; look = [0, 0]; return delta; },
    sample() {
      const keys = new Set([...movement, ...pulses]);
      for (const pointer of pointers.values()) if (pointer.kind === 'key') keys.add(pointer.key);
      pulses.clear();
      const previous = mouse;
      if (mouseQueue.length) mouse = mouseQueue.shift();
      const result = {active, keys: [...keys], mouse: {...mouse, dx: mouse.x - previous.x, dy: mouse.y - previous.y}, wheel};
      wheel = 0;
      return result;
    }
  };
  render();
})();
