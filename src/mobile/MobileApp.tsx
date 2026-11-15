/**
 * DBFlow for phones (the Android app). The same engine, hooks and result views as the desktop app, with an
 * interface built for touch: three screens (Data, Pipeline, Results) behind a bottom bar, a goal list and a step
 * checklist instead of the canvas, and a progress sheet while the engine runs.
 */
/* eslint-disable @typescript-eslint/no-explicit-any -- engine results are untyped (see useResultViews) */
import { useCallback, useEffect, useRef, useState } from 'react';
import { Play } from 'lucide-react';
import './mobile.css';
import { useNormalization } from '../hooks/useNormalization';
import { useModelNaming } from '../hooks/useModelNaming';
import { getSharedProvider } from '../hooks/useNLWorkflow';
import { resolve, NullProvider } from '../services/nlp/workflow';
import { isSupportedInputName } from '../services/processors/spreadsheetProcessor';
import type { ProcessingOptions, SQLDialect } from '../services/api/databaseService';
import { useTheme } from '../theme/theme';
import { DEFAULT_GOAL, ordered, planForGoal, toggleStep } from './pipeline';
import { DataScreen, fileSize } from './DataScreen';
import { PipelineScreen, type AskState } from './PipelineScreen';
import { ResultsScreen } from './ResultsScreen';
import { RunSheet } from './RunSheet';
import { BottomNav, Sheet, Switch, Toast, TopBar, type Screen, type ToastMessage } from './ui';

const TITLE: Record<Screen, string> = { data: 'Data', pipeline: 'Pipeline', results: 'Results' };

export default function MobileApp() {
  const [screen, setScreenState] = useState<Screen>('data');
  const [files, setFiles] = useState<File[]>([]);
  const [steps, setSteps] = useState<string[]>(() => planForGoal(DEFAULT_GOAL));
  const [dialect, setDialect] = useState<SQLDialect | undefined>(undefined);
  const [results, setResults] = useState<any | null>(null);
  const [freshResults, setFreshResults] = useState(false);
  const [settings, setSettings] = useState(false);
  const [toast, setToast] = useState<ToastMessage | null>(null);
  const [ask, setAsk] = useState<AskState>({ busy: false, message: null, candidates: [] });
  const { isProcessing, progress, processFiles, cancelProcessing } = useNormalization();
  const [aiNames, setAiNames] = useModelNaming();
  const { theme, toggle: toggleTheme } = useTheme();
  const say = useCallback((text: string, error = false) => setToast({ id: Date.now(), text, error }), []);

  // Android's back button walks back through the screens instead of closing the app.
  const setScreen = useCallback((s: Screen) => {
    setScreenState(prev => {
      if (prev !== s) window.history.pushState({ screen: s }, '');
      return s;
    });
    if (s === 'results') setFreshResults(false);
  }, []);
  useEffect(() => {
    window.history.replaceState({ screen: 'data' }, '');
    const onPop = (e: PopStateEvent) => setScreenState((e.state?.screen as Screen) ?? 'data');
    window.addEventListener('popstate', onPop);
    return () => window.removeEventListener('popstate', onPop);
  }, []);

  const addFiles = useCallback(
    (picked: File[]) => {
      const ok = picked.filter(f => isSupportedInputName(f.name, f.type));
      const rejected = picked.filter(f => !ok.includes(f));
      if (ok.length) setFiles(prev => [...prev, ...ok]);
      if (rejected.length) say(`Not a table: ${rejected.map(f => f.name).join(', ')}. Use CSV, Excel or OpenDocument files.`, true);
      else if (ok.length) say(`${ok.length === 1 ? ok[0].name : `${ok.length} files`} added`);
    },
    [say],
  );

  const run = useCallback(
    async (extra: ProcessingOptions = {}) => {
      if (isProcessing) return;
      if (!files.length) {
        say('Add at least one table first.', true);
        setScreen('data');
        return;
      }
      if (!steps.length) {
        say('Pick a goal or switch on at least one step.', true);
        return;
      }
      const result: any = await processFiles(files, ordered(steps), { sqlDialect: dialect, ...extra });
      if (result?.success) {
        setResults(result);
        setScreen('results');
      } else if (result && !/cancel/i.test(result.error ?? '')) {
        say(result.error ?? 'The run failed.', true);
      }
    },
    [dialect, files, isProcessing, processFiles, say, setScreen, steps],
  );

  // A new result is announced on the Results tab until it is opened.
  const previous = useRef<any>(null);
  useEffect(() => {
    if (results && results !== previous.current && screen !== 'results') setFreshResults(true);
    previous.current = results;
  }, [results, screen]);

  const askFor = useCallback(
    async (text: string, provider = getSharedProvider()) => {
      setAsk({ busy: true, message: null, candidates: [] });
      try {
        const res = await resolve(text, provider);
        if (res.plan.length) {
          setSteps(ordered(res.plan));
          if (res.options.dialect) setDialect(res.options.dialect as SQLDialect);
          const extra = res.addedAsDependencies.length ? ` (plus ${res.addedAsDependencies.length} steps they need)` : '';
          setAsk({
            busy: false,
            message: `Planned ${res.targets.join(', ')}${extra}${res.options.dialect ? `, SQL for ${res.options.dialect}` : ''}.`,
            candidates: [],
          });
          if (res.options.run) void run();
        } else {
          setAsk({
            busy: false,
            message: res.message ?? 'I did not understand that. Try "normalize to 3NF" or pick a goal.',
            candidates: res.candidates.slice(0, 3).map(c => c.operation),
          });
        }
      } catch (e) {
        setAsk({ busy: false, message: `Something went wrong reading that (${(e as Error).message}).`, candidates: [] });
      }
    },
    [run],
  );

  const toggle = useCallback(
    (op: string) => {
      const t = toggleStep(steps, op);
      setSteps(t.steps);
      if (t.alsoChanged.length) {
        const added = t.steps.includes(op);
        say(`${added ? 'Also added' : 'Also removed'}: ${t.alsoChanged.join(', ')}`);
      }
    },
    [say, steps],
  );

  const detail =
    screen === 'data'
      ? files.length
        ? `${files.length} ${files.length === 1 ? 'file' : 'files'} · ${fileSize(files.reduce((n, f) => n + f.size, 0))}`
        : 'No files'
      : screen === 'pipeline'
        ? `${steps.length} of 19 steps${dialect ? ` · SQL: ${dialect}` : ''}`
        : results
          ? `Last run: ${results.duration ?? '—'}`
          : 'No run yet';

  return (
    <div className="m-app">
      <TopBar title={TITLE[screen]} detail={detail} onSettings={() => setSettings(true)} />

      <main className={`m-screen${screen === 'pipeline' ? ' m-with-cta' : ''}`} key={screen}>
        {screen === 'data' && (
          <DataScreen
            files={files}
            busy={isProcessing}
            onAdd={addFiles}
            onRemove={i => setFiles(prev => prev.filter((_, j) => j !== i))}
          />
        )}
        {screen === 'pipeline' && (
          <PipelineScreen
            steps={steps}
            onSteps={setSteps}
            onToggle={toggle}
            ask={ask}
            onAsk={text => void askFor(text)}
            onPickCandidate={op => void askFor(op, new NullProvider())}
          />
        )}
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

      <BottomNav screen={screen} onGo={setScreen} files={files.length} resultsReady={freshResults} />

      {isProcessing && <RunSheet progress={progress} onCancel={cancelProcessing} />}

      {settings && (
        <Sheet title="Settings" onClose={() => setSettings(false)}>
          <div className="m-group">
            <div className="m-row">
              <span className="m-row-text">
                <strong>AI table names</strong>
                <small>An on-device model names new tables. Off: names come from key columns (faster).</small>
              </span>
              <Switch on={aiNames} onChange={setAiNames} label="AI table names" />
            </div>
            <div className="m-row">
              <span className="m-row-text">
                <strong>Dark theme</strong>
              </span>
              <Switch on={theme === 'dark'} onChange={() => toggleTheme()} label="Dark theme" />
            </div>
          </div>
        </Sheet>
      )}

      <Toast toast={toast} onDone={() => setToast(null)} />
    </div>
  );
}
