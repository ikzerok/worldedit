async (page) => {
  const base = 'http://127.0.0.1:8788/?eds11=1';
  const results = { browser: 'Microsoft Edge (Playwright headless)', paths: [], responses: [] };
  page.on('response', response => { if (response.url().startsWith('http://127.0.0.1:8788/')) results.responses.push({url: response.url(), status: response.status()}); });
  const waitFrame = () => page.waitForTimeout(180);
  const capture = async (name, scale = 'css') => page.screenshot({ path: `docs/prototypes/eds11/evidence/20260927/${name}.png`, scale });
  const metrics = async () => page.evaluate(async () => {
    const canvas = document.querySelector('#worldedit-canvas');
    const rect = canvas.getBoundingClientRect();
    const resizeObservation = await new Promise(resolve => {
      const observer = new ResizeObserver(entries => {
        const entry = entries[0];
        const list = value => value ? Array.from(value, item => ({
          inlineSize: item.inlineSize,
          blockSize: item.blockSize,
        })) : null;
        observer.disconnect();
        resolve({
          contentBoxSize: list(entry.contentBoxSize),
          devicePixelContentBoxSize: list(entry.devicePixelContentBoxSize),
          contentRect: { width: entry.contentRect.width, height: entry.contentRect.height },
        });
      });
      try {
        observer.observe(canvas, { box: 'device-pixel-content-box' });
      } catch (error) {
        resolve({ unsupported: String(error) });
      }
    });
    return {
      userAgent: navigator.userAgent,
      dpr: window.devicePixelRatio,
      canvasBacking: [canvas.width, canvas.height],
      viewport: [window.innerWidth, window.innerHeight],
      canvasClient: [canvas.clientWidth, canvas.clientHeight],
      canvasRect: [rect.x, rect.y, rect.width, rect.height],
      resizeObservation,
      localStorageKeys: Object.keys(localStorage),
      loadingOverlay: Boolean(document.querySelector('#loading')),
    };
  });
  const open = async () => { await page.goto(base); await page.waitForFunction(() => !document.querySelector('#loading'), null, { timeout: 15000 }); await waitFrame(); };

  await open();
  await page.setViewportSize({ width: 1024, height: 640 });
  const baseline = await metrics();
  if (baseline.dpr !== 1 || baseline.viewport[0] !== 1024 || baseline.viewport[1] !== 640) {
    throw new Error(`unexpected scaled viewport: ${JSON.stringify(baseline)}`);
  }
  if (baseline.canvasClient[0] !== 1024 || baseline.canvasClient[1] !== 640 || baseline.loadingOverlay) {
    throw new Error(`canvas did not settle at requested logic size: ${JSON.stringify(baseline)}`);
  }
  if (baseline.localStorageKeys.length !== 0) throw new Error('prototype unexpectedly wrote localStorage');
  results.baseline = baseline;
  await capture('web-1024x640-dpr1', 'device');

  // J1: drag preview, cancel, apply once, undo once, and reject stale/read-only writes.
  await page.mouse.move(230, 132); await page.mouse.down(); await page.mouse.move(272, 132, { steps: 8 }); await page.mouse.up();
  await waitFrame(); await capture('web-j1-preview-drag-dpr1');
  await page.mouse.click(244, 174); await waitFrame(); await capture('web-j1-cancel-dpr1');
  await page.mouse.move(230, 132); await page.mouse.down(); await page.mouse.move(255, 132, { steps: 6 }); await page.mouse.up();
  await page.mouse.click(370, 174); await waitFrame(); await capture('web-j1-apply-dpr1');
  await page.mouse.click(480, 174); await waitFrame(); await capture('web-j1-undo-dpr1');
  await page.mouse.move(230, 132); await page.mouse.down(); await page.mouse.move(255, 132, { steps: 6 }); await page.mouse.up();
  await page.mouse.click(512, 12); await page.mouse.click(370, 174); await waitFrame();
  await capture('web-j1-readonly-reject-dpr1');
  await page.mouse.click(512, 12); await page.mouse.click(558, 12); await page.mouse.click(370, 174); await waitFrame();
  await capture('web-j1-stale-reject-dpr1');
  results.paths.push('J1 pointer drag/cancel/apply/undo and read-only/stale rejection rendered');

  // J2: inject a synthetic DOM composition sequence, then test committed text/focus recovery separately.
  await open(); await page.setViewportSize({ width: 1024, height: 640 });
  await page.mouse.click(48, 198); await page.mouse.click(320, 140); await waitFrame();
  const activeElement = await page.evaluate(() => document.activeElement?.tagName);
  if (activeElement !== 'INPUT') throw new Error(`Web TextAgent not focused: ${activeElement}`);
  await page.evaluate(() => {
    const input = document.activeElement;
    input.dispatchEvent(new CompositionEvent('compositionstart', { bubbles: true, data: '' }));
    input.dispatchEvent(new CompositionEvent('compositionupdate', { bubbles: true, data: '雾港' }));
    input.dispatchEvent(new CompositionEvent('compositionend', { bubbles: true, data: '雾港' }));
  });
  await waitFrame(); await capture('web-j2-synthetic-composition-event-dpr1');
  await page.mouse.click(235, 333); await waitFrame(); await capture('web-j2-temporary-reader-dpr1');
  await page.keyboard.press('Escape'); await waitFrame();
  await page.keyboard.insertText('继续'); await waitFrame(); await capture('web-j2-focus-return-dpr1');
  results.paths.push('J2 synthetic DOM composition sequence, drawer/Escape event path, and committed CJK text after focus return (not OS IME validation)');

  // J3: actual core compile output and distinct preview / fake runtime switches.
  await open(); await page.setViewportSize({ width: 1024, height: 640 });
  await page.mouse.click(48, 221); await waitFrame();
  await page.mouse.click(230, 130); await page.mouse.click(308, 130); await waitFrame();
  await capture('web-j3-core-preview-runtime-dpr1');
  results.paths.push('J3 core compile page and separate preview/fake runtime actions rendered');

  // J4: fake three-way proposal comparison; the only action explicitly says it does not accept.
  await open(); await page.setViewportSize({ width: 1024, height: 640 });
  await page.mouse.click(48, 244); await waitFrame(); await page.mouse.click(260, 130); await waitFrame();
  await capture('web-j4-fake-three-way-dpr1');
  results.paths.push('J4 explicitly fake three-way comparison and non-accept action rendered');

  await page.setViewportSize({ width: 700, height: 640 }); await waitFrame();
  results.narrow = await metrics();
  if (results.narrow.canvasClient[0] !== 700 || results.narrow.canvasClient[1] !== 640) {
    throw new Error(`narrow canvas mismatch: ${JSON.stringify(results.narrow)}`);
  }
  await capture('web-700x640-dpr1', 'device');
  if (results.narrow.localStorageKeys.length !== 0) throw new Error('prototype unexpectedly wrote localStorage after tasks');
  return results;
}
