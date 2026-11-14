/**
 * Instance-level verification of a decomposition: re-join the projections of the data and
 * compare with the input. Implemented in Rust: crates/dbflow-core/src/keys.rs.
 */
import { engine } from '../../engine';

export interface InstanceLosslessResult {
  lossless: boolean | null;
  inputRows: number;
  joinedRows: number;
  spuriousRows: number;
  missingRows: number;
  note?: string;
}

export function verifyLosslessOnData(rows: Record<string, unknown>[], relationAttrs: string[][], maxJoinRows = 250_000): InstanceLosslessResult {
  return engine('instance.verifyLosslessOnData', rows, relationAttrs, maxJoinRows);
}
