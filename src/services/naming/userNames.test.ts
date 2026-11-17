// @vitest-environment jsdom
import { afterEach, describe, expect, it } from 'vitest';
import { modelNamingAvailable, modelNamingEnabled, setModelNaming } from './userNames';

const androidUa = 'Mozilla/5.0 (Linux; Android 14; Pixel 8) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/120 Mobile Safari/537.36';

function inAndroidApp() {
  Object.defineProperty(window, '__TAURI_INTERNALS__', { configurable: true, value: {} });
  Object.defineProperty(navigator, 'userAgent', { configurable: true, value: androidUa });
}

afterEach(() => {
  delete (window as unknown as Record<string, unknown>).__TAURI_INTERNALS__;
  Reflect.deleteProperty(navigator, 'userAgent');
  localStorage.clear();
});

describe('AI table names on phones', () => {
  it('is available in the desktop app and the browser', () => {
    expect(modelNamingAvailable()).toBe(true);
    setModelNaming(true);
    expect(modelNamingEnabled()).toBe(true);
  });

  it('is never used in the Android app, even when switched on earlier', () => {
    setModelNaming(true);
    inAndroidApp();
    expect(modelNamingAvailable()).toBe(false);
    expect(modelNamingEnabled()).toBe(false);
  });
});
