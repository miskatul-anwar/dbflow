import { lazy, Suspense, useCallback, useEffect, useMemo, useRef, useState } from 'react';
import { BrowserRouter as Router, Routes, Route, useLocation, useNavigate } from 'react-router-dom';
import { ReactFlowProvider, useReactFlow } from '@xyflow/react';
import type { Node } from '@xyflow/react';

import { ChevronsLeft, PanelLeftOpen, RotateCcw } from 'lucide-react';
import Heading from './components/heading';
const OutputPage = lazy(() => import('./components/OutputPage'));
import { OperationPalette } from './components/workflow/OperationPalette';
import { FilesPanel } from './components/workflow/FilesPanel';
import { FlowCanvas } from './components/workflow/FlowCanvas';
import { useNormalization } from './hooks/useNormalization';
import type { ProcessingOptions, SQLDialect } from './services/api/databaseService';
import { useNLWorkflow } from './hooks/useNLWorkflow';
import { NLCommandBar } from './components/nl/NLCommandBar';
import { WorkspaceTabBar } from './components/workflow/WorkspaceTabBar';
import { useWorkspaceTabs, MAX_TABS, type TabStatus } from './hooks/useWorkspaceTabs';
import { WorkspaceNameDialog } from './components/workflow/WorkspaceNameDialog';
import { ConfirmDialog } from './components/common/ConfirmDialog';
import { useModelNaming } from './hooks/useModelNaming';
import { activeSurface } from './utils/tabColors';
import { prerequisitesToAdd } from './services/nlp/workflow/planner';
import { DEFAULT_PALETTE_WIDTH, isListMode, usePaletteResize } from './hooks/usePaletteResize';
import { IconButton } from './components/ui/button';
import { useWorkflowGraph, type WorkflowSnapshot, buildChainEdges, nodePosition } from './hooks/useWorkflowGraph';
import { Logger } from './services/utils/logger';
import { notify, setActiveWorkspace, setWorkspaceNameResolver, toast, type ToastAction } from './services/notify';
import { OPERATIONS } from './config/operations';
import { validateSequence } from './utils/workflowValidation';
import { clearCachedResult, loadCachedResult, saveCachedResult } from './utils/resultCache';
import { ErrorBoundary } from './components/common/ErrorBoundary';
import { isSupportedInputName } from './services/processors/spreadsheetProcessor';
import { saveFileInBackground } from './utils/saveFile';
import { addFiles, baseNameOf, moveFile, removeFile, renameFile, replaceFile } from './utils/fileList';
import './styles/App.css';

const logger = new Logger();

type LogLevel = 'info' | 'success' | 'error' | 'warning';

const ALL_OPERATIONS = Object.keys(OPERATIONS);

function downloadJson(snapshot: WorkflowSnapshot): void {
  const name = `dbflow-workflow-${snapshot.timestamp.slice(0, 19).replace(/[:T]/g, '-')}.json`;
  saveFileInBackground(name, JSON.stringify(snapshot, null, 2), 'application/json');
}

interface WorkspaceProps {
  tabId: string;
  /** False while another tab (or the output page) is showing; the workspace stays mounted. */
  active: boolean;
  onStatus: (id: string, status: TabStatus) => void;
  onSuggestTitle: (id: string, title: string) => void;
}

/** One workspace tab: operation palette + full-canvas workflow builder, with its own files, graph and results. */
function Workspace({ tabId, active, onStatus, onSuggestTitle }: WorkspaceProps) {
  const navigate = useNavigate();
  const { fitView } = useReactFlow();
  // No dataset is preloaded: results always come from files the user uploaded.
  const [uploadedFiles, setUploadedFiles] = useState<File[]>([]);
  const [processedResults, setProcessedResults] = useState<any>(() => loadCachedResult(tabId));

  const { isProcessing, processFiles } = useNormalization();
  // The SQL dialect asked for in the request box ("... in PostgreSQL"); unset means the default output.
  const [sqlDialect, setSqlDialect] = useState<SQLDialect | undefined>(undefined);

  // ----- logging & status ----------------------------------------------------

  const logMessage = useCallback((level: LogLevel, message: string, category?: string) => {
    if (level === 'success') logger.success(message, undefined, category);
    else if (level === 'error') logger.error(message, undefined, category);
    else logger.info(level === 'warning' ? `[WARNING] ${message}` : message, undefined, category);

    // Errors that stopped what the user just did are toasts. Warnings and routine confirmations (files added,
    // workflow saved, steps added by a request) stay in the log; the Files panel and the canvas already show them.
    // A run's outcome goes through reportOutcome instead.
    if (level === 'error') toast('error', message);
  }, []);

  // A run's outcome: a toast when the user is looking, a system notification when they are away.
  const reportOutcome = useCallback((level: 'success' | 'error', message: string, action?: ToastAction) => {
    if (level === 'success') logger.success(message, undefined, 'EXECUTION');
    else logger.error(message, undefined, 'EXECUTION');
    notify(level, message, tabId, { action });
  }, [tabId]);

  // ----- workflow graph ------------------------------------------------------

  const graph = useWorkflowGraph({ log: logMessage });

  // ----- file handling -------------------------------------------------------

  // Files are added to the list (one at a time or many at once); the Files panel renames, replaces, reorders and
  // removes them. A file already in the list is skipped.
  const filesRef = useRef(uploadedFiles);
  filesRef.current = uploadedFiles;
  const setFiles = useCallback((files: File[]) => {
    filesRef.current = files;
    setUploadedFiles(files);
  }, []);

  const handleFileUpload = useCallback(
    (files: File[]) => {
      const r = addFiles(filesRef.current, files);
      if (r.added.length) {
        if (filesRef.current.length === 0) onSuggestTitle(tabId, baseNameOf(r.added[0].name));
        setFiles(r.files);
        logMessage('success', `Added ${r.added.length} file${r.added.length === 1 ? '' : 's'}: ${r.added.map(f => f.name).join(', ')}`, 'FILE');
      }
      if (r.duplicates.length) logMessage('warning', `Already added: ${r.duplicates.map(f => f.name).join(', ')}`, 'FILE');
      if (r.unsupported.length) logMessage('error', `Not a CSV or spreadsheet: ${r.unsupported.map(f => f.name).join(', ')}`, 'FILE');
    },
    [logMessage, onSuggestTitle, setFiles, tabId],
  );

  const fileActions = useMemo(
    () => ({
      onAdd: handleFileUpload,
      onRename: (i: number, name: string) => {
        const r = renameFile(filesRef.current, i, name);
        if (r.error !== undefined) return r.error;
        setFiles(r.files);
        return undefined;
      },
      onReplace: (i: number, file: File) => {
        if (!isSupportedInputName(file.name, file.type)) {
          logMessage('error', `Not a CSV or spreadsheet: ${file.name}`, 'FILE');
          return;
        }
        setFiles(replaceFile(filesRef.current, i, file));
      },
      onMove: (i: number, by: -1 | 1) => setFiles(moveFile(filesRef.current, i, by)),
      onRemove: (i: number) => setFiles(removeFile(filesRef.current, i)),
      onClear: () => setFiles([]),
      onDownload: (i: number) => {
        const f = filesRef.current[i];
        if (f) saveFileInBackground(f.name, f, f.type || 'application/octet-stream');
      },
    }),
    [handleFileUpload, logMessage, setFiles],
  );

  // ----- output navigation ---------------------------------------------------

  // Whether this workspace is the one on screen (read inside async callbacks).
  const activeRef = useRef(active);
  activeRef.current = active;
  // Output was pressed while a run was in progress: open the results when it finishes.
  const openOutputWhenDone = useRef(false);

  // The result goes into this tab's in-memory cache and only the tab id travels with the navigation. Router state is
  // written with history.pushState, which WebKit (the desktop app's webview on Linux and macOS) limits in size; a large
  // result made it throw, and the router then fell back to a full page load that reset every workspace.
  const navigateToOutput = useCallback(
    (results: any) => {
      saveCachedResult(results, tabId);
      navigate('/output', { state: { tabId } });
    },
    [navigate, tabId],
  );

  // ----- execution -----------------------------------------------------------

  const executePipeline = useCallback(
    async (files: File[], operations: string[], extra: ProcessingOptions = {}) => {
      try {
        const result = await processFiles(files, operations, { sqlDialect, ...extra });
        if (result?.success) {
          setProcessedResults(result);
          saveCachedResult(result, tabId);
          const autoOpen = openOutputWhenDone.current && activeRef.current;
          // When the results do not open by themselves, the toast offers to open them.
          reportOutcome(
            'success',
            `Pipeline completed for ${files.length} table(s)`,
            !autoOpen && activeRef.current ? { label: 'View results', onClick: () => navigateToOutput(result) } : undefined,
          );
          if (openOutputWhenDone.current) {
            openOutputWhenDone.current = false;
            if (activeRef.current) navigateToOutput(result);
            else logMessage('info', 'Results are ready. Open this workspace and press Output to see them.', 'NAVIGATION');
          }
        } else {
          openOutputWhenDone.current = false;
          // A run the user cancelled needs no message.
          if (!/cancel/i.test(result?.error ?? '')) reportOutcome('error', `Processing failed: ${result?.error ?? 'unknown error'}`);
        }
        return result;
      } catch (err) {
        openOutputWhenDone.current = false;
        reportOutcome('error', `Execution error: ${(err as Error).message}`);
        return null;
      }
    },
    [logMessage, navigateToOutput, processFiles, reportOutcome, sqlDialect, tabId],
  );

  const hasFiles = uploadedFiles.length > 0;
  const operationCount = Math.max(0, graph.nodes.length - 1);

  /** Runs the pipeline on the canvas; resolves to the result, or null when nothing ran. */
  const runCurrent = useCallback(async (extra: ProcessingOptions = {}) => {
    if (isProcessing) {
      logMessage('info', 'This workspace is already running its pipeline.', 'EXECUTION');
      return null;
    }
    if (!hasFiles) {
      logMessage('error', 'Upload at least one CSV file before executing.', 'EXECUTION');
      return null;
    }
    if (operationCount === 0) {
      logMessage('error', 'Workflow is empty — add operations from the palette.', 'EXECUTION');
      return null;
    }

    const validation = validateSequence(graph.nodes, graph.edges);
    if (!validation.isValid) {
      logMessage('error', `Workflow validation failed: ${validation.errors.join('; ')}`, 'EXECUTION');
      return null;
    }
    if (validation.warnings.length > 0) {
      validation.warnings.forEach(w => logMessage('warning', w, 'EXECUTION'));
    }

    return executePipeline(uploadedFiles, graph.operationLabels, extra);
  }, [executePipeline, graph.edges, graph.nodes, graph.operationLabels, hasFiles, isProcessing, logMessage, operationCount, uploadedFiles]);

  const handleRun = useCallback(() => {
    void runCurrent();
  }, [runCurrent]);

  const handleRunAll = useCallback(async () => {
    if (!hasFiles) {
      logMessage('error', 'Upload at least one CSV file before running the pipeline.', 'EXECUTION');
      return null;
    }
    if (isProcessing) return null;
    const files = uploadedFiles;

    // Build the canonical full pipeline on the canvas, laid out like any other pipeline (rows of steps).
    const start = graph.nodes[0];
    const builtNodes: Node[] = [start, ...ALL_OPERATIONS.map((label, index) => ({
      id: String(index + 1),
      position: nodePosition(index + 1),
      data: {
        label,
        phase: OPERATIONS[label]?.phase ?? 'PREP',
        operationId: OPERATIONS[label]?.id ?? label.toLowerCase(),
        order: OPERATIONS[label]?.order ?? index + 1,
      },
      type: 'tealNode' as const,
    }))];

    const sequenceResult = validateSequence(builtNodes);
    if (!sequenceResult.isValid) {
      logMessage('error', `Pipeline validation failed: ${sequenceResult.errors.join(', ')}`, 'EXECUTION');
      return null;
    }

    // replaceGraph derives the sequential chain edges when none are given.
    graph.replaceGraph(builtNodes, buildChainEdges(builtNodes));
    // Frame the freshly built chain once React Flow has laid it out.
    requestAnimationFrame(() => void fitView({ padding: 0.08, maxZoom: 1.1, duration: 400 }));
    return executePipeline(files, ALL_OPERATIONS);
  }, [executePipeline, fitView, graph, hasFiles, isProcessing, logMessage, uploadedFiles]);

  const handleViewOutput = useCallback(async () => {
    // While a run is in progress the canvas is left alone; its results open when it finishes.
    if (isProcessing) {
      openOutputWhenDone.current = true;
      logMessage('info', 'The pipeline is still running. Its results open as soon as it finishes.', 'NAVIGATION');
      return;
    }
    let ready = processedResults;
    if (!ready) ready = loadCachedResult(tabId);
    if (ready?.success || ready?.tables) {
      navigateToOutput(ready);
      return;
    }
    if (!hasFiles) {
      logMessage('error', 'Nothing to show yet. Upload a data file and run the pipeline first.', 'NAVIGATION');
      return;
    }
    // Run the pipeline the user built; only an empty canvas gets the full default pipeline.
    openOutputWhenDone.current = true;
    if (operationCount > 0) {
      await runCurrent();
    } else {
      logMessage('info', 'Building the full pipeline to generate output…', 'NAVIGATION');
      await handleRunAll();
    }
    openOutputWhenDone.current = false;
  }, [handleRunAll, hasFiles, isProcessing, logMessage, navigateToOutput, operationCount, processedResults, runCurrent, tabId]);

  // ----- rerun asked for by the results page ------------------------------------

  // "Merge overlapping files and run again" on the results page comes back here with the request in the router
  // state. The request is cleared first, so going back and forth never repeats it.
  const location = useLocation();
  const rerun = (location.state as { rerun?: { tabId: string; mergeOverlappingFiles?: boolean } } | null)?.rerun;
  const handledRerun = useRef<object | null>(null);
  useEffect(() => {
    if (!rerun || rerun.tabId !== tabId || handledRerun.current === rerun) return;
    handledRerun.current = rerun;
    navigate('/', { replace: true, state: null });
    openOutputWhenDone.current = true;
    logMessage('info', 'Merging overlapping files and running the pipeline again…', 'EXECUTION');
    void runCurrent({ mergeOverlappingFiles: rerun.mergeOverlappingFiles }).then(result => {
      if (!result) openOutputWhenDone.current = false;
    });
  }, [logMessage, navigate, rerun, runCurrent, tabId]);

  // ----- snapshot save/load ----------------------------------------------------

  const handleSaveState = useCallback(() => {
    try {
      downloadJson(graph.serializeSnapshot());
      logMessage('success', `Workflow saved (${operationCount} operations)`, 'FILE');
    } catch (error) {
      logMessage('error', `Save failed: ${(error as Error).message}`, 'FILE');
    }
  }, [graph, logMessage, operationCount]);

  const handleLoadState = useCallback(
    (file: File) => {
      const reader = new FileReader();
      reader.onload = event => graph.loadSnapshot(String(event.target?.result ?? ''));
      reader.onerror = () => logMessage('error', `Could not read ${file.name}`, 'FILE');
      reader.readAsText(file);
    },
    [graph, logMessage],
  );

  const nl = useNLWorkflow({
    graph,
    run: handleRun,
    hasFiles,
    workspaceId: tabId,
    onOptions: o => setSqlDialect(o.dialect as SQLDialect | undefined),
  });

  // Report running state and step count to the tab bar.
  useEffect(() => {
    onStatus(tabId, { running: isProcessing, operations: operationCount });
  }, [isProcessing, onStatus, operationCount, tabId]);

  // Clicking a locked palette item adds the steps it needs first, in order.
  const addWithPrerequisites = useCallback(
    (label: string) => {
      const { add, prerequisites } = prerequisitesToAdd(label, graph.placedOperationLabels);
      graph.addOperationsBatch(add);
      if (prerequisites.length > 0) {
        logMessage('info', `Added ${label} with ${prerequisites.length} prerequisite${prerequisites.length === 1 ? '' : 's'}: ${prerequisites.join(', ')}`, 'WORKFLOW');
      }
    },
    [graph, logMessage],
  );

  const palette = usePaletteResize();
  const [aiNaming, setAiNaming] = useModelNaming();

  return (
    <div
      className={`nf-shell${palette.isCollapsed ? ' is-palette-collapsed' : ''}${palette.isDragging ? ' is-dragging' : ''}`}
      style={{
        ['--nf-palette-width' as string]: palette.isCollapsed ? '0px' : `${palette.width}px`,
      }}
    >
      <header className="nf-header">
        <Heading
          onUndo={graph.undo}
          onRedo={graph.redo}
          canUndo={graph.canUndo}
          canRedo={graph.canRedo}
          onRun={handleRun}
          onViewOutput={() => void handleViewOutput()}
          onSaveState={handleSaveState}
          onLoadState={handleLoadState}
          isRunning={isProcessing}
          isOutputReady={Boolean(processedResults?.success)}
          aiNaming={aiNaming}
          onToggleAiNaming={() => {
            setAiNaming(!aiNaming);
            logMessage('info', aiNaming ? 'AI naming off: new tables are named from their key columns.' : 'AI naming on: the on-device model suggests names for new tables.', 'NAMING');
          }}
          onNotice={message => logMessage('error', message, 'FILE')}
        />
      </header>

      {/* Menu: the left sidebar holding Files and Operations. It is collapsed, expanded and resized as one. */}
      <aside
        className={`nf-palette nf-palette--stack${palette.isCollapsed ? ' is-collapsed' : ''}`}
        aria-label="Menu"
      >
        <div className={`nf-menu-head${isListMode(palette.width) ? '' : ' is-rail'}`}>
          {isListMode(palette.width) && <h2 className="text-sm font-semibold text-gray-900">Menu</h2>}
          <div className="flex items-center gap-0.5">
            {isListMode(palette.width) && Math.abs(palette.width - DEFAULT_PALETTE_WIDTH) > 2 && (
              <IconButton size="icon-sm" label="Reset sidebar width" icon={<RotateCcw />} onClick={palette.resetWidth} />
            )}
            <IconButton size="icon-sm" label="Collapse menu" icon={<ChevronsLeft />} onClick={palette.toggleCollapse} />
          </div>
        </div>
        <div className="nf-side-card nf-side-card--files">
          <FilesPanel files={uploadedFiles} compact={!isListMode(palette.width)} busy={isProcessing} {...fileActions} />
        </div>
        <div className="nf-side-card nf-side-card--grow" aria-label="Operation palette">
        <OperationPalette
          activeOperations={graph.placedOperationLabels}
          onAdd={graph.tryAddOperation}
          onAddWithPrerequisites={addWithPrerequisites}
          width={palette.width}
        />
        </div>
        <div
          className={`nf-palette-resizer${palette.isDragging ? ' is-active' : ''}`}
          onMouseDown={palette.handleResizeStart}
          onDoubleClick={palette.resetWidth}
          title="Drag to resize sidebar (double-click to reset)"
          role="separator"
          aria-orientation="vertical"
        />
      </aside>

      <main className="nf-canvas-panel">
        <h1 className="sr-only">Pipeline builder</h1>
        {palette.isCollapsed && (
          <button
            type="button"
            className="nf-palette-expand-tab"
            onClick={palette.toggleCollapse}
            onMouseDown={e => palette.handleResizeStart(e, true)}
            title="Expand the menu (click or drag)"
            aria-label="Expand menu"
          >
            <PanelLeftOpen size={15} />
            <span>Menu</span>
          </button>
        )}

        <NLCommandBar nl={nl} active={active} />

        <div className="nf-canvas">
          <FlowCanvas
            flowId={tabId}
            nodes={graph.nodes}
            edges={graph.edges}
            onNodesChange={graph.onNodesChange}
            onEdgesChange={graph.onEdgesChange}
            onConnect={graph.onConnect}
            onConnectEnd={graph.onConnectEnd}
            isValidConnection={graph.isValidConnection}
          />
        </div>
      </main>
    </div>
  );
}

/**
 * Hosts every workspace tab. Tabs stay mounted (hidden) while inactive or while the
 * output page is open, so their graphs, files, results and running jobs survive switching.
 */
function AppShell() {
  const location = useLocation();
  const onOutput = location.pathname === '/output';
  const tabsApi = useWorkspaceTabs();
  const { tabs, activeId, status, addTab, closeTab, cycleTab } = tabsApi;

  // One dialog at a time: naming the first workspace (on launch), naming a new one (+), confirming that a
  // workspace with work in it is closed, or naming the workspace that replaces the last one when it is closed.
  type ShellDialog =
    | { kind: 'first' }
    | { kind: 'new' }
    | { kind: 'confirm-close'; id: string }
    | { kind: 'replace-last'; id: string };
  const [dialog, setDialog] = useState<ShellDialog | null>({ kind: 'first' });
  const askForNewWorkspace = useCallback(() => {
    if (tabsRef.current.length >= MAX_TABS) return;
    setDialog({ kind: 'new' });
  }, []);

  // System notifications name the workspace (its current title, so renames are picked up).
  const tabsRef = useRef(tabs);
  tabsRef.current = tabs;
  useEffect(() => {
    setWorkspaceNameResolver(id => tabsRef.current.find(t => t.id === id)?.title);
  }, []);
  useEffect(() => setActiveWorkspace(activeId), [activeId]);

  const finishClose = useCallback(
    (id: string, replacementName?: string) => {
      setDialog(null);
      closeTab(id, replacementName);
      clearCachedResult(id);
    },
    [closeTab],
  );

  // Closing asks first when work would be lost (in-app dialogs, never the browser's confirm box). Closing the last
  // workspace asks for the name of the one that replaces it.
  const requestClose = useCallback(
    (id: string) => {
      if (!tabs.some(t => t.id === id)) return;
      const { running = false, operations = 0 } = status[id] ?? {};
      if (tabs.length === 1) setDialog({ kind: 'replace-last', id });
      else if (running || operations > 0) setDialog({ kind: 'confirm-close', id });
      else finishClose(id);
    },
    [finishClose, status, tabs],
  );

  /** What closing workspace `id` discards, for the dialogs; empty when nothing is lost. */
  const lossOf = (id: string): string => {
    const tab = tabs.find(t => t.id === id);
    const { running = false, operations = 0 } = status[id] ?? {};
    const name = `“${tab?.title ?? 'this workspace'}”`;
    if (running) return `A pipeline is running in ${name}. Closing it stops the run and discards its files, pipeline and results.`;
    if (operations > 0) return `${name} has ${operations} pipeline step${operations === 1 ? '' : 's'}. Closing it discards its files, pipeline and results.`;
    return '';
  };

  // Tab shortcuts work on both pages.
  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      if (dialog) return; // a dialog is open: it owns the keyboard
      const mod = e.ctrlKey || e.metaKey;
      if (mod && !e.shiftKey && !e.altKey && e.key.toLowerCase() === 't') {
        e.preventDefault();
        askForNewWorkspace();
      } else if (mod && !e.shiftKey && !e.altKey && e.key.toLowerCase() === 'w') {
        e.preventDefault();
        requestClose(activeId);
      } else if (e.ctrlKey && e.key === 'Tab') {
        e.preventDefault();
        cycleTab(e.shiftKey ? -1 : 1);
      }
    };
    window.addEventListener('keydown', onKey);
    return () => window.removeEventListener('keydown', onKey);
  }, [activeId, askForNewWorkspace, cycleTab, dialog, requestClose]);

  return (
    <div
      className="nf-app"
      // The header takes the active tab's surface colour so the two read as one piece, on both pages.
      style={{ ['--nf-chrome-bg' as string]: activeSurface(tabs.find(t => t.id === activeId)?.color ?? null) }}
    >
      {/* One tab strip above both pages: the tab you pick decides whose pipeline or results you see. */}
      <WorkspaceTabBar
        tabs={tabs}
        activeId={activeId}
        status={status}
        onSelect={tabsApi.selectTab}
        onAdd={askForNewWorkspace}
        onClose={requestClose}
        onRename={tabsApi.renameTab}
        onColor={tabsApi.setTabColor}
      />
      {/* Workspaces stay mounted (hidden) while the Output page is open, so graphs, files and running jobs survive. */}
      <div className="nf-workspaces" hidden={onOutput}>
        {tabs.map(tab => (
          <div key={tab.id} className="nf-tab-pane" hidden={tab.id !== activeId}>
            <ErrorBoundary componentName={`Workspace ${tab.title}`}>
              <ReactFlowProvider>
                <Workspace
                  tabId={tab.id}
                  active={!onOutput && tab.id === activeId}
                  onStatus={tabsApi.setStatus}
                  onSuggestTitle={tabsApi.suggestTitle}
                />
              </ReactFlowProvider>
            </ErrorBoundary>
          </div>
        ))}
      </div>
      <Routes>
        <Route path="/" element={null} />
        {/* Keyed by tab so each workspace gets its own results, section and scroll position. */}
        <Route
          path="/output"
          element={
            // The Output page (tables, SQL viewer, diagrams) loads on first visit, not at start-up.
            <Suspense fallback={null}>
              <OutputPage key={activeId} tabId={activeId} tabs={tabs} />
            </Suspense>
          }
        />
      </Routes>
      {dialog?.kind === 'first' && (
        <WorkspaceNameDialog
          title="Name your workspace"
          suggestion={tabs[0]?.title ?? 'Workspace 1'}
          confirmLabel="Start"
          onConfirm={name => {
            if (tabs[0]) tabsApi.renameTab(tabs[0].id, name);
            setDialog(null);
          }}
        />
      )}
      {dialog?.kind === 'new' && (
        <WorkspaceNameDialog
          title="New workspace"
          suggestion={tabsApi.nextDefaultName()}
          existing={tabs.map(t => t.title)}
          onCancel={() => setDialog(null)}
          onConfirm={name => {
            addTab(name);
            setDialog(null);
          }}
        />
      )}
      {dialog?.kind === 'confirm-close' && (
        <ConfirmDialog
          title={`Close “${tabs.find(t => t.id === dialog.id)?.title ?? 'workspace'}”?`}
          message={lossOf(dialog.id)}
          confirmLabel="Close workspace"
          tone="danger"
          onCancel={() => setDialog(null)}
          onConfirm={() => finishClose(dialog.id)}
        />
      )}
      {dialog?.kind === 'replace-last' && (
        <WorkspaceNameDialog
          title={`Close “${tabs.find(t => t.id === dialog.id)?.title ?? 'workspace'}”`}
          warning={[lossOf(dialog.id), 'It is the only workspace, so a new one opens in its place.'].filter(Boolean).join(' ')}
          suggestion={tabsApi.nextDefaultName()}
          confirmLabel="Close and create"
          onCancel={() => setDialog(null)}
          onConfirm={name => finishClose(dialog.id, name)}
        />
      )}
    </div>
  );
}

export default function App() {
  return (
    <ErrorBoundary componentName="Application Root">
      <Router>
        <AppShell />
      </Router>
    </ErrorBoundary>
  );
}
