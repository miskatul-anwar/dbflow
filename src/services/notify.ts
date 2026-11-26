/**
 * Feedback, in two channels with separate jobs:
 *
 * - Toasts (in the app, via Sonner): the answer to something the user just did while looking at the app — a pipeline
 *   built from a request (with Undo), a file that could not be read, a check that stopped a run. Same message
 *   replaces itself instead of stacking.
 * - System notifications (the OS notification centre, with the app's logo): the outcome of work the user may not be
 *   watching — a run that finished or failed while DBFlow was in the background or minimised.
 *
 * `toast()` always stays in the app. `notify()` picks the channel: a toast when the user is looking at the app (naming
 * the workspace when the run belongs to another one), a system notification when they are away. Advisory `info` and
 * `warning` messages go to the console only, where a pipeline built in one go would otherwise raise one per step.
 */
import { toast as sonner } from 'sonner';

export type NotifyLevel = 'success' | 'error' | 'warning' | 'info';

export const APP_NAME = 'DBFlow';

type WorkspaceNameResolver = (workspaceId: string) => string | undefined;
let resolveWorkspaceName: WorkspaceNameResolver = () => undefined;
let activeWorkspace: string | undefined;

/** The app tells the notifier how to turn a workspace id into the tab's current name. */
export function setWorkspaceNameResolver(fn: WorkspaceNameResolver): void {
  resolveWorkspaceName = fn;
}

/** The workspace on screen: outcomes of the others name their workspace. */
export function setActiveWorkspace(id: string | undefined): void {
  activeWorkspace = id;
}

export interface NotificationContent {
  title: string;
  body: string;
}

const MAX_BODY = 240;
const clean = (s: string) => s.replace(/\s+/g, ' ').trim();

/** What the OS will show: the workspace name as the title, the message as the body. The app name is the OS's job. */
export function composeNotification(message: string, workspace?: string): NotificationContent {
  const name = workspace ? clean(workspace) : '';
  const body = clean(message);
  return {
    title: name || APP_NAME,
    body: body.length > MAX_BODY ? `${body.slice(0, MAX_BODY - 1)}…` : body,
  };
}

// ----- toasts --------------------------------------------------------------------------------------------------------

export interface ToastAction {
  label: string;
  onClick: () => void;
}

export interface ToastOptions {
  /** Shown under the message, e.g. the workspace a background run belongs to. */
  description?: string;
  action?: ToastAction;
}

/** How long a toast stays: errors until read (or 8 s), confirmations briefly, longer when they offer an action. */
export function toastDuration(level: NotifyLevel, hasAction: boolean): number {
  if (level === 'error') return 8000;
  return hasAction ? 6000 : 3500;
}

/** An in-app toast. Never throws. The same message replaces the one on screen instead of adding another. */
export function toast(level: NotifyLevel, message: string, opts: ToastOptions = {}): void {
  const text = clean(message ?? '');
  if (!text) return;
  const options = {
    id: `${level}\u0000${text}`,
    description: opts.description,
    duration: toastDuration(level, Boolean(opts.action)),
    action: opts.action,
  };
  try {
    if (level === 'success') sonner.success(text, options);
    else if (level === 'error') sonner.error(text, options);
    else if (level === 'warning') sonner.warning(text, options);
    else sonner(text, options);
  } catch {
    /* a toast is never worth an exception */
  }
}

// ----- system notifications ------------------------------------------------------------------------------------------

const isTauri = () => typeof window !== 'undefined' && '__TAURI_INTERNALS__' in window;
const isAndroid = () => typeof navigator !== 'undefined' && /android/i.test(navigator.userAgent);

/** The user is not looking at the app: it is hidden (another app, screen off, minimised) or another window has focus. */
export function isUserAway(): boolean {
  if (typeof document === 'undefined') return false;
  if (document.visibilityState === 'hidden') return true;
  return typeof document.hasFocus === 'function' && !document.hasFocus();
}

/**
 * Android draws the small icon as a white silhouette in the status bar, so it needs its own drawable
 * (res/drawable/ic_stat_dbflow.xml); the large icon is the full-colour logo. Without them Android shows a generic "i".
 */
export const ANDROID_ICONS = { icon: 'ic_stat_dbflow', largeIcon: 'ic_notification_large', iconColor: '#7c3aed' } as const;

let tauriReady: Promise<boolean> | null = null;
async function ensureTauriPermission(): Promise<boolean> {
  tauriReady ??= (async () => {
    try {
      const { isPermissionGranted, requestPermission } = await import('@tauri-apps/plugin-notification');
      if (await isPermissionGranted()) return true;
      return (await requestPermission()) === 'granted';
    } catch {
      return false;
    }
  })();
  return tauriReady;
}

async function show({ title, body }: NotificationContent, level: NotifyLevel): Promise<void> {
  if (isTauri()) {
    if (!(await ensureTauriPermission())) return;
    const { sendNotification } = await import('@tauri-apps/plugin-notification');
    // Desktop shells show the application icon themselves; Android needs the icons named.
    sendNotification(isAndroid() ? { title, body, ...ANDROID_ICONS, autoCancel: true } : { title, body });
    return;
  }
  if (typeof Notification === 'undefined') return;
  if (Notification.permission === 'default') {
    try {
      await Notification.requestPermission();
    } catch {
      return;
    }
  }
  if (Notification.permission !== 'granted') return;
  new Notification(title, { body, icon: `${import.meta.env.BASE_URL}icon.png`, tag: `${level}:${body}` });
}

/** The same message is not shown again within this time, and no workspace raises more than one per burst. */
export const REPEAT_WINDOW_MS = 5000;
export const BURST_WINDOW_MS = 1500;
const lastShown = new Map<string, number>(); // message key -> time
const lastBurst = new Map<string, number>(); // workspace and level -> time

/** Forget the rate-limit state (tests). */
export function resetNotifyLimits(): void {
  lastShown.clear();
  lastBurst.clear();
}

function systemNotify(level: NotifyLevel, content: NotificationContent, workspaceId?: string): void {
  const now = Date.now();
  const key = `${content.title}\u0000${content.body}`;
  // Bursts are counted per level, so an error is never hidden behind the success just before it.
  const scope = `${workspaceId ?? ''}\u0000${level}`;
  if (now - (lastShown.get(key) ?? -Infinity) < REPEAT_WINDOW_MS) return;
  if (now - (lastBurst.get(scope) ?? -Infinity) < BURST_WINDOW_MS) return;
  lastShown.set(key, now);
  lastBurst.set(scope, now);
  void show(content, level).catch(() => {});
}

/**
 * The outcome of work the user may not be watching (a run that finished or failed). Never throws, never blocks.
 * Looking at the app: a toast, with the workspace named when it is not the one on screen, and the action if given.
 * Away from the app: a system notification titled with the workspace name. `info` and `warning` stay in the console.
 */
export function notify(level: NotifyLevel, message: string, workspaceId?: string, opts: { action?: ToastAction } = {}): void {
  if (!message || !message.trim()) return;
  const workspace = workspaceId ? resolveWorkspaceName(workspaceId) : undefined;
  const content = composeNotification(message, workspace);
  console.log(`[${level}] ${content.title}: ${content.body}`);
  if (level === 'info' || level === 'warning') return;
  if (isUserAway()) {
    systemNotify(level, content, workspaceId);
    return;
  }
  const elsewhere = workspaceId !== undefined && activeWorkspace !== undefined && workspaceId !== activeWorkspace;
  toast(level, content.body, { description: elsewhere ? workspace : undefined, action: opts.action });
}
