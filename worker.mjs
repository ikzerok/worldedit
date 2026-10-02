// Only the editor's same-origin static JS/WASM may be loaded. No author code runs here.
self.onmessage = async ({ data }) => {
  let request;
  try {
    request = JSON.parse(data.request_json);
    const envelope = {
      job_id: request.job_id,
      generation: String(request.generation),
      baseline: request.baseline,
    };
    const moduleUrl = new URL(data.module_url, self.location.href);
    const wasmUrl = new URL(data.wasm_url, self.location.href);
    if (moduleUrl.origin !== self.location.origin || wasmUrl.origin !== self.location.origin
        || !['http:', 'https:'].includes(moduleUrl.protocol)
        || !['http:', 'https:'].includes(wasmUrl.protocol)) {
      throw new Error('后台资源必须同源');
    }
    self.postMessage({ ...envelope, event: 'progress', stage: '初始化', completed: 0, total: 1 });
    const module = await import(moduleUrl.href);
    await module.default({ module_or_path: wasmUrl.href });
    const result = module.worker_execute(data.request_json, data.files, data.retained);
    self.postMessage({ ...envelope, event: 'done', ...result }, result.binaries.map(bytes => bytes.buffer));
  } catch (error) {
    self.postMessage({
      job_id: request?.job_id ?? '', generation: String(request?.generation ?? 0),
      baseline: request?.baseline ?? '', event: 'error', message: String(error),
    });
  }
};
