(() => {
  if (window !== window.top || window.opener) return;
  let nextId = 0;
  const pending = new Map(), lockListeners = new Set();
  const post = (command, payload = {}) => new Promise((resolve, reject) => {
    const id = ++nextId;
    const timeout = setTimeout(() => { pending.delete(id); reject(new Error('桌面服务响应超时')); }, 15000);
    pending.set(id, { resolve, reject, timeout });
    window.ipc.postMessage(JSON.stringify({ id, command, ...payload }));
  });
  window.__nativeReply = (id, value, error) => {
    const task = pending.get(id); if (!task) return;
    clearTimeout(task.timeout); pending.delete(id);
    error ? task.reject(new Error(error)) : task.resolve(value);
  };
  window.__nativeLocked = value => lockListeners.forEach(fn => fn(value));
  window.__nativeAudio = data => window.dispatchEvent(new CustomEvent('native-pcm', { detail: data }));
  window.desktopPetBridge = {
    isElectron: false, isRust: true,
    assetProtocolBase: 'https://pet.localhost/character_assets',
    externalAssetDir: 'character_assets',
    setLocked: value => post('set_locked', { value }), getLocked: () => post('get_locked'),
    onLocked: callback => { lockListeners.add(callback); return () => lockListeners.delete(callback); },
    showConsole: () => void post('show_console'), hideConsole: () => void post('hide_console'),
    quit: () => void post('quit'), openSoundSettings: () => void post('sound_settings'),
    startLoopback: () => post('audio_start'), stopLoopback: () => post('audio_stop'),
  };
  // WebView2 supports app-region too; explicit drag keeps behavior consistent on older runtimes.
  function installDrag(doc, target) {
    doc.addEventListener('mousedown', e => {
      if (e.button !== 0 || !(e.target instanceof target.Element)) return;
      if (e.target.closest('button,input,select,textarea,a,summary')) return;
      const region = e.target.closest('.console-titlebar,.pet-stage:not(.pet-locked)');
      if (region) { e.preventDefault(); void post('drag', { target: region.matches('.console-titlebar') ? 'console' : 'pet' }); }
    });
  }
  installDrag(document, window);
  const originalOpen = window.open.bind(window);
  window.open = (...args) => {
    const host = originalOpen(...args);
    if (host && args[1] === 'pet-console') {
      // All audio/timer/state is still owned by the opener's single React tree.
      installDrag(host.document, host);
    }
    return host;
  };
  addEventListener('error', e => window.ipc.postMessage(JSON.stringify({ command:'frontend_error', message:String(e.error?.stack || e.message) })));
  addEventListener('unhandledrejection', e => window.ipc.postMessage(JSON.stringify({ command:'frontend_error', message:String(e.reason?.stack || e.reason) })));
  // Shape the native window from the visible alpha, including holes inside PNGs.
  const installHitRegion = () => void import('/pet-hit-region.js').catch(error =>
    window.ipc.postMessage(JSON.stringify({ command:'frontend_error', message:String(error) })));
  if (document.readyState === 'loading') addEventListener('DOMContentLoaded', installHitRegion, { once:true });
  else installHitRegion();
})();
