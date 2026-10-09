// Usage: node scripts/test-draft-rehearsal-worker.mjs /absolute/path/to/final-web-dist
// Runs the final packaged WASM in genuine Node Worker threads. This does not claim
// browser same-origin loading, browser interaction, IME, CORS, or cross-origin isolation QA.
import assert from 'node:assert/strict';
import { readdir } from 'node:fs/promises';
import { resolve, join } from 'node:path';
import { pathToFileURL } from 'node:url';
import { Worker } from 'node:worker_threads';
const directory = resolve(process.argv[2] ?? 'dist');
const names = await readdir(directory);
const wasm = names.find(name => name.endsWith('_bg.wasm'));
const js = names.find(name => name.endsWith('.js') && name !== 'worker.mjs' && name !== 'rehearsal-worker.mjs');
assert(wasm && js, 'Final distribution must contain wasm-bindgen JS and WASM');
const workerSource = `
import { parentPort, workerData } from 'node:worker_threads';
import { readFile } from 'node:fs/promises';
globalThis.self = globalThis;
const api = await import(workerData.module);
await api.default({ module_or_path: await readFile(workerData.wasm) });
parentPort.postMessage({ ready: true, exports: ['rehearsal_worker_prepare', 'rehearsal_worker_command'].map(k => typeof api[k]) });
let queuedPrepare;
parentPort.on('message', request => {
  try {
    // Test-only dispatch barrier: keep a real prepare payload queued after the final
    // WASM is loaded. This proves queued cancellation, not interruption inside Rust.
    if (workerData.holdPrepare && request.kind === 'prepare') {
      if (queuedPrepare) throw new Error('Only one prepare may wait at the test barrier');
      queuedPrepare = request;
      const { session_id, request_id } = JSON.parse(request.json);
      parentPort.postMessage({ queued_prepare: { session_id, request_id } });
      return;
    }
    if (request.kind === 'release_test_queue') {
      if (!queuedPrepare) throw new Error('No queued prepare to release');
      request = queuedPrepare;
      queuedPrepare = undefined;
    }
    const result = request.kind === 'prepare'
      ? api.rehearsal_worker_prepare(request.json, request.files, request.retained)
      : api.rehearsal_worker_command(request.json);
    parentPort.postMessage({ result: JSON.parse(result) });
  } catch (error) { parentPort.postMessage({ error: String(error) }); }
});`;
async function createWorker(holdPrepare = false) {
  const worker = new Worker(new URL(`data:text/javascript,${encodeURIComponent(workerSource)}`), {
    workerData: { module: pathToFileURL(join(directory, js)).href, wasm: join(directory, wasm), holdPrepare }
  });
  try {
    const ready = await next(worker);
    assert.deepEqual(ready.exports, ['function', 'function']);
    return worker;
  } catch (error) {
    await worker.terminate();
    throw error;
  }
}
function next(worker) {
  return new Promise((yes, no) => {
    const timer = setTimeout(() => { cleanup(); no(new Error('Worker did not respond in 30 seconds')); }, 30_000);
    const message = value => { cleanup(); yes(value); };
    const error = value => { cleanup(); no(value); };
    const exited = code => { cleanup(); no(new Error('Worker exited before response: ' + code)); };
    const cleanup = () => {
      clearTimeout(timer);
      worker.off('message', message); worker.off('error', error); worker.off('exit', exited);
    };
    worker.once('message', message); worker.once('error', error); worker.once('exit', exited);
  });
}
async function ask(worker, value) { const response = next(worker); worker.postMessage(value); return response; }
const original = 'let n = 0\nevent start\n  applied source\n  -> END\n';
const draft = 'let n = 0\nevent start\n  draft actual output\n  choice "continue"\n    set n = 7\n    -> END\n';
const baseline = (() => {
  let hash = 0xcbf29ce484222325n;
  const mix = value => {
    const bytes = typeof value === 'string' ? Buffer.from(value) : value;
    const size = Buffer.alloc(8); size.writeBigUInt64LE(BigInt(bytes.length));
    for (const byte of Buffer.concat([size, bytes])) { hash ^= BigInt(byte); hash = BigInt.asUintN(64, hash * 0x100000001b3n); }
  };
  mix('worldline-content-v1'); mix('world.wl'); mix('source'); mix('world.wl'); mix(Buffer.from([0])); mix(original);
  return hash.toString(16).padStart(16, '0');
})();
const prepare = {
  schema_version: 1, session_id: 'node-final-wasm', request_id: '0', entry: 'world.wl',
  snapshot_state: { schema_version: 1, documents: [{ path: 'world.wl', authoring: false, deleted: false, read_only: false }] },
  input: { schema_version: 1, content_baseline: baseline,
    drafts: [{ path: 'world.wl', original_source: original, source: draft, generation: 1 }],
    excluded_inputs: [{ kind: 'form', source: 'excluded pending form' }], composing: false }
};
const files = [{ path: 'world.wl', bytes: new Uint8Array(Buffer.from(original)) }];
const command = (request_id, action, session_id = prepare.session_id) => ({ kind: 'command', json: JSON.stringify({ schema_version: 1, session_id, request_id: String(request_id), action }) });
const budget = { max_steps: 1000, time_budget_ms: 250 };
function prepareFor(session_id, source = draft) {
  return { ...prepare, session_id, input: { ...prepare.input,
    drafts: [{ ...prepare.input.drafts[0], source }] } };
}
function resultFor(response, session_id, request_id) {
  assert(!response.error, response.error);
  assert.equal(response.result.schema_version, 1);
  assert.equal(response.result.session_id, session_id, 'Do not accept another session result');
  assert.equal(response.result.request_id, String(request_id), 'Do not accept an old request result');
  return response.result;
}
let worker = await createWorker();
try {
  let response = await ask(worker, { kind: 'prepare', json: JSON.stringify(prepare), files, retained: [] });
  assert(!response.error, response.error);
  assert.equal(response.result.view.seed, null);
  assert.equal(response.result.view.scope.sources[0].kind, 'writing_draft');
  assert.equal(response.result.view.scope.excluded_inputs.length, 1);
  response = await ask(worker, command(1, { action: 'start', seed: 9, budget }));
  assert(!response.result.error, response.result.error);
  assert.equal(response.result.view.outputs[0].content, 'draft actual output');
  const runId = response.result.view.inspection.stamp.run_id;
  const choice = response.result.view.choices[0].id;
  const declaration = response.result.view.inspection.items[0].source;
  response = await ask(worker, command(2, { action: 'declaration_source', source: declaration }));
  assert.equal(response.result.source.path, 'world.wl');
  assert.equal(response.result.source.preview, 'let n = 0');
  response = await ask(worker, command(3, { action: 'choose', id: choice, budget }));
  assert.equal(response.result.view.ended, true);
  assert.equal(response.result.view.inspection.stamp.run_id, runId);
  assert.equal(response.result.view.inspection.items[0].current.display, '7');
  response = await ask(worker, command(3, { action: 'continue', budget }));
  assert(response.result.error, 'Duplicate request must fail');
  response = await ask(worker, command(4, { action: 'inspect', query: {} }, 'foreign-session'));
  assert(response.result.error, 'Foreign session must fail');
} finally { await worker.terminate(); }
worker = await createWorker();
try {
  const fresh = { ...prepare, session_id: 'reopened-final-wasm' };
  const response = await ask(worker, { kind: 'prepare', json: JSON.stringify(fresh), files, retained: [] });
  assert(!response.error, response.error);
  assert.equal(response.result.view.seed, null);
  assert.deepEqual(response.result.view.outputs, []);
} finally { await worker.terminate(); }

// Actual worker termination while an observed prepare is still queued. The barrier
// avoids a timing race or a fabricated slow compiler and never executes author text as JS.
const cancelledSession = 'cancelled-queued-final-wasm';
const queuedWorker = await createWorker(true);
const oldMessages = [];
queuedWorker.on('message', message => oldMessages.push(message));
try {
  const queued = await ask(queuedWorker, {
    kind: 'prepare', json: JSON.stringify(prepareFor(cancelledSession)), files, retained: []
  });
  assert.deepEqual(queued.queued_prepare, { session_id: cancelledSession, request_id: '0' });
  assert.equal(oldMessages.length, 1);
  assert(!oldMessages.some(message => message.result), 'Queued prepare must not have completed');
  assert.equal(await queuedWorker.terminate(), 1, 'Termination must join the live worker');
  assert.equal(queuedWorker.threadId, -1, 'Old worker must actually be gone before reopening');
} finally {
  if (queuedWorker.threadId !== -1) await queuedWorker.terminate();
}

worker = await createWorker();
try {
  const session = 'reopened-after-queued-cancel-final-wasm';
  const fresh = prepareFor(session, draft.replace('draft actual output', 'fresh after queued cancellation'));
  let result = resultFor(await ask(worker, {
    kind: 'prepare', json: JSON.stringify(fresh), files, retained: []
  }), session, 0);
  assert.equal(result.view.seed, null);
  assert.deepEqual(result.view.outputs, []);
  result = resultFor(await ask(worker, command(1, { action: 'start', seed: 9, budget }, session)), session, 1);
  assert(!result.error, result.error);
  assert.equal(result.view.outputs[0].content, 'fresh after queued cancellation');
  const before = result.view.inspection;
  const choice = result.view.choices[0].id;
  result = resultFor(await ask(worker, command(2, { action: 'choose', id: choice, budget }, cancelledSession)), session, 2);
  assert(result.error, 'Old session command must not be accepted by the reopened WASM engine');
  assert.equal(result.view.outcome, 'choice');
  assert.deepEqual(result.view.inspection, before);
  assert.equal(result.view.turns, 0);
  result = resultFor(await ask(worker, command(2, { action: 'choose', id: choice, budget }, session)), session, 2);
  assert(!result.error, result.error);
  assert.equal(result.view.ended, true);
  assert.equal(result.view.inspection.items[0].current.display, '7');
  assert.equal(oldMessages.length, 1, 'Terminated old worker must not deliver a late result');
} finally { await worker.terminate(); }

worker = await createWorker();
try {
  const session = 'actual-error-final-wasm';
  const failing = prepareFor(session,
    'let divisor = 0\nevent start\n  before failure\n  choice "divide by zero"\n    result {1 / divisor}\n    -> END\n');
  let result = resultFor(await ask(worker, {
    kind: 'prepare', json: JSON.stringify(failing), files, retained: []
  }), session, 0);
  assert(!result.error, result.error);
  result = resultFor(await ask(worker, command(1, { action: 'start', seed: 9, budget }, session)), session, 1);
  assert(!result.error, result.error);
  assert.equal(result.view.outcome, 'choice');
  assert.equal(result.view.inspection.status, 'choice');
  const stamp = result.view.inspection.stamp;
  const actualChoice = result.view.choices[0].id;
  result = resultFor(await ask(worker, command(2, { action: 'choose', id: 'unknown-choice', budget }, session)), session, 2);
  assert(result.error, 'Unknown choice must be rejected without changing the real wait');
  assert.equal(result.view.error, null);
  assert.equal(result.view.outcome, 'choice');
  assert.deepEqual(result.view.inspection.stamp, stamp);
  result = resultFor(await ask(worker, command(3, { action: 'choose', id: actualChoice, budget }, session)), session, 3);
  assert(result.error?.includes('除以零'), result.error);
  assert(result.view.error?.includes('除以零'), result.view.error);
  assert.equal(result.view.outcome, null, 'Actual runtime failure must clear the old choice outcome');
  assert.equal(result.view.inspection.status, 'failed');
  assert.equal(result.view.inspection.stamp.run_id, stamp.run_id);
  assert.equal(result.view.ended, false);
} finally { await worker.terminate(); }
console.log('PASS: final WASM prepare/start/actual output/choose/state/source/identity rejection/terminate/reopen');
console.log('PASS: real Worker queued prepare -> terminate/join -> fresh session; no late old result or old-session advancement');
console.log('PASS: final WASM unknown choice retains wait; actual choice -> division-by-zero clears outcome and reports failed inspection');
console.log('NOT COVERED: browser host/HTTP loader, browser interaction/physical IME, CORS/CSP, browser host response callbacks, or interruption inside Rust compilation');
