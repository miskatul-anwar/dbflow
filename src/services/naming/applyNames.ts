/**
 * Names every relation of every table after the engine has run (Rust: crates/dbflow-core/src/naming.rs).
 * The model sees column names only; a name the user typed always wins.
 */
import type { NameableRelation } from './relationNames';
import { engine } from '../../engine';

export type NamerFn = (
  items: Array<{ key: string; columns: string[] }>,
  onProgress?: (done: number, total: number) => void,
) => Promise<Record<string, string>>;

export interface NamableTable {
  tableName: string;
  primaryKey: string[];
  final: NameableRelation[];
  others: NameableRelation[][];
}

export interface ApplyNamesOptions {
  namer?: NamerFn | null;
  userNames?: Record<string, string>;
  onProgress?: (done: number, total: number) => void;
  onUnavailable?: (reason: string) => void;
}

const wire = (set: NameableRelation[]) => set.map(r => ({ ...r, rows: [] }));

function copyBack(target: NameableRelation[], named: NameableRelation[]): void {
  named.forEach((n, i) => {
    const r = target[i];
    r.name = n.name;
    r.displayName = n.displayName;
    r.nameSource = n.nameSource;
    r.sourceTable = n.sourceTable;
    r.foreignKeys.forEach((fk, j) => {
      fk.references = n.foreignKeys[j]?.references ?? fk.references;
    });
  });
}

export async function applyNames(tables: NamableTable[], opts: ApplyNamesOptions = {}): Promise<void> {
  const input = tables.map(t => ({ tableName: t.tableName, primaryKey: t.primaryKey, final: wire(t.final), others: t.others.map(wire) }));
  const plan = engine<{ chosen: Record<string, unknown>; toAsk: Array<{ key: string; columns: string[] }> }>('naming.plan', input, opts.userNames ?? {});
  let answers: Record<string, string> | null = null;
  if (opts.namer && plan.toAsk.length > 0) {
    try {
      answers = await opts.namer(plan.toAsk, opts.onProgress);
    } catch (e) {
      opts.onUnavailable?.((e as Error)?.message ?? String(e));
    }
  }
  const named = engine<Array<{ final: NameableRelation[]; others: NameableRelation[][] }>>('naming.apply', input, plan.chosen, plan.toAsk, answers);
  named.forEach((n, i) => {
    copyBack(tables[i].final, n.final);
    n.others.forEach((o, j) => copyBack(tables[i].others[j], o));
  });
}
