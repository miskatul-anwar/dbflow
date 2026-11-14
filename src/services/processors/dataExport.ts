/**
 * Normalized data export: CSV per relation, INSERTs in dependency order, orphan checks.
 * Implemented in Rust: crates/dbflow-core/src/schema.rs.
 */
import type { DecomposedRelation, SQLDialect } from '../api/databaseService';
import type { BuiltSchema } from './schemaGraph';
import { schemaToWire } from './schemaGraph';
import { engine } from '../../engine';

export interface ExportedFile {
  name: string;
  content: string;
  type: 'csv' | 'sql';
}

export function csvCell(value: unknown): string {
  return engine('schema.csvCell', value === undefined ? null : value);
}

export function exportNormalizedData(schema: BuiltSchema, dialect: SQLDialect = 'sqlite'): ExportedFile[] {
  return engine('schema.exportNormalizedData', schemaToWire(schema), dialect);
}

export function orphanChecks(relations: DecomposedRelation[], dialect: SQLDialect = 'sqlite'): string {
  return engine('schema.orphanChecks', relations, dialect);
}
