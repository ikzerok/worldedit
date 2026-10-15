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
// Default-language source: no manifest upgrade, extra compiler, mocked catalog, or
// host-side state filtering. The owner selector is interpreted inside final WASM.
const ownerDraft = `world shared as "同名"
character shared as "同名"
character other as "同名"
character empty as "无状态人物"
tag calm as "平静"
tag alert as "警觉"
tag proved as "已验证"
state person_mood on character shared with calm as "状态"
state person_empty on character shared with [] as "状态"
state other_mood on character other with calm as "状态"
state world_mood on world shared with [] as "状态"
let shared_score = 3
let shared_mood = false
event start with shared
  effect on enter
    become person_mood with alert
  choice "继续 {rnd(1, 9)}" if has(person_mood, alert)
    become person_mood with alert
    become person_empty with proved
    choice "结束"
      -> END
`;
const sharedOwner = { kind: 'character', id: 'shared' };
const ownerQuery = (owner = sharedOwner, extra = {}) => ({ owner, group: 'state', ...extra });
const row = (page, name) => {
  const item = page.items.find(item => item.key.name === name);
  assert(item, `Missing real state row: ${name}`);
  return item;
};
function assertTags(cell, expected) {
  assert.equal(cell.status, 'present');
  assert.deepEqual(cell.value, { TagSet: expected });
  assert.equal(cell.truncated, false);
}
function persistentRun(view) {
  // outputs is the per-command delivery batch, which commands intentionally clear.
  // Raw save/trace/RNG internals are not exposed by this Worker View protocol.
  const { outputs, inspection, ...persistent } = view;
  return persistent;
}
function observedRun(view) {
  return { ...persistentRun(view), stamp: view.inspection.stamp };
}
function assertUnscopedMain(page) {
  assert(!Object.hasOwn(page, 'owner'), 'Main inspection cannot inherit a hidden side owner');
  assert(page.items.every(item => !Object.hasOwn(item, 'state_definition')));
  for (const [group, name] of [['global', 'shared_score'], ['global', 'shared_mood'],
    ['state', 'person_mood'], ['state', 'person_empty'], ['state', 'other_mood'], ['state', 'world_mood']]) {
    assert(page.items.some(item => item.key.group === group && item.key.name === name),
      `Main inspection must still include ${group}:${name}`);
  }
}
function assertReadOnly(result, before) {
  assert.deepEqual(observedRun(result.view), before,
    'Inspection must preserve the exposed run identity, choices/conditions, turns, and wait');
}
worker = await createWorker();
try {
  const session = 'owner-frozen-final-wasm';
  const payload = prepareFor(session, ownerDraft);
  let result = resultFor(await ask(worker, {
    kind: 'prepare', json: JSON.stringify(payload), files, retained: []
  }), session, 0);
  assert(!result.error, result.error);
  let request = 1;
  const issue = async action => {
    const id = request++;
    return resultFor(await ask(worker, command(id, action, session)), session, id);
  };
  result = await issue({ action: 'start', seed: 9, budget });
  assert(!result.error, result.error);
  assert.equal(result.view.outcome, 'choice');
  const before = observedRun(result.view);
  const mainPage = result.view.inspection;
  assertUnscopedMain(mainPage);
  const firstStamp = before.stamp;
  for (const field of ['run_id', 'compiled_snapshot', 'fingerprint', 'trace_generation', 'revision']) {
    assert.match(firstStamp[field], /^\d+$/, `${field} must keep its decimal-string wire identity`);
  }
  assert(!Object.hasOwn(result.view.inspection, 'owner'), 'Legacy query omits owner metadata');
  assert(result.view.inspection.items.every(item => !Object.hasOwn(item, 'state_definition')),
    'Legacy query does not invent owner definitions for ordinary variables');
  const inspect = async query => {
    const response = await issue({ action: 'inspect_object', query });
    assert(!response.error, response.error);
    assertReadOnly(response, before);
    return response;
  };
  for (const [owner, expected] of [
    [sharedOwner, ['person_empty', 'person_mood']],
    [{ kind: 'character', id: 'other' }, ['other_mood']],
    [{ kind: 'world', id: 'shared' }, ['world_mood']]
  ]) {
    result = await inspect(ownerQuery(owner, { expected_stamp: firstStamp }));
    const page = result.view.inspection;
    assert.deepEqual(page.owner, { target: owner, display: '同名' });
    assert.equal(page.total_items, expected.length);
    assert.equal(page.total_matches, expected.length);
    assert.deepEqual(page.items.map(item => item.key.name), expected);
    for (const item of page.items) {
      assert.equal(item.key.group, 'state');
      assert.deepEqual(item.state_definition.owner, owner);
      assert.equal(item.state_definition.display, '状态');
      assert(item.source, 'Real owner state must retain its declaration source');
    }
  }
  result = await inspect(ownerQuery(sharedOwner, { expected_stamp: firstStamp }));
  const frozenPage = result.view.inspection;
  assert.equal(frozenPage.first_observation, 1);
  assert.equal(frozenPage.previous_observation, null);
  assert.equal(frozenPage.current_observation, 1);
  const mood = row(frozenPage, 'person_mood');
  assertTags(mood.state_definition.initial, ['calm']);
  assertTags(mood.first, ['alert']);
  assertTags(mood.current, ['alert']);
  assert.equal(mood.previous.status, 'unrecorded');
  assert.equal(mood.previous.value, null);
  assert.notDeepEqual(mood.first, mood.state_definition.initial,
    'First real observation follows enter effects; it is not the declaration initial value');
  assertTags(row(frozenPage, 'person_empty').state_definition.initial, []);
  assertTags(row(frozenPage, 'person_empty').current, []);
  // The old generic inspect owner API remains compatible, but it is not the UI
  // side-channel. Explicitly restore the ordinary query before checking isolation.
  result = await issue({ action: 'inspect', query: ownerQuery(sharedOwner, { expected_stamp: firstStamp }) });
  assert(!result.error, result.error);
  assert.deepEqual(result.view.inspection, frozenPage);
  result = await issue({ action: 'inspect', query: { owner: null, expected_stamp: firstStamp } });
  assert(!result.error, result.error);
  assertReadOnly(result, before);
  assertUnscopedMain(result.view.inspection);
  assert.deepEqual(result.view.inspection, mainPage);
  for (let repeat = 0; repeat < 3; repeat++) {
    result = await inspect(ownerQuery(sharedOwner, { expected_stamp: firstStamp }));
    assert.deepEqual(result.view.inspection, frozenPage, 'A read cannot manufacture another observation');
  }
  const firstPage = (await inspect(ownerQuery(sharedOwner,
    { expected_stamp: firstStamp, limit: 1 }))).view.inspection;
  const secondPage = (await inspect(ownerQuery(sharedOwner,
    { expected_stamp: firstStamp, offset: 1, limit: 1 }))).view.inspection;
  assert.equal(firstPage.next_offset, 1);
  assert.equal(secondPage.next_offset, null);
  assert.equal(firstPage.total_matches, 2);
  assert.equal(secondPage.total_matches, 2);
  assert.deepEqual([...firstPage.items, ...secondPage.items], frozenPage.items);
  result = await inspect(ownerQuery(sharedOwner, { expected_stamp: firstStamp, text: 'PERSON_MOOD' }));
  assert.deepEqual(result.view.inspection.items.map(item => item.key.name), ['person_mood']);
  result = await inspect(ownerQuery(sharedOwner, { expected_stamp: firstStamp, text: '状态' }));
  assert.deepEqual(result.view.inspection.items, frozenPage.items);
  const emptyOwner = { kind: 'character', id: 'empty' };
  result = await inspect(ownerQuery(emptyOwner, { expected_stamp: firstStamp }));
  assert.deepEqual(result.view.inspection.owner, { target: emptyOwner, display: '无状态人物' });
  assert.equal(result.view.inspection.total_items, 0);
  assert.equal(result.view.inspection.total_matches, 0);
  assert.deepEqual(result.view.inspection.items, []);
  const emptyView = result.view;
  for (const query of [{}, { owner: null }]) {
    result = await issue({ action: 'inspect_object', query });
    assert.match(result.error ?? '', /必须提供完整 owner/);
    assert.deepEqual(result.view, emptyView, 'Missing side owner retains the last valid side page');
  }
  for (const owner of [{ kind: 'entity', id: 'shared' }, { kind: 'character', id: 'missing' }]) {
    result = await issue({ action: 'inspect_object', query: ownerQuery(owner, { expected_stamp: firstStamp }) });
    assert.match(result.error ?? '', /冻结目录/, 'A wrong full identity is an error, not an empty object');
    assert.deepEqual(result.view, emptyView, 'Rejected owner must retain the last valid query and run');
  }
  for (const query of [
    ownerQuery({ kind: 'character', id: '' }),
    ownerQuery({ kind: 'unknown-kind', id: 'shared' }),
    ownerQuery(sharedOwner, { group: 'global' })
  ]) {
    result = await issue({ action: 'inspect_object', query });
    assert.match(result.error ?? '', /完整已知种类/, 'Invalid owner/group must not silently fall back');
    assert.deepEqual(result.view, emptyView);
  }
  for (const owner of [{ id: 'shared' }, { kind: 'character' },
    { ...sharedOwner, display: '同名' }, { kind: 'character', id: 7 }]) {
    const rejected = await ask(worker, command(request, { action: 'inspect_object', query: { owner } }, session));
    assert(rejected.error, 'Malformed owner must fail strict wire decoding');
    assert(!rejected.result, 'Malformed owner must not become a fabricated successful response');
    // Decode failures never entered Engine::execute: the same request identity is still usable.
    result = await inspect(ownerQuery(emptyOwner, { expected_stamp: firstStamp }));
    assert.deepEqual(result.view, emptyView);
  }

  // A separately prepared newer draft changes owner display, ownership, and static
  // initial tags. It must not backfill the still-live old Worker's frozen catalog.
  const changedDraft = ownerDraft
    .replace('character shared as "同名"', 'character shared as "新稿人物"')
    .replace('state person_mood on character shared with calm',
      'state person_mood on character other with proved');
  payload.input.drafts[0].source = changedDraft;
  const newer = await createWorker();
  try {
    const newerSession = 'owner-newer-final-wasm';
    let nextResult = resultFor(await ask(newer, {
      kind: 'prepare', json: JSON.stringify({ ...payload, session_id: newerSession }), files, retained: []
    }), newerSession, 0);
    assert(!nextResult.error, nextResult.error);
    nextResult = resultFor(await ask(newer,
      command(1, { action: 'start', seed: 9, budget }, newerSession)), newerSession, 1);
    assert(!nextResult.error, nextResult.error);
    const newerBefore = observedRun(nextResult.view);
    nextResult = resultFor(await ask(newer,
      command(2, { action: 'inspect_object', query: ownerQuery() }, newerSession)), newerSession, 2);
    assert(!nextResult.error, nextResult.error);
    assertReadOnly(nextResult, newerBefore);
    assert.equal(nextResult.view.inspection.owner.display, '新稿人物');
    assert.deepEqual(nextResult.view.inspection.items.map(item => item.key.name), ['person_empty']);
    const newerView = nextResult.view;
    assert.notDeepEqual(newerView.inspection.stamp, firstStamp);
    nextResult = resultFor(await ask(newer, command(3, {
      action: 'inspect_object', query: ownerQuery(sharedOwner, { expected_stamp: firstStamp })
    }, newerSession)), newerSession, 3);
    assert.match(nextResult.error ?? '', /运行或观察位置已变化/);
    assert.deepEqual(nextResult.view, newerView, 'Foreign snapshot stamp must not replace a valid page');
    nextResult = resultFor(await ask(newer, command(4, {
      action: 'inspect_object', query: ownerQuery({ kind: 'character', id: 'other' })
    }, newerSession)), newerSession, 4);
    assert(!nextResult.error, nextResult.error);
    assertReadOnly(nextResult, newerBefore);
    assertTags(row(nextResult.view.inspection, 'person_mood').state_definition.initial, ['proved']);
    result = await inspect(ownerQuery(sharedOwner, { expected_stamp: firstStamp }));
    assert.deepEqual(result.view.inspection, frozenPage, 'Newer draft cannot rewrite old owner/source/initial');
    result = await issue({ action: 'declaration_source', source: mood.source });
    assert(!result.error, result.error);
    assertReadOnly(result, before);
    assert.equal(result.source.path, 'world.wl');
    assert.equal(result.source.draft, true);
    assert.equal(result.source.preview, 'state person_mood on character shared with calm as "状态"');
    assertUnscopedMain(result.view.inspection);
    assert.deepEqual(result.view.inspection, mainPage,
      'Side-source navigation returns the unchanged ordinary inspection, not the owner page');
    result = await issue({ action: 'inspect', query: { group: 'global', expected_stamp: firstStamp } });
    assert(!result.error, result.error);
    assertReadOnly(result, before);
    assert.deepEqual(result.view.inspection.items.map(item => item.key.name), ['shared_mood', 'shared_score']);
    result = await issue({ action: 'inspect', query: { owner: null, expected_stamp: firstStamp } });
    assert(!result.error, result.error);
    assert.deepEqual(result.view.inspection, mainPage);
  } finally { await newer.terminate(); }

  const choice = result.view.choices[0].id;
  result = await issue({ action: 'choose', id: choice, budget });
  assert(!result.error, result.error);
  assert.equal(result.view.outcome, 'choice');
  const advanced = observedRun(result.view);
  const advancedView = result.view;
  assertUnscopedMain(advancedView.inspection);
  assert.equal(advanced.stamp.run_id, firstStamp.run_id);
  assert.notDeepEqual(advanced.stamp, firstStamp);
  assert.equal(advancedView.inspection.previous_observation, 1);
  assert.equal(advancedView.inspection.current_observation, 2);
  assertTags(row(advancedView.inspection, 'person_empty').current, ['proved']);
  assert.equal(row(advancedView.inspection, 'person_empty').previous_change, 'changed');
  assert.equal(row(advancedView.inspection, 'person_mood').previous_change, 'unchanged');
  for (const owner of [sharedOwner, { kind: 'character', id: 'missing' }]) {
    result = await issue({ action: 'inspect_object', query: ownerQuery(owner, { expected_stamp: firstStamp }) });
    assert.match(result.error ?? '', /运行或观察位置已变化/,
      'Old observation stamp must fail before a different missing-owner lookup');
    assert.deepEqual(persistentRun(result.view), persistentRun(advancedView));
    assert.deepEqual(result.view.inspection, frozenPage,
      'A failed stale side request keeps its old side page, never the fresh main page');
  }
  result = await issue({ action: 'inspect_object', query: ownerQuery(sharedOwner,
    { expected_stamp: advanced.stamp, changed_only: true, compare_to: 'previous' }) });
  assert(!result.error, result.error);
  assertReadOnly(result, advanced);
  assert.equal(result.view.inspection.total_items, 2);
  assert.deepEqual(result.view.inspection.items.map(item => item.key.name), ['person_empty']);
  assertTags(result.view.inspection.items[0].state_definition.initial, []);
  assertTags(result.view.inspection.items[0].first, []);
  assertTags(result.view.inspection.items[0].previous, []);
  assertTags(result.view.inspection.items[0].current, ['proved']);
  const sideSource = result.view.inspection.items[0].source;
  result = await issue({ action: 'continue', budget });
  assert(!result.error, result.error);
  assertUnscopedMain(result.view.inspection);
  const endChoice = result.view.choices[0].id;
  result = await issue({ action: 'choose', id: endChoice, budget });
  assert(!result.error, result.error);
  assert.equal(result.view.ended, true);
  assertUnscopedMain(result.view.inspection);
  const ended = observedRun(result.view);
  // Restrict the main query explicitly, so only the now-stale side cache could
  // authorize this source. A stale side page must not grant a fresh source jump.
  result = await issue({ action: 'inspect', query: { group: 'global', expected_stamp: ended.stamp } });
  assert(!result.error, result.error);
  const endedMain = result.view;
  result = await issue({ action: 'declaration_source', source: sideSource });
  assert.match(result.error ?? '', /运行或观察位置已变化/);
  assert.equal(result.source, null);
  assert.deepEqual(result.view, endedMain);
  result = await issue({ action: 'inspect', query: { owner: null, expected_stamp: ended.stamp } });
  assert(!result.error, result.error);
  assertReadOnly(result, ended);
  assertUnscopedMain(result.view.inspection);
} finally { await worker.terminate(); }
console.log('PASS: final WASM prepare/start/actual output/choose/state/source/identity rejection/terminate/reopen');
console.log('PASS: real Worker queued prepare -> terminate/join -> fresh session; no late old result or old-session advancement');
console.log('PASS: final WASM unknown choice retains wait; actual choice -> division-by-zero clears outcome and reports failed inspection');
console.log('PASS: final WASM owner full kind/id, same display names, multiple states/empty owner, strict malformed/wrong-owner rejection, legacy query compatibility');
console.log('PASS: final WASM frozen draft catalog/source, static initial differs from first observation, read-only exposed run fields and same-stamp pagination/filtering, stale snapshot/observation rejection');
console.log('PASS: independent inspect_object side pages preserve generic inspect globals/other owners and continue/choose/source main responses; stale side sources rejected');
console.log('NOT COVERED: browser host/HTTP loader, browser interaction/physical IME, CORS/CSP, browser host response callbacks, interruption inside Rust compilation, or direct raw save/trace/RNG memory assertions');
