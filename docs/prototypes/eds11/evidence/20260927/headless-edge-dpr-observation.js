async (page) => {
  const url = 'http://127.0.0.1:8788/?eds11=1';
  await page.goto(url);
  await page.waitForFunction(() => !document.querySelector('#loading'), null, { timeout: 15000 });
  await page.setViewportSize({ width: 1024, height: 640 });
  await page.waitForTimeout(200);
  const observation = await page.evaluate(() => new Promise(resolve => {
    const canvas = document.querySelector('#worldedit-canvas');
    const rect = canvas.getBoundingClientRect();
    const observer = new ResizeObserver(entries => {
      const entry = entries[0];
      const size = value => value ? Array.from(value, item => ({
        inlineSize: item.inlineSize,
        blockSize: item.blockSize,
      })) : null;
      observer.disconnect();
      resolve({
        userAgent: navigator.userAgent,
        isHeadlessEmulation: true,
        devicePixelRatio: window.devicePixelRatio,
        viewportCssPixels: { width: window.innerWidth, height: window.innerHeight },
        canvasCssClientPixels: { width: canvas.clientWidth, height: canvas.clientHeight },
        canvasCssRect: { x: rect.x, y: rect.y, width: rect.width, height: rect.height },
        canvasBackingPixels: { width: canvas.width, height: canvas.height },
        resizeObserverContentRect: { width: entry.contentRect.width, height: entry.contentRect.height },
        resizeObserverContentBoxSize: size(entry.contentBoxSize),
        resizeObserverDevicePixelContentBoxSize: size(entry.devicePixelContentBoxSize),
        localStorageKeys: Object.keys(localStorage),
      });
    });
    try {
      observer.observe(canvas, { box: 'device-pixel-content-box' });
    } catch (error) {
      observer.disconnect();
      resolve({ unsupported: String(error) });
    }
  }));
  if (observation.devicePixelRatio !== 1.5) {
    throw new Error(`expected configured emulated DPR 1.5: ${JSON.stringify(observation)}`);
  }
  await page.screenshot({
    path: 'docs/prototypes/eds11/evidence/20260927/web-1024x640-dpr1_5-emulated.png',
    scale: 'device',
  });
  return { observation, screenshot: 'web-1024x640-dpr1_5-emulated.png' };
}
