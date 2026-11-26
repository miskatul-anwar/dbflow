// @vitest-environment jsdom
import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest';

const sendNotification = vi.fn();
const isPermissionGranted = vi.fn();
const requestPermission = vi.fn();
vi.mock('@tauri-apps/plugin-notification', () => ({ sendNotification, isPermissionGranted, requestPermission }));

const sonner = Object.assign(vi.fn(), { success: vi.fn(), error: vi.fn(), warning: vi.fn() });
vi.mock('sonner', () => ({ toast: sonner }));

async function load() {
  vi.resetModules();
  return import('./notify');
}
const flush = () => new Promise(r => setTimeout(r, 0));

/** The user is in another app (desktop: another window has focus; phone: DBFlow is in the background). */
const away = () => vi.spyOn(document, 'hasFocus').mockReturnValue(false);
const looking = () => vi.spyOn(document, 'hasFocus').mockReturnValue(true);

beforeEach(() => {
  sendNotification.mockReset();
  isPermissionGranted.mockReset().mockResolvedValue(true);
  requestPermission.mockReset().mockResolvedValue('granted');
  sonner.mockReset();
  sonner.success.mockReset();
  sonner.error.mockReset();
  sonner.warning.mockReset();
  vi.spyOn(console, 'log').mockImplementation(() => {});
  away();
});
afterEach(() => {
  delete (window as any).__TAURI_INTERNALS__;
  vi.unstubAllGlobals();
  vi.restoreAllMocks();
});

describe('toasts (in the app)', () => {
  it('shows the message once: the same text replaces the toast on screen', async () => {
    const { toast } = await load();
    toast('success', 'Pipeline built: 11 steps');
    toast('success', 'Pipeline built:  11 steps');
    expect(sonner.success).toHaveBeenCalledTimes(2);
    expect(sonner.success.mock.calls[0][1].id).toBe(sonner.success.mock.calls[1][1].id);
  });

  it('keeps errors longer and confirmations with an action longer than plain ones', async () => {
    const { toastDuration } = await load();
    expect(toastDuration('error', false)).toBeGreaterThan(toastDuration('success', true));
    expect(toastDuration('success', true)).toBeGreaterThan(toastDuration('success', false));
  });

  it('passes an action (Undo, View results) and ignores empty text', async () => {
    const { toast } = await load();
    const onClick = vi.fn();
    toast('success', 'Pipeline built: 3 steps', { action: { label: 'Undo', onClick } });
    expect(sonner.success).toHaveBeenCalledWith('Pipeline built: 3 steps', expect.objectContaining({ action: { label: 'Undo', onClick } }));
    toast('error', '   ');
    expect(sonner.error).not.toHaveBeenCalled();
  });

  it('never throws', async () => {
    sonner.error.mockImplementation(() => {
      throw new Error('boom');
    });
    const { toast } = await load();
    expect(() => toast('error', 'careful')).not.toThrow();
  });
});

describe('routing a run outcome', () => {
  beforeEach(() => {
    (window as any).__TAURI_INTERNALS__ = {};
  });

  it('is a toast, not a system notification, while the user is looking at the app', async () => {
    looking();
    const { notify, setActiveWorkspace, setWorkspaceNameResolver } = await load();
    setWorkspaceNameResolver(id => (id === 't1' ? 'Sales' : 'HR'));
    setActiveWorkspace('t1');
    notify('success', 'Pipeline completed for 2 table(s)', 't1');
    await flush();
    expect(sendNotification).not.toHaveBeenCalled();
    expect(sonner.success).toHaveBeenCalledWith('Pipeline completed for 2 table(s)', expect.objectContaining({ description: undefined }));
  });

  it('names the workspace when the run belongs to one that is not on screen, and carries the action', async () => {
    looking();
    const { notify, setActiveWorkspace, setWorkspaceNameResolver } = await load();
    setWorkspaceNameResolver(id => (id === 't1' ? 'Sales' : 'HR'));
    setActiveWorkspace('t1');
    const onClick = vi.fn();
    notify('error', 'Processing failed: disk full', 't2', { action: { label: 'View', onClick } });
    expect(sonner.error).toHaveBeenCalledWith('Processing failed: disk full', expect.objectContaining({ description: 'HR', action: { label: 'View', onClick } }));
  });

  it('is a system notification when the user is away', async () => {
    const { notify, setWorkspaceNameResolver } = await load();
    setWorkspaceNameResolver(() => 'Sales');
    notify('success', 'Pipeline completed for 2 table(s)', 't1');
    await flush();
    expect(sonner.success).not.toHaveBeenCalled();
    expect(sendNotification).toHaveBeenCalledWith({ title: 'Sales', body: 'Pipeline completed for 2 table(s)' });
  });

  it('names the logo icons on Android, where the system would otherwise show a generic one', async () => {
    vi.spyOn(navigator, 'userAgent', 'get').mockReturnValue('Mozilla/5.0 (Linux; Android 14; Pixel 7) AppleWebKit/537.36');
    vi.spyOn(document, 'visibilityState', 'get').mockReturnValue('hidden');
    const { notify, ANDROID_ICONS } = await load();
    notify('error', 'Processing failed');
    await flush();
    expect(sendNotification).toHaveBeenCalledWith(expect.objectContaining({ body: 'Processing failed', ...ANDROID_ICONS }));
    expect(ANDROID_ICONS.icon).toBe('ic_stat_dbflow');
  });
});

describe('notification content', () => {
  it('titles it with the workspace name only, and carries the message', async () => {
    const { composeNotification } = await load();
    expect(composeNotification('Pipeline completed for 2 table(s)', 'Sales data')).toEqual({
      title: 'Sales data',
      body: 'Pipeline completed for 2 table(s)',
    });
  });
  it('falls back to the app name, tidies whitespace and bounds the body', async () => {
    const { composeNotification } = await load();
    expect(composeNotification('a\n\n b').title).toBe('DBFlow');
    expect(composeNotification('a\n\n b').body).toBe('a b');
    expect(composeNotification('x'.repeat(1000), 'W').body.length).toBeLessThanOrEqual(240);
  });
});

describe('desktop (Tauri) delivery', () => {
  beforeEach(() => {
    (window as any).__TAURI_INTERNALS__ = {};
  });

  it('sends a system notification titled with the current workspace name', async () => {
    const { notify, setWorkspaceNameResolver } = await load();
    let name = 'Workspace 1';
    setWorkspaceNameResolver(id => (id === 't1' ? name : undefined));
    notify('success', 'Workflow saved (4 operations)', 't1');
    await flush();
    expect(sendNotification).toHaveBeenCalledWith({ title: 'Workspace 1', body: 'Workflow saved (4 operations)' });
    name = 'Chinook'; // a rename is picked up
    notify('error', 'Processing failed', 't1');
    await flush();
    expect(sendNotification).toHaveBeenLastCalledWith({ title: 'Chinook', body: 'Processing failed' });
  });

  it('asks for permission once and stays silent when it is refused', async () => {
    isPermissionGranted.mockResolvedValue(false);
    requestPermission.mockResolvedValue('denied');
    const { notify } = await load();
    notify('error', 'one');
    notify('error', 'two');
    await flush();
    await flush();
    expect(requestPermission).toHaveBeenCalledTimes(1);
    expect(sendNotification).not.toHaveBeenCalled();
  });

  it('does not notify for routine info messages or empty text, and never throws', async () => {
    const { notify } = await load();
    notify('info', 'Added 1NF with 4 prerequisites');
    notify('success', '   ');
    sendNotification.mockImplementation(() => {
      throw new Error('boom');
    });
    expect(() => notify('error', 'careful')).not.toThrow();
    await flush();
    expect(sendNotification).toHaveBeenCalledTimes(1);
  });

  it('keeps warnings in the console, and drops repeats and bursts', async () => {
    (window as any).__TAURI_INTERNALS__ = {};
    vi.useFakeTimers({ toFake: ['Date'] });
    try {
      const { notify, BURST_WINDOW_MS, REPEAT_WINDOW_MS } = await load();
      // a pipeline built from a request: one advisory warning per step, then the run's result
      for (let i = 0; i < 12; i++) notify('warning', `Step ${i} is usually run after another step`, 'w1');
      notify('success', 'Pipeline completed for 2 table(s)', 'w1');
      notify('success', 'Pipeline completed for 2 table(s)', 'w1'); // repeat
      notify('success', 'Workflow saved (4 operations)', 'w1'); // same burst, same level
      notify('error', 'Processing failed: disk full', 'w1'); // an error is never hidden behind a success
      notify('error', 'Processing failed: disk full', 'w1'); // repeat
      await flush();
      expect(sendNotification.mock.calls.map(c => c[0].body)).toEqual(['Pipeline completed for 2 table(s)', 'Processing failed: disk full']);
      // after the burst a new success shows; the same one waits out the repeat window
      vi.setSystemTime(Date.now() + BURST_WINDOW_MS + 1);
      notify('success', 'Pipeline completed for 2 table(s)', 'w1');
      await flush();
      expect(sendNotification).toHaveBeenCalledTimes(2);
      vi.setSystemTime(Date.now() + REPEAT_WINDOW_MS + 1);
      notify('success', 'Pipeline completed for 2 table(s)', 'w1');
      await flush();
      expect(sendNotification).toHaveBeenCalledTimes(3);
    } finally {
      vi.useRealTimers();
    }
  });
});

describe('browser fallback', () => {
  it('uses the Web Notifications API with the app icon', async () => {
    const ctor = vi.fn();
    class FakeNotification {
      static permission = 'granted';
      static requestPermission = vi.fn();
      constructor(title: string, opts: unknown) {
        ctor(title, opts);
      }
    }
    vi.stubGlobal('Notification', FakeNotification);
    const { notify, setWorkspaceNameResolver } = await load();
    setWorkspaceNameResolver(() => 'Workspace 2');
    notify('success', 'Pipeline completed', 't2');
    await flush();
    expect(ctor).toHaveBeenCalledWith('Workspace 2', expect.objectContaining({ body: 'Pipeline completed', icon: expect.stringMatching(/icon\.png$/) }));
  });

  it('does nothing visible when notifications are unavailable', async () => {
    vi.stubGlobal('Notification', undefined);
    const { notify } = await load();
    expect(() => notify('error', 'x')).not.toThrow();
    expect(document.body.textContent).toBe('');
  });
});
