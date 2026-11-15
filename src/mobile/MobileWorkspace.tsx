/**
 * One workspace on the phone: its files, pipeline, request box, run and results. It runs on the same hooks as a
 * desktop workspace (useWorkflowGraph, useNLWorkflow, useNormalization, the result cache), so undo/redo, saved
 * workflow files, plan previews and cached results behave identically. Workspaces stay mounted while hidden, so a
 * run continues in the background when another workspace is on screen.
 */
/* eslint-disable @typescript-eslint/no-explicit-any -- engine results are untyped (see useResultViews) */
import { useCallback, useEffect, useMemo, useState } from 'react';
import { Play } from 'lucide-react';
import { useNormalization } from '../hooks/useNormalization';
import { useWorkflowGraph } from '../hooks/useWorkflowGraph';
import { useNLWorkflow } from '../hooks/useNLWorkflow';
import { OPERATIONS } from '../config/operations';
import { prerequisitesToAdd } from '../services/nlp/workflow/planner';
import { isSupportedInputName } from '../services/processors/spreadsheetProcessor';
import type { ProcessingOptions, SQLDialect } from '../services/api/databaseService';
import { notify } from '../services/notify';
import { validateSequence } from '../utils/workflowValidation';
import { loadCachedResult, saveCachedResult } from '../utils/resultCache';
import { saveFileInBackground } from '../utils/saveFile';
import type { WorkspaceTab, TabStatus } from '../hooks/useWorkspaceTabs';
import { DEFAULT_GOAL, ordered, planForGoal, toggleStep } from './pipeline';
import { DataScreen, fileSize } from './DataScreen';
import { PipelineScreen, type PipelineControls } from './PipelineScreen';
import { ResultsScreen } from './ResultsScreen';
import { RunSheet } from './RunSheet';
import { BottomNav, TopBar, type Screen } from './ui';

type Say = (text: string, error?: boolean) => void;

export function MobileWorkspace({
  tab,
  active,
  say,
  onStatus,
  onSuggestTitle,
  onWorkspaces,
  onSettings,
}: {
  tab: WorkspaceTab;
  active: boolean;
  say: Say;
  onStatus: (id: string, status: TabStatus) => void;
  onSuggestTitle: (id: string, title: string) => void;
  onWorkspaces: () => void;
  onSettings: () => void;
}) {
  const tabId = tab.id;
  const [screen, setScreenState] = useState<Screen>('data');
  const [files, setFiles] = useState<File[]>([]);
  const [dialect, setDialect] = useState<SQLDialect | undefined>(undefined);
  const [results, setResults] = useState<any | null>(() => loadCachedResult(tabId));
  const [fresh, setFresh] = useState(false);
  const { isProcessing, progress, processFiles, cancelProcessing } = useNormalization();

  // Messages for the user: errors are shown, and success and failure also go to the system notifications.
  const log = useCallback(
    (level: 'info' | 'success' | 'warning' | 'error', message: string) => {
      if (level === 'error') say(message, true);
      if (level !== 'info') notify(level, message, tabId);
    },
    [say, tabId],
  );
  // The graph's own notes (phase-order hints, "removed operation") are routine on a list-based pipeline, where
  // every step is placed in run order: only its errors reach the user.
  const graphLog = useCallback(
    (level: 'info' | 'success' | 'warning' | 'error', message: string) => {
      if (level === 'error') say(message, true);
      else console.debug(`[pipeline] ${message}`);
    },
    [say],
  );
  const graph = useWorkflowGraph({ log: graphLog });

  // A new workspace starts with the default goal, so Run works right away.
  useEffect(() => {
    if (graph.placedOperationLabels.length === 0) graph.addOperationsBatch(planForGoal(DEFAULT_GOAL));
    // Runs once, when the workspace is created.
  }, []);

  // Android's back button walks back through this workspace's screens.
  const setScreen = useCallback((s: Screen) => {
    setScreenState(prev => {
      if (prev !== s) window.history.pushState({ screen: s }, '');
      return s;
    });
    if (s === 'results') setFresh(false);
  }, []);
  useEffect(() => {
    if (!active) return;
    const onPop = (e: PopStateEvent) => setScreenState((e.state?.screen as Screen) ?? 'data');
    window.addEventListener('popstate', onPop);
    return () => window.removeEventListener('popstate', onPop);
  }, [active]);

  const steps = useMemo(() => ordered(graph.placedOperationLabels), [graph.placedOperationLabels]);

  const run = useCallback(
    async (extra: ProcessingOptions = {}) => {
      if (isProcessing) return say('This workspace is already running.');
      if (!files.length) {
        say('Add at least one file first.', true);
        return setScreen('data');
      }
      if (!steps.length) return say('Choose a goal or check at least one step.', true);
      const check = validateSequence(graph.nodes, graph.edges);
      if (!check.isValid) return say(`The pipeline is not valid: ${check.errors.join('; ')}`, true);
      const result: any = await processFiles(files, graph.operationLabels, { sqlDialect: dialect, ...extra });
      if (result?.success) {
        setResults(result);
        saveCachedResult(result, tabId);
        notify('success', `Pipeline completed for ${files.length} table(s)`, tabId);
        if (active) setScreen('results');
        else setFresh(true);
      } else if (result && !/cancel/i.test(result.error ?? '')) {
        log('error', `Processing failed: ${result.error ?? 'unknown error'}`);
      }
    },
    [active, dialect, files, graph.edges, graph.nodes, graph.operationLabels, isProcessing, log, processFiles, say, setScreen, steps.length, tabId],
  );

  const nl = useNLWorkflow({
    graph,
    run: () => void run(),
    hasFiles: files.length > 0,
    workspaceId: tabId,
    onOptions: o => setDialect(o.dialect as SQLDialect | undefined),
  });

  useEffect(() => {
    onStatus(tabId, { running: isProcessing, operations: steps.length });
  }, [isProcessing, onStatus, steps.length, tabId]);

  const addFiles = useCallback(
    (picked: File[]) => {
      const ok = picked.filter(f => isSupportedInputName(f.name, f.type));
      const rejected = picked.filter(f => !ok.includes(f));
      if (ok.length) {
        setFiles(prev => [...prev, ...ok]);
        onSuggestTitle(tabId, ok[0].name.replace(/\.[^.]+$/, ''));
      }
      if (rejected.length) say(`Not a CSV or spreadsheet: ${rejected.map(f => f.name).join(', ')}`, true);
    },
    [onSuggestTitle, say, tabId],
  );

  const pipeline: PipelineControls = {
    steps,
    canUndo: graph.canUndo,
    canRedo: graph.canRedo,
    undo: graph.undo,
    redo: graph.redo,
    setSteps: next => {
      graph.clearGraph();
      if (next.length) graph.addOperationsBatch(ordered(next));
    },
    toggle: op => {
      if (steps.includes(op)) {
        const t = toggleStep(steps, op);
        graph.clearGraph();
        if (t.steps.length) graph.addOperationsBatch(t.steps);
        if (t.alsoChanged.length) say(`Also removed: ${t.alsoChanged.join(', ')}`);
      } else {
        const { add, prerequisites } = prerequisitesToAdd(op, graph.placedOperationLabels);
        graph.addOperationsBatch(add);
        if (prerequisites.length) say(`Also added: ${prerequisites.join(', ')}`);
      }
    },
    save: () => {
      const snapshot = graph.serializeSnapshot();
      const name = `dbflow-workflow-${snapshot.timestamp.slice(0, 19).replace(/[:T]/g, '-')}.json`;
      saveFileInBackground(name, JSON.stringify(snapshot, null, 2), 'application/json');
    },
    open: file => {
      file.text().then(
        text => {
          if (graph.loadSnapshot(text)) say(`Opened ${file.name}`);
        },
        () => say(`Could not read ${file.name}`, true),
      );
    },
  };

  const detail =
    screen === 'data'
      ? files.length
        ? `${files.length} ${files.length === 1 ? 'file' : 'files'} · ${fileSize(files.reduce((n, f) => n + f.size, 0))}`
        : 'No files'
      : screen === 'pipeline'
        ? `${steps.length} of ${Object.keys(OPERATIONS).length} steps${dialect ? ` · SQL: ${dialect}` : ''}`
        : results
          ? `Last run: ${results.duration ?? '—'}`
          : 'No run yet';

  return (
    <div className={`m-ws${active ? '' : ' m-hidden'}`} aria-hidden={!active}>
      <TopBar title={tab.title} color={tab.color} running={isProcessing} detail={detail} onTitle={onWorkspaces} onSettings={onSettings} />

      <main className={`m-screen${screen === 'pipeline' ? ' m-with-cta' : ''}`}>
        {screen === 'data' && (
          <DataScreen files={files} busy={isProcessing} onAdd={addFiles} onRemove={i => setFiles(prev => prev.filter((_, j) => j !== i))} />
        )}
        {screen === 'pipeline' && <PipelineScreen pipeline={pipeline} nl={nl} />}
        {screen === 'results' && (
          <ResultsScreen results={results} onGoData={() => setScreen('data')} onMerge={() => void run({ mergeOverlappingFiles: true })} />
        )}
      </main>

      {screen === 'pipeline' && (
        <div className="m-cta">
          <button className="m-btn m-btn-primary m-btn-block" onClick={() => void run()} disabled={isProcessing || !steps.length || !files.length}>
            <Play />
            {files.length ? `Run ${steps.length} steps on ${files.length} ${files.length === 1 ? 'file' : 'files'}` : 'Add files first'}
          </button>
        </div>
      )}

      <BottomNav screen={screen} onGo={setScreen} files={files.length} resultsReady={fresh} />

      {active && isProcessing && <RunSheet progress={progress} onCancel={cancelProcessing} />}
    </div>
  );
}
