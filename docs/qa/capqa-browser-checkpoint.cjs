async (page) => {
  const errors = [];
  page.on("pageerror", (error) => errors.push(error.message));
  page.on("console", (message) => {
    if (message.type() === "error") errors.push(message.text());
  });

  await page.mouse.click(786, 30);
  await page.waitForTimeout(250);
  await page.mouse.click(808, 110);
  await page.waitForTimeout(500);
  await page.mouse.click(430, 216);
  await page.keyboard.type("capqa-clock");
  await page.mouse.click(674, 216);
  await page.waitForTimeout(750);
  await page.screenshot({ path: "D:/Desktop/Code/work-worldline/qa-capqa-stage1/evidence/browser-checkpoint-created.png" });

  const state = await page.evaluate(() => ({
    title: document.title,
    canvasCount: document.querySelectorAll("canvas").length,
    canvasSize: [...document.querySelectorAll("canvas")].map(({ width, height }) => ({ width, height })),
  }));
  if (state.canvasCount !== 1 || errors.length > 0) {
    throw new Error(`Web checkpoint creation failed: state=${JSON.stringify(state)} errors=${JSON.stringify(errors)}`);
  }
  return { state, errors };
}
