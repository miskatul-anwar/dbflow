/**
 * Data cleaning, atomicity (multi-valued column) detection and type profiling.
 * Implemented in Rust: crates/dbflow-core/src/data_processor.rs.
 */
import { engine } from '../../engine';

export class DataProcessor {
  static async clean(data: any[]): Promise<any[]> {
    if (!data || !Array.isArray(data) || data.length === 0) return [];
    return engine('dataProcessor.clean', data.filter(r => r && typeof r === 'object'));
  }

  static async checkAtomicity(data: any[]): Promise<any> {
    if (!data || !Array.isArray(data) || data.length === 0) return { atomicity: {}, violations: {} };
    return engine('dataProcessor.checkAtomicity', data);
  }

  /** List items when `text` looks like a list (JSON array or short items split by ; | , or " / "), else null. */
  static listTokens(text: string): string[] | null {
    return engine('dataProcessor.listTokens', text);
  }

  static async profileDataTypes(data: any[]): Promise<any> {
    if (!data || !Array.isArray(data) || data.length === 0) return {};
    return engine('dataProcessor.profileDataTypes', data);
  }
}
