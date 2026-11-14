import { EntityTag, CrossTableRelationship, TableData } from '../../types';
import { engine } from '../../engine';
import { detectForeignKeys } from './foreignKeyDetector';

/** Column classification and cross-file foreign keys (Rust: crates/dbflow-core/src/relations.rs). */
export class RelationDetectorService {
  public static classifyColumnEntity(tableName: string, colName: string, sampleValues: any[]): EntityTag {
    return engine('semantic.classifyColumnEntity', tableName, colName, sampleValues);
  }

  public static performNER(tables: { tableName: string; headers: string[]; rows: any[]; rowCount?: number }[]): { [tableName: string]: EntityTag[] } {
    return engine('semantic.performNER', tables);
  }

  public static detectCrossTableRelationships(tables: TableData[]): CrossTableRelationship[] {
    return detectForeignKeys(tables.map(t => ({ tableName: t.tableName, headers: t.headers, rows: t.rows, primaryKey: t.primaryKey })));
  }
}
