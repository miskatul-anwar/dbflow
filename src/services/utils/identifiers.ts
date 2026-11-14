/**
 * Identifier handling shared by the ERD, SQL and report emitters (Rust: crates/dbflow-core/src/identifiers.rs).
 */
import { engine } from '../../engine';

/** Keep word characters only; never empty, never starts with a digit. */
export function sanitizeIdentifier(raw: string): string {
  return engine('identifiers.sanitize', String(raw ?? ''));
}

/** Hands out a unique sanitized name per original; the same original always maps to the same name. */
export class IdentifierRegistry {
  private readonly used = new Set<string>();
  private readonly byOriginal = new Map<string, string>();

  assign(original: string): string {
    const known = this.byOriginal.get(original);
    if (known) return known;
    const candidate = this.unique(sanitizeIdentifier(original));
    this.byOriginal.set(original, candidate);
    return candidate;
  }

  fresh(base: string): string {
    return this.unique(sanitizeIdentifier(base));
  }

  get(original: string): string | undefined {
    return this.byOriginal.get(original);
  }

  private unique(base: string): string {
    let candidate = base;
    let n = 2;
    while (this.used.has(candidate.toLowerCase())) candidate = `${base}_${n++}`;
    this.used.add(candidate.toLowerCase());
    return candidate;
  }
}

/** Escape text for a Markdown table cell or inline span (CSV headers and values are untrusted). */
export function mdEscape(value: unknown): string {
  return engine('identifiers.mdEscape', String(value ?? ''));
}
