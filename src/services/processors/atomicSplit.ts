/**
 * First normal form: move multi-valued columns into child tables `(key…, column)`.
 * Implemented in Rust: crates/dbflow-core/src/keys.rs.
 */
import type { RelationSchema } from '../../types';
import { engine } from '../../engine';

export interface AtomicSplit {
  parentRows: Record<string, unknown>[];
  parentAttributes: string[];
  key: string[];
  children: RelationSchema[];
  moved: string[];
}

export function splitMultiValuedColumns(rows: Record<string, unknown>[], multiValued: string[], tableName: string): AtomicSplit | null {
  if (rows.length === 0) return null;
  return engine('atomicSplit.split', rows, multiValued, tableName);
}
