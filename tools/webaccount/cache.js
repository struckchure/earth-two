// The worker is optional: plain LAN HTTP and restricted browsers still use
// normal HTTP caching. Never let worker registration delay startup indefinitely.
async function prepareBrowserCache() {
  if (typeof navigator === 'undefined' || !globalThis.isSecureContext || !('serviceWorker' in navigator)) return;
  let timer;
  let listener;
  let finished = false;
  try {
    await Promise.race([
      (async () => {
        await navigator.serviceWorker.register('service-worker.js', {updateViaCache: 'none'});
        if (finished) return;
        if (!navigator.serviceWorker.controller) {
          await new Promise(resolve => {
            listener = resolve;
            navigator.serviceWorker.addEventListener('controllerchange', listener, {once: true});
            if (navigator.serviceWorker.controller) resolve();
          });
        }
      })(),
      new Promise(resolve => { timer = setTimeout(resolve, 2500); })
    ]);
  } catch (_) { /* Loading the game remains possible without persistent caching. */ }
  finally {
    finished = true;
    clearTimeout(timer);
    if (listener) navigator.serviceWorker.removeEventListener('controllerchange', listener);
  }
}

// IndexedDB also works on ordinary LAN HTTP, where service workers cannot run.
// Keep one complete revision per artifact; replace it only after a good download.
function artifactStorage(name, entry) {
  return new Promise((resolve, reject) => {
    let db, transaction, result, finished = false;
    const finish = (error) => {
      if (finished) return;
      finished = true;
      clearTimeout(timer);
      db?.close();
      error ? reject(error) : resolve(result);
    };
    const timer = setTimeout(() => {
      transaction?.abort();
      finish(new Error('Asset storage timed out'));
    }, 10000);
    try {
      const request = indexedDB.open('earth-two-downloads', 1);
      request.onupgradeneeded = () => request.result.createObjectStore('artifacts');
      request.onerror = () => finish(request.error);
      request.onblocked = () => finish(new Error('Asset storage blocked'));
      request.onsuccess = () => {
        db = request.result;
        if (finished) { db.close(); return; }
        try {
          transaction = db.transaction('artifacts', entry ? 'readwrite' : 'readonly');
          const store = transaction.objectStore('artifacts');
          const operation = entry ? store.put(entry, name) : store.get(name);
          operation.onsuccess = () => { result = operation.result; };
          transaction.oncomplete = () => finish();
          transaction.onabort = () => finish(transaction.error || new Error('Asset storage aborted'));
          transaction.onerror = () => finish(transaction.error || new Error('Asset storage failed'));
        } catch (error) { finish(error); }
      };
    } catch (error) { finish(error); }
  });
}

async function loadSavedArtifact(name, url, version, size, report) {
  report('checking');
  try {
    const saved = await artifactStorage(name);
    if (saved?.version === version && saved.data?.size === size) {
      report('cached');
      return await saved.data.arrayBuffer();
    }
  } catch (_) { /* Storage restrictions must not prevent playing. */ }
  report('loading');
  const response = await fetch(url);
  if (!response.ok) throw new Error(`${name}: HTTP ${response.status}`);
  let bytes = 0;
  let data;
  if (response.body) {
    const reader = response.body.getReader();
    data = await new Response(new ReadableStream({
      async pull(controller) {
        try {
          const {done, value} = await reader.read();
          if (done) { controller.close(); return; }
          bytes += value.byteLength;
          report('progress', bytes);
          controller.enqueue(value);
        } catch (error) { controller.error(error); }
      },
      cancel(reason) { return reader.cancel(reason); }
    })).arrayBuffer();
  } else {
    data = await response.arrayBuffer();
  }
  if (data.byteLength !== size) throw new Error(`${name}: incomplete asset (${data.byteLength}/${size} bytes)`);
  report('saving');
  try {
    await artifactStorage(name, {version, data: new Blob([data])});
  } catch (error) {
    console.warn('Could not save game assets for the next visit:', error);
    report('unavailable');
  }
  return data;
}
