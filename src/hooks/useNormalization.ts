import { useState, useCallback, useEffect, useRef } from 'react';
import { runEngine, type EngineRun } from '../services/api/engineClient';
import { createWorkerNamer } from '../services/naming/workerNamer';
import { loadUserNames, modelNamingEnabled } from '../services/naming/userNames';
import DatabaseService, { ProcessingOptions, ProcessingResult, ProcessingState } from '../services/api/databaseService';

export interface UseNormalizationReturn {
  isProcessing: boolean;
  progress: ProcessingState | null;
  results: ProcessingResult | null;
  error: string | null;
  processFile: (file: File, operations: string[]) => Promise<ProcessingResult>;
  processFiles: (files: File[], operations: string[], options?: ProcessingOptions) => Promise<ProcessingResult>;
  cancelProcessing: () => void;
  downloadFile: (file: any) => Promise<void>;
  reset: () => void;
}

export const useNormalization = (): UseNormalizationReturn => {
  const [isProcessing, setIsProcessing] = useState(false);
  const [progress, setProgress] = useState<ProcessingState | null>(null);
  const [results, setResults] = useState<ProcessingResult | null>(null);
  const [error, setError] = useState<string | null>(null);
  const runRef = useRef<EngineRun | null>(null);

  // Progress can arrive many times a second; the UI takes at most one update per frame, so a long run
  // never keeps the page busy re-rendering (the tab strip, dialogs and other workspaces stay responsive).
  const pendingProgress = useRef<ProcessingState | null>(null);
  const frame = useRef<number | null>(null);
  const reportProgress = useCallback((p: ProcessingState) => {
    pendingProgress.current = p;
    if (frame.current !== null) return;
    const flush = () => {
      frame.current = null;
      setProgress(pendingProgress.current);
    };
    frame.current = typeof requestAnimationFrame === 'function' ? requestAnimationFrame(flush) : (setTimeout(flush, 16) as unknown as number);
  }, []);
  const dropProgress = useCallback(() => {
    if (frame.current !== null) {
      if (typeof cancelAnimationFrame === 'function') cancelAnimationFrame(frame.current);
      else clearTimeout(frame.current);
      frame.current = null;
    }
    pendingProgress.current = null;
  }, []);

  // A workspace that is closed mid-run stops its run (it would otherwise keep the CPU busy for nothing).
  useEffect(
    () => () => {
      runRef.current?.cancel();
      runRef.current = null;
      dropProgress();
    },
    [dropProgress],
  );

  const processFiles = useCallback(async (files: File[], operations: string[], extra: ProcessingOptions = {}): Promise<ProcessingResult> => {
    setIsProcessing(true);
    setError(null);
    setResults(null);
    setProgress({ phase: 'PREP', operation: 'csv-loader', progress: 0, message: `Starting multi-CSV pipeline for ${files.length} files...` });

    try {
      const run = runEngine(files, operations, reportProgress, {
        maxLhsSize: 4,
        userNames: loadUserNames(),
        namer: modelNamingEnabled() ? createWorkerNamer() : null,
        namingMode: modelNamingEnabled() ? 'model' : 'rules',
        ...extra,
      });
      runRef.current = run;
      const result = await run.promise;

      setResults(result);
      if (!result.success) {
        setError(result.error || 'Multi-CSV processing failed');
      }

      return result;
    } catch (err) {
      const errorMessage = err instanceof Error ? err.message : String(err ?? 'unknown error');
      setError(errorMessage);

      const failedResult: ProcessingResult = {
        success: false,
        error: errorMessage,
        stats: {},
        outputFiles: [],
        logs: [`Error: ${errorMessage}`],
        duration: '0s',
        selectedOperations: operations
      };

      setResults(failedResult);
      return failedResult;
    } finally {
      runRef.current = null;
      dropProgress();
      setIsProcessing(false);
      setProgress(null);
    }
  }, [dropProgress, reportProgress]);

  const processFile = useCallback(async (file: File, operations: string[]): Promise<ProcessingResult> => {
    return processFiles([file], operations);
  }, [processFiles]);

  const cancelProcessing = useCallback(() => {
    runRef.current?.cancel();
    dropProgress();
    setIsProcessing(false);
    setProgress(null);
  }, [dropProgress]);

  const downloadFile = useCallback(async (file: any) => {
    await DatabaseService.downloadFile(file);
  }, []);

  const reset = useCallback(() => {
    setIsProcessing(false);
    setProgress(null);
    setResults(null);
    setError(null);
  }, []);

  return {
    isProcessing,
    progress,
    results,
    error,
    processFile,
    processFiles,
    cancelProcessing,
    downloadFile,
    reset
  };
};