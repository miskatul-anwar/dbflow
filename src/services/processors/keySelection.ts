/**
 * Primary-key selection (identifier column, smallest clean candidate key, or a surrogate key).
 * Implemented in Rust: crates/dbflow-core/src/keys.rs.
 */
import { engine } from '../../engine';

export type KeyRule = 'identifier-column' | 'candidate-key' | 'surrogate';

export interface KeyChoice {
  primaryKey: string[];
  rule: KeyRule;
  reason: string;
  cleanKeys: string[][];
  surrogateAdded: boolean;
  surrogateColumn?: string;
}

export const SURROGATE_KEY_COLUMN = 'row_id';

export function isUniqueAndComplete(rows: any[], cols: string[]): boolean {
  return engine('keys.isUniqueAndComplete', rows, cols);
}

/**
 * Choose a primary key for `rows`. When a surrogate is needed it is written into `rows` and
 * `attributes` in place, so later steps see a real column.
 */
export function chooseKey(attributes: string[], rows: any[], candidateKeys: string[][], tableName = 'table'): KeyChoice {
  const r = engine<{ choice: KeyChoice; attributes: string[] }>('keys.chooseKey', attributes, rows, candidateKeys, tableName);
  if (r.choice.surrogateAdded && r.choice.surrogateColumn) {
    const column = r.choice.surrogateColumn;
    rows.forEach((row, i) => {
      row[column] = i + 1;
    });
    attributes.splice(0, attributes.length, ...r.attributes);
  }
  return r.choice;
}
