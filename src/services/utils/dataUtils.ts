/**
 * Value-level utilities. The implementations live in the Rust engine (crates/dbflow-core/src/data_utils.rs).
 */
import { engine } from '../../engine';

export interface IndexColumnOptions {
  /** Allow treating identifier-named columns (id, sn, no, idx) as index artifacts when strictly sequential. */
  allowDiscardPrimaryKey?: boolean;
}

export class DataUtils {
  static normalizeName(name: string): string {
    if (!name || typeof name !== 'string') return '';
    return engine('dataUtils.normalizeName', name);
  }

  static isIndexColumn(columnName: string, values?: any[], options?: IndexColumnOptions): boolean {
    return engine('dataUtils.isIndexColumn', typeof columnName === 'string' ? columnName : null, values ?? null, options ?? {});
  }

  static inferDataType(values: any[]): string {
    if (!values || values.length === 0) return 'string';
    return engine('dataUtils.inferDataType', values);
  }

  static cleanValue(value: any): string {
    return engine('dataUtils.cleanValue', value === undefined ? null : value);
  }

  static isAtomic(value: any): boolean {
    return engine('dataUtils.isAtomic', value === undefined ? null : value);
  }

  static getUniqueValues(values: any[]): any[] {
    return engine('dataUtils.getUniqueValues', values);
  }

  static getValueFrequency(values: any[]): Map<any, number> {
    const pairs: Array<[string, number]> = engine('dataUtils.getValueFrequency', values);
    return new Map(pairs);
  }

  static analyzeDataQuality(values: any[]): {
    totalCount: number;
    nullCount: number;
    uniqueCount: number;
    completeness: number;
    uniqueness: number;
    mostFrequent: { value: any; count: number };
  } {
    return engine('dataUtils.analyzeDataQuality', values);
  }

  static generateRowHash(row: any, attributes: string[]): string {
    return engine('dataUtils.generateRowHash', row ?? {}, attributes);
  }

  static removeDuplicates(data: any[], attributes: string[]): any[] {
    return engine('dataUtils.removeDuplicates', data, attributes);
  }

  static isValidAttributeName(name: string): boolean {
    return engine('dataUtils.isValidAttributeName', typeof name === 'string' ? name : null);
  }

  static suggestValidAttributeName(name: string): string {
    return engine('dataUtils.suggestValidAttributeName', typeof name === 'string' ? name : null);
  }

  static formatValueForDisplay(value: any, maxLength: number = 50): string {
    return engine('dataUtils.formatValueForDisplay', value === undefined ? null : value, maxLength);
  }

  static parseDelimitedValue(value: string, delimiter: string = ','): string[] {
    if (!value || typeof value !== 'string') return [];
    return engine('dataUtils.parseDelimitedValue', value, delimiter);
  }

  static areValuesEqual(value1: any, value2: any): boolean {
    return engine('dataUtils.areValuesEqual', value1 ?? null, value2 ?? null);
  }

  static calculateNumericStats(values: any[]): {
    min: number;
    max: number;
    mean: number;
    median: number;
    stdDev: number;
    count: number;
  } | null {
    return engine('dataUtils.calculateNumericStats', values);
  }
}
