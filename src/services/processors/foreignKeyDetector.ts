/**
 * Cross-table foreign-key detection from inclusion dependencies scored by containment, coverage,
 * name similarity and type. Implemented in Rust: crates/dbflow-core/src/relations.rs.
 */
import type { CrossTableRelationship } from '../../types';
import { engine } from '../../engine';

export interface DetectorTable {
  tableName: string;
  headers: string[];
  rows: Record<string, unknown>[];
  primaryKey?: string[];
}

export interface DetectorOptions {
  minContainment?: number;
  minScore?: number;
}

export function tokenizeName(name: string): string[] {
  return engine('fk.tokenizeName', name);
}

export function singularize(word: string): string {
  return engine('fk.singularize', word);
}

export function detectForeignKeys(tables: DetectorTable[], options: DetectorOptions = {}): CrossTableRelationship[] {
  return engine('fk.detect', tables, options);
}
