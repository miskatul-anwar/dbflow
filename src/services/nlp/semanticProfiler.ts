/**
 * Rule-based semantic column profiler: fuses column-name evidence with value-pattern evidence.
 * Implemented in Rust: crates/dbflow-core/src/relations.rs.
 */
import type { EntityTag } from '../../types';
import { engine } from '../../engine';

export interface ColumnSampleStats {
  samples: string[];
  filled: number;
  distinct: number;
}

export const SAMPLE_LIMIT = 24;

export function tokenizeName(name: string): string[] {
  return engine('semantic.tokenizeName', name);
}

export function singularize(word: string): string {
  return engine('semantic.singularize', word);
}

export function looksLikeDate(v: string): boolean {
  return engine('semantic.looksLikeDate', v);
}

export interface ClassifyInput {
  tableName: string;
  columnName: string;
  columnIndex: number;
  stats: ColumnSampleStats;
}

export function classifyColumn(input: ClassifyInput): EntityTag {
  return engine('semantic.classifyColumn', input);
}
