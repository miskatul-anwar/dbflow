import { describe, expect, it } from 'vitest';
import { toBase64 } from './engineClient';

describe('toBase64', () => {
  it('matches the standard encoding for every length remainder and across slice boundaries', () => {
    for (const length of [0, 1, 2, 3, 4, 49151, 49152, 49153, 200_000]) {
      const bytes = Uint8Array.from({ length }, (_, i) => (i * 31 + 7) % 256);
      expect(toBase64(bytes)).toBe(Buffer.from(bytes).toString('base64'));
    }
  });
});
