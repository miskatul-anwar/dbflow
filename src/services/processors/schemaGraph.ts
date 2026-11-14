/**
 * Multi-table schema assembly, Mermaid ERD and DDL input from the normalized relations.
 * Implemented in Rust: crates/dbflow-core/src/schema.rs.
 */
import type { CrossTableRelationship, RelationSchema } from '../../types';
import type { DecomposedRelation } from '../api/databaseService';
import { engine } from '../../engine';

export interface TableSchemaInput {
  tableName: string;
  headers: string[];
  rows: any[];
  primaryKey: string[];
  relations: RelationSchema[];
}

export interface JunctionTable {
  relation: string;
  between: string[];
}

export interface BuiltSchema {
  relations: RelationSchema[];
  optionalFks: Set<string>;
  referencedUnique: Set<string>;
  unplaced: Array<{ link: CrossTableRelationship; reason: string }>;
  junctions: JunctionTable[];
}

type WireSchema = Omit<BuiltSchema, 'optionalFks' | 'referencedUnique'> & { optionalFks: string[]; referencedUnique: string[] };

const toWire = (s: BuiltSchema): WireSchema => ({ ...s, optionalFks: [...s.optionalFks], referencedUnique: [...s.referencedUnique] });

export function buildSchema(tables: TableSchemaInput[], cross: CrossTableRelationship[]): BuiltSchema {
  const w = engine<WireSchema>('schema.build', tables, cross);
  return { ...w, optionalFks: new Set(w.optionalFks), referencedUnique: new Set(w.referencedUnique) };
}

export function findJunctionTables(relations: RelationSchema[]): JunctionTable[] {
  return engine('schema.findJunctionTables', relations);
}

export function emitMermaidErd(schema: BuiltSchema): string {
  return engine('schema.emitMermaidErd', toWire(schema));
}

export function toDecomposedRelations(schema: BuiltSchema): DecomposedRelation[] {
  return engine('schema.toDecomposedRelations', toWire(schema));
}

export { toWire as schemaToWire };
