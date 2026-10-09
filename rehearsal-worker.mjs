// One same-origin Rust runtime per rehearsal. Author source is never executed as JS.
let module;
let queue = Promise.resolve();
self.onmessage = ({ data }) => {
  queue = queue.then(async () => {
    try {
      let response;
      if (typeof data.prepare_json === 'string') {
        if (module) throw new Error('此 Worker 已经准备试演');
        const moduleUrl = new URL(data.module_url, self.location.href);
        const wasmUrl = new URL(data.wasm_url, self.location.href);
        for (const url of [moduleUrl, wasmUrl]) {
          if (url.origin !== self.location.origin || !['http:', 'https:'].includes(url.protocol)) {
            throw new Error('试演后台资源必须同源');
          }
        }
        module = await import(moduleUrl.href);
        await module.default({ module_or_path: wasmUrl.href });
        response = module.rehearsal_worker_prepare(data.prepare_json, data.files, data.retained);
      } else if (typeof data.command_json === 'string' && module) {
        response = module.rehearsal_worker_command(data.command_json);
      } else {
        throw new Error('试演请求缺少准备或命令');
      }
      self.postMessage({ response_json: response });
    } catch (error) {
      self.postMessage({ error: String(error) });
    }
  });
};
