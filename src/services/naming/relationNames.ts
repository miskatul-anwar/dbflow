/**
 * Relation names (source, user, model or placeholder). The engine never builds a name out of
 * column names; see crates/dbflow-core/src/naming.rs for the implementation.
 */
import { engine } from '../../engine';
import type { IdentifierRegistry } from '../utils/identifiers';

export type NameSource = 'source' | 'user' | 'model' | 'placeholder';

export interface NameableRelation {
  name: string;
  attributes: string[];
  primaryKey: string[];
  foreignKeys: Array<{ references: string }>;
  displayName?: string;
  nameSource?: NameSource;
  sourceTable?: string;
}

/** Identifies a relation by its content (table + columns + key), so a user's rename is found again. */
export function relationSignature(tableName: string, r: Pick<NameableRelation, 'attributes' | 'primaryKey'>): string {
  return engine('naming.relationSignature', tableName, { attributes: r.attributes, primaryKey: r.primaryKey });
}

export interface NamingRequest {
  signature: string;
  columns: string[];
}

export interface ChosenName {
  text: string;
  source: Exclude<NameSource, 'source' | 'placeholder'>;
}

/** Checks a model answer before it is used; null when it is not a usable name. */
export function cleanModelName(raw: string, columns: string[]): string | null {
  return engine('naming.cleanModelName', String(raw ?? ''), columns);
}

export interface NameSetOptions {
  tableName: string;
  primaryKey: string[];
  chosen?: Map<string, ChosenName>;
  /** Not used by the Rust engine (each call names one set with its own registry). */
  registry?: IdentifierRegistry;
}

/** The relation that keeps the table's own name. */
export function mainOf<T extends Pick<NameableRelation, 'attributes' | 'primaryKey'>>(set: T[], primaryKey: string[]): T | undefined {
  const i = engine<number | null>('naming.mainOf', set.map(r => ({ name: '', ...r, rows: [] })), primaryKey);
  return i === null ? undefined : set[i];
}

/** Gives every relation in one normal-form set its final name and rewrites foreign-key targets (in place). */
export function nameRelationSet(set: NameableRelation[], opts: NameSetOptions): void {
  if (set.length === 0) return;
  const chosen = Object.fromEntries(opts.chosen ?? []);
  const wire = set.map(r => ({ ...r, rows: [] }));
  const named = engine<NameableRelation[]>('naming.nameRelationSet', wire, { tableName: opts.tableName, primaryKey: opts.primaryKey, chosen });
  named.forEach((n, i) => {
    const r = set[i];
    r.name = n.name;
    r.displayName = n.displayName;
    r.nameSource = n.nameSource;
    r.sourceTable = n.sourceTable;
    r.foreignKeys.forEach((fk, j) => {
      fk.references = n.foreignKeys[j]?.references ?? fk.references;
    });
  });
}
