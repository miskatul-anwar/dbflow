/**
 * Starts an analysis run on the Rust engine.
 *
 *  - Desktop app (Tauri): the native engine runs on a background thread in the app process
 *    (`src-tauri`), with progress streamed over a channel and cancellation by flag.
 *  - Browser: the WebAssembly build runs in a Web Worker so the window stays responsive;
 *    `cancel()` terminates the worker. Without workers it runs on the main thread.
 */
import { invoke, Channel, isTauri } from '@tauri-apps/api/core';
import DatabaseService, {
  askNamer,
  engineOptions,
  prepareEngineFiles,
  type ProcessingResult,
  type ProcessingState,
} from './databaseService';
import type { EngineMessage, NameReply } from '../../workers/engine.worker';
import type { NamerFn } from '../naming/applyNames';

export interface EngineRun {
  promise: Promise<ProcessingResult>;
  cancel: () => void;
}

function nativeAvailable(): boolean {
  try {
    return isTauri();
  } catch {
    return false;
  }
}

function runNative(
  files: File[],
  operations: string[],
  onProgress: (state: ProcessingState) => void,
  options: Record<string, unknown>,
  namer: NamerFn | null | undefined,
): EngineRun {
  const runId = `${Date.now()}-${Math.random().toString(36).slice(2)}`;
  let cancelled = false;
  let rejectRun: ((e: Error) => void) | null = null;
  const channel = new Channel<ProcessingState>();
  channel.onmessage = p => {
    if (!cancelled) onProgress(p);
  };
  const promise = new Promise<ProcessingResult>((resolve, reject) => {
    rejectRun = reject;
    (async () => {
      const prepared = await prepareEngineFiles(files);
      const uploads: number[] = [];
      for (const f of prepared.files) {
        if (cancelled) throw new Error('Processing was cancelled.');
        uploads.push(
          await invoke<number>('engine_upload', f.bytes, { headers: { 'x-file-name': encodeURIComponent(f.name), 'x-run-id': runId } }),
        );
      }
      const run = await invoke<{ session: number; naming: Array<{ key: string; columns: string[] }> }>('engine_prepare', {
        runId,
        uploads,
        operations,
        options: engineOptions(options, prepared.notes),
        onProgress: channel,
      });
      if (cancelled) throw new Error('Processing was cancelled.');
      const { answers, error } = await askNamer(namer, run.naming, onProgress);
      return invoke<ProcessingResult>('engine_finish', { session: run.session, answers, namerError: error, onProgress: channel });
    })().then(resolve, reject);
  });
  return {
    promise,
    cancel: () => {
      cancelled = true;
      invoke('engine_cancel', { runId }).catch(() => undefined);
      rejectRun?.(new Error('Processing was cancelled.'));
    },
  };
}

export function runEngine(
  files: File[],
  operations: string[],
  onProgress: (state: ProcessingState) => void,
  allOptions: Record<string, unknown> = {},
): EngineRun {
  // The namer is a function (it talks to the naming worker), so it stays on this side of the boundary.
  const { namer, ...options } = allOptions as Record<string, unknown> & { namer?: NamerFn | null };
  if (nativeAvailable()) return runNative(files, operations, onProgress, options, namer);

  const inThread = (): EngineRun => ({
    promise: DatabaseService.processFiles(files, operations, onProgress, { ...options, namer }),
    cancel: () => DatabaseService.cancelProcessing(),
  });

  if (typeof Worker === 'undefined') return inThread();

  let worker: Worker;
  try {
    worker = new Worker(new URL('../../workers/engine.worker.ts', import.meta.url), { type: 'module' });
  } catch {
    return inThread();
  }

  let fallback: EngineRun | null = null;
  let settle: { resolve: (r: ProcessingResult) => void; reject: (e: Error) => void } | null = null;
  const promise = new Promise<ProcessingResult>((resolve, reject) => {
    settle = { resolve, reject };
  });

  worker.onmessage = (event: MessageEvent<EngineMessage>) => {
    const msg = event.data;
    if (msg.type === 'progress') onProgress(msg.progress as ProcessingState);
    else if (msg.type === 'name-request') {
      const reply = (r: NameReply) => worker.postMessage(r);
      if (!namer) reply({ type: 'names-error', id: msg.id, message: 'no naming model' });
      else
        namer(msg.items, (done, total) => {
          reply({ type: 'names-progress', id: msg.id, done, total });
          onProgress({ phase: 'NORMAL', operation: 'naming', progress: 88, message: `Naming relations ${done}/${total}…` });
        }).then(
          names => reply({ type: 'names', id: msg.id, names }),
          e => reply({ type: 'names-error', id: msg.id, message: (e as Error)?.message ?? String(e) }),
        );
    } else if (msg.type === 'done') {
      worker.terminate();
      settle!.resolve(msg.result as ProcessingResult);
    } else if (msg.type === 'error') {
      worker.terminate();
      settle!.reject(new Error(msg.message));
    } else if (msg.type === 'clone-error') {
      worker.terminate();
      fallback = inThread();
      fallback.promise.then(settle!.resolve, settle!.reject);
    }
  };
  worker.onerror = event => {
    worker.terminate();
    fallback = inThread();
    fallback.promise.then(settle!.resolve, settle!.reject);
    event.preventDefault();
  };

  worker.postMessage({ files, operations, options, canName: !!namer });

  return {
    promise,
    cancel: () => {
      if (fallback) fallback.cancel();
      else worker.terminate();
      settle?.reject(new Error('Processing was cancelled.'));
    },
  };
}
