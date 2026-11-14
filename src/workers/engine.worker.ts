/// <reference lib="webworker" />
/**
 * Runs the Rust engine (WebAssembly build) off the UI thread. The page sends the files and the
 * selected operations; progress and the final result come back as messages. Cancelling is done by
 * terminating this worker from the page. The desktop app uses the native engine instead.
 */
import { initEngine, enginePrepare, engineFinish } from '../engine';
import { prepareEngineFiles, engineOptions } from '../services/api/databaseService';

export type EngineRequest = {
  files: File[];
  operations: string[];
  options: Record<string, unknown>;
  /** The page can write relation names (it hosts the naming model); the worker asks it through `name-request`. */
  canName?: boolean;
};

/** Page -> worker, answering a `name-request`. */
export type NameReply =
  | { type: 'names-progress'; id: number; done: number; total: number }
  | { type: 'names'; id: number; names: Record<string, string> }
  | { type: 'names-error'; id: number; message: string };

export type EngineMessage =
  | { type: 'progress'; progress: unknown }
  | { type: 'done'; result: unknown }
  | { type: 'name-request'; id: number; items: Array<{ key: string; columns: string[] }> }
  | { type: 'error'; message: string }
  /** The result could not be copied to the page; the page re-runs on the main thread. */
  | { type: 'clone-error' };

const ctx = self as unknown as DedicatedWorkerGlobalScope;

let nextNameId = 1;
const waiting = new Map<number, { resolve: (n: Record<string, string>) => void; reject: (e: Error) => void }>();

function askPage(items: Array<{ key: string; columns: string[] }>): Promise<Record<string, string>> {
  return new Promise((resolve, reject) => {
    const id = nextNameId++;
    waiting.set(id, { resolve, reject });
    ctx.postMessage({ type: 'name-request', id, items } satisfies EngineMessage);
  });
}

ctx.onmessage = async (event: MessageEvent<EngineRequest | NameReply>) => {
  const data = event.data;
  if ('type' in data) {
    const w = waiting.get(data.id);
    if (!w || data.type === 'names-progress') return;
    waiting.delete(data.id);
    if (data.type === 'names') w.resolve(data.names);
    else w.reject(new Error(data.message));
    return;
  }
  const { files, operations, options, canName } = data;
  const progress = (p: unknown) => ctx.postMessage({ type: 'progress', progress: p } satisfies EngineMessage);
  try {
    await initEngine();
    const prepared = await prepareEngineFiles(files);
    const run = enginePrepare(prepared.files, operations, engineOptions(options, prepared.notes), progress);
    let answers: Record<string, string> | null = null;
    let namerError: string | null = null;
    if (canName && run.naming.length > 0) {
      try {
        answers = await askPage(run.naming);
      } catch (e) {
        namerError = (e as Error)?.message ?? String(e);
      }
    }
    const result = engineFinish(run.session, answers, namerError, progress);
    try {
      ctx.postMessage({ type: 'done', result } satisfies EngineMessage);
    } catch {
      ctx.postMessage({ type: 'clone-error' } satisfies EngineMessage);
    }
  } catch (err) {
    ctx.postMessage({ type: 'error', message: (err as Error)?.message ?? String(err) } satisfies EngineMessage);
  }
};
