/**
 * Barker and UML class-diagram notations as Graphviz DOT (Rust: crates/dbflow-core/src/erd_dot.rs).
 */
import type { ChenRelation } from './chenErd';
import { engine } from '../../engine';

export interface BoxRelation extends ChenRelation {
  rows?: Array<Record<string, unknown>>;
  metadata?: { attributeTypes?: Record<string, string> };
}

export interface BoxOptions {
  attributes?: boolean;
}

/** At most ~2000 evenly spaced rows: the engine samples them the same way for optional columns. */
function sample(rows: Array<Record<string, unknown>> = []): Array<Record<string, unknown>> {
  const step = Math.max(1, Math.floor(rows.length / 2000));
  return step === 1 ? rows : rows.filter((_, i) => i % step === 0);
}

const wire = (r: BoxRelation) => ({ primaryKey: [], foreignKeys: [], ...r, rows: sample(r.rows) });

export function optionalColumns(rel: BoxRelation): Set<string> {
  return new Set(engine<string[]>('erd.optionalColumns', wire(rel)));
}

export function buildBarkerDot(relations: BoxRelation[], { attributes = true }: BoxOptions = {}): string {
  return engine('erd.barkerDot', relations.map(wire), { attributes });
}

export function buildUmlDot(relations: BoxRelation[], { attributes = true }: BoxOptions = {}): string {
  return engine('erd.umlDot', relations.map(wire), { attributes });
}
