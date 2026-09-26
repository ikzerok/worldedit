async (page) => {
  const errors = [];
  page.on("pageerror", (error) => errors.push(error.message));
  page.on("console", (message) => {
    if (message.type() === "error") errors.push(message.text());
  });
  page.on("dialog", async (dialog) => {
    if (dialog.type() === "beforeunload") await dialog.accept();
  });

  const url = "http://127.0.0.1:18765/";
  if (page.url() !== url) await page.goto(url, { waitUntil: "load" });
  await page.waitForTimeout(3000);

  const state = await page.evaluate(() => ({
    title: document.title,
    loadingVisible: Boolean(document.getElementById("loading")),
    canvasCount: document.querySelectorAll("canvas").length,
    canvasSize: [...document.querySelectorAll("canvas")].map(({ width, height }) => ({ width, height })),
  }));
  await page.screenshot({ path: "D:/Desktop/Code/work-worldline/qa-capqa-stage1/evidence/browser-startup-green.png" });

  if (state.loadingVisible || state.canvasCount !== 1 || errors.length > 0) {
    throw new Error(`WASM startup failed: state=${JSON.stringify(state)} errors=${JSON.stringify(errors)}`);
  }
  return { state, errors };
}
