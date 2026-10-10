// Browser camera input is independent of raylib's absolute-position mouse API.
// Go sets the gameplay state and consumes relative movement once per frame.
(() => {
  const canvas = document.getElementById("canvas");
  const hint = document.getElementById("pointer-hint");
  const supported = typeof canvas.requestPointerLock === "function";
  let playing = false;
  let pending = false;
  let unlocked = false;
  let x = 0, y = 0;
  const locked = () => document.pointerLockElement === canvas;
  function updateHint() {
    hint.hidden = !playing || locked();
    hint.textContent = supported
      ? "Click to capture mouse · Esc to pause"
      : "Hold right-click to look around · Esc to pause";
  }
  function request() {
    if (!playing || !supported || locked() || pending) return;
    pending = true;
    try {
      // Older browsers return void; newer ones also expose a Promise.
      const result = canvas.requestPointerLock();
      result?.catch(() => { pending = false; updateHint(); });
    } catch (_) { pending = false; updateHint(); }
  }
  document.addEventListener("pointerlockchange", () => {
    pending = false;
    x = y = 0;
    if (locked()) {
      // A pending request may complete after a menu has opened.
      if (!playing) document.exitPointerLock();
    } else if (playing) {
      unlocked = true;
    }
    updateHint();
  });
  document.addEventListener("pointerlockerror", () => {
    pending = false;
    updateHint();
  });
  document.addEventListener("mousemove", event => {
    if (playing && locked()) {
      x += event.movementX || 0;
      y += event.movementY || 0;
    }
  });
  canvas.addEventListener("mousedown", event => {
    if (playing && supported && !locked() && event.button === 0) {
      request();
      // Capturing the mouse must not also activate a HUD control or attack.
      event.preventDefault();
      event.stopImmediatePropagation();
    }
  }, true);
  globalThis.earthTwoPointer = {
    setPlaying(value) {
      if (playing === value) return;
      playing = value;
      x = y = 0;
      unlocked = false;
      if (playing) request();
      else if (locked()) document.exitPointerLock();
      updateHint();
    },
    locked,
    takeDelta() {
      const delta = [x, y];
      x = y = 0;
      return delta;
    },
    consumeUnlock() {
      const value = unlocked;
      unlocked = false;
      return value;
    }
  };
})();
