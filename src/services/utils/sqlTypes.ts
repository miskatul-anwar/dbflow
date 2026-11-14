/**
 * SQL type inference for generated DDL (Rust: crates/dbflow-core/src/sql_types.rs).
 */
import { engine } from '../../engine';

export interface InferredColumn {
  sqlType: string;
  notNull: boolean;
}

export function inferColumnSqlType(values: unknown[]): InferredColumn {
  return engine('sqlTypes.inferColumnSqlType', values);
}

export function inferColumnTypes(rows: Record<string, unknown>[], attributes: string[]): Record<string, InferredColumn> {
  return engine('sqlTypes.inferColumnTypes', rows, attributes);
}
