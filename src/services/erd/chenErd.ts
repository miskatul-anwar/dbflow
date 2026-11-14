/**
 * Chen-notation ER diagram as Graphviz DOT (Rust: crates/dbflow-core/src/erd_dot.rs).
 * Names come from untrusted CSV headers: every string is escaped, node ids are generated.
 */
import { engine } from '../../engine';

export interface ChenRelation {
  name: string;
  attributes: string[];
  primaryKey?: string[];
  foreignKeys?: Array<{ attributes: string[]; references: string; referencedKey?: string[] }>;
}

export interface ChenOptions {
  tree?: boolean;
  attributes?: boolean;
}

const wire = (r: ChenRelation) => ({ primaryKey: [], foreignKeys: [], ...r, rows: [] });

export function isJunction(r: ChenRelation): boolean {
  return engine('erd.isJunction', wire(r));
}

export function isWeak(r: ChenRelation): boolean {
  return engine('erd.isWeak', wire(r));
}

export function buildChenDot(relations: ChenRelation[], { attributes = true, tree = false }: ChenOptions = {}): string {
  return engine('erd.chenDot', relations.map(wire), { attributes, tree });
}

export function attributeCount(relations: ChenRelation[]): number {
  return relations.reduce((n, r) => n + r.attributes.length, 0);
}
