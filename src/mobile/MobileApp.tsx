/**
 * DBFlow for phones and tablets (the Android app): the desktop app's features in a touch layout.
 * Workspaces (switch, create, rename, colour, close) replace the tab strip; each workspace has three screens
 * (Data, Pipeline, Results) behind a bottom bar. Settings hold AI table names, the dark theme and a full reset.
 */
import { useCallback, useEffect, useRef, useState } from 'react';
import './mobile.css';
import { useWorkspaceTabs, type TabStatus } from '../hooks/useWorkspaceTabs';
import { useModelNaming } from '../hooks/useModelNaming';
import { modelNamingAvailable } from '../services/naming/userNames';
import { useTheme } from '../theme/theme';
import { setWorkspaceNameResolver } from '../services/notify';
import { clearCachedResult } from '../utils/resultCache';
import { MobileWorkspace } from './MobileWorkspace';
import { ConfirmSheet, WorkspaceForm, WorkspacesSheet } from './WorkspaceSheets';
import { Sheet, Switch, Toast, type ToastMessage } from './ui';

type Dialog =
  | { kind: 'workspaces' }
  | { kind: 'settings' }
  | { kind: 'new' }
  | { kind: 'edit'; id: string }
  | { kind: 'close'; id: string }
  | { kind: 'reset' }
  // As on the desktop: the first workspace is named on launch, and closing the last one names its replacement.
  | { kind: 'first' }
  | { kind: 'replace-last'; id: string };

export default function MobileApp() {
  const { tabs, activeId, nextDefaultName, status, addTab, closeTab, renameTab, setTabColor, suggestTitle, selectTab, setStatus } =
    useWorkspaceTabs();
  const [dialog, setDialog] = useState<Dialog | null>({ kind: 'first' });
  const [toast, setToast] = useState<ToastMessage | null>(null);
  const [aiNames, setAiNames] = useModelNaming();
  const { theme, toggle: toggleTheme } = useTheme();
  const say = useCallback((text: string, error = false) => setToast({ id: Date.now(), text, error }), []);
  const done = useCallback(() => setDialog(null), []);

  // System notifications name the workspace they are about.
  const tabsRef = useRef(tabs);
  tabsRef.current = tabs;
  useEffect(() => setWorkspaceNameResolver(id => tabsRef.current.find(t => t.id === id)?.title), []);

  const onStatus = useCallback((id: string, s: TabStatus) => setStatus(id, s), [setStatus]);

  const finishClose = useCallback(
    (id: string, replacementName?: string) => {
      setDialog(null);
      closeTab(id, replacementName);
      clearCachedResult(id);
    },
    [closeTab],
  );
  // Closing the last workspace asks for the name of the one that replaces it.
  const close = useCallback(
    (id: string) => {
      if (tabs.length === 1 && tabs[0].id === id) setDialog({ kind: 'replace-last', id });
      else finishClose(id);
    },
    [finishClose, tabs],
  );

  const editing = dialog?.kind === 'edit' ? tabs.find(t => t.id === dialog.id) : undefined;
  const closing = dialog?.kind === 'close' ? tabs.find(t => t.id === dialog.id) : undefined;

  return (
    <div className="m-app">
      {tabs.map(tab => (
        <MobileWorkspace
          key={tab.id}
          tab={tab}
          active={tab.id === activeId}
          say={say}
          onStatus={onStatus}
          onSuggestTitle={suggestTitle}
          onWorkspaces={() => setDialog({ kind: 'workspaces' })}
          onSettings={() => setDialog({ kind: 'settings' })}
        />
      ))}

      {dialog?.kind === 'workspaces' && (
        <WorkspacesSheet
          tabs={tabs}
          activeId={activeId}
          status={status}
          onSelect={id => {
            selectTab(id);
            done();
          }}
          onNew={() => setDialog({ kind: 'new' })}
          onEdit={id => setDialog({ kind: 'edit', id })}
          onClose={id => {
            const s = status[id];
            // Ask first when the workspace has work in it, as the desktop app does.
            if (s?.running || (s?.operations ?? 0) > 0) setDialog({ kind: 'close', id });
            else close(id);
          }}
          onDismiss={done}
        />
      )}
      {dialog?.kind === 'first' && tabs[0] && (
        <WorkspaceForm
          title="Name your workspace"
          initialName={tabs[0].title}
          withColor={false}
          confirmLabel="Start"
          onConfirm={name => {
            renameTab(tabs[0].id, name);
            done();
          }}
        />
      )}
      {dialog?.kind === 'replace-last' && (
        <WorkspaceForm
          title="Name the new workspace"
          note="This was the last workspace, so a new one replaces it."
          initialName={nextDefaultName()}
          withColor={false}
          confirmLabel="Close and start"
          onConfirm={name => finishClose(dialog.id, name)}
          onDismiss={done}
        />
      )}
      {dialog?.kind === 'new' && (
        <WorkspaceForm
          title="New workspace"
          initialName={nextDefaultName()}
          withColor={false}
          confirmLabel="Create"
          onConfirm={name => {
            addTab(name);
            done();
          }}
          onDismiss={done}
        />
      )}
      {editing && (
        <WorkspaceForm
          title="Edit workspace"
          initialName={editing.title}
          initialColor={editing.color}
          withColor
          confirmLabel="Save"
          onConfirm={(name, color) => {
            renameTab(editing.id, name);
            setTabColor(editing.id, color);
            done();
          }}
          onDismiss={done}
        />
      )}
      {closing && (
        <ConfirmSheet
          title={`Close “${closing.title}”?`}
          message={
            status[closing.id]?.running
              ? 'Its run is stopped, and its files, pipeline and results are discarded.'
              : 'Its files, pipeline and results are discarded.'
          }
          confirmLabel="Close workspace"
          onConfirm={() => close(closing.id)}
          onDismiss={done}
        />
      )}
      {dialog?.kind === 'settings' && (
        <Sheet title="Settings" onClose={done}>
          <div className="m-group">
            {modelNamingAvailable() ? (
              <div className="m-row">
                <span className="m-row-text">
                  <strong>AI table names</strong>
                </span>
                <Switch on={aiNames} onChange={setAiNames} label="AI table names" />
              </div>
            ) : (
              <div className="m-row">
                <span className="m-row-text">
                  <strong>AI table names</strong>
                </span>
                <span className="m-row-end">Desktop only</span>
              </div>
            )}
            <div className="m-row">
              <span className="m-row-text">
                <strong>Dark theme</strong>
              </span>
              <Switch on={theme === 'dark'} onChange={() => toggleTheme()} label="Dark theme" />
            </div>
            <button className="m-row" onClick={() => setDialog({ kind: 'reset' })}>
              <span className="m-row-text">
                <strong className="m-danger">Reset everything</strong>
              </span>
            </button>
          </div>
        </Sheet>
      )}
      {dialog?.kind === 'reset' && (
        <ConfirmSheet
          title="Reset everything?"
          message="Every workspace, its files, pipeline and results are cleared, and running pipelines stop."
          confirmLabel="Reset"
          onConfirm={() => window.location.reload()}
          onDismiss={done}
        />
      )}

      <Toast toast={toast} onDone={() => setToast(null)} />
    </div>
  );
}
