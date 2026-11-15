/**
 * Database normalization service: the UI-facing API of the engine.
 *
 * All parsing and computation runs in the Rust engine (crates/dbflow-core): natively in the
 * desktop app (see `engineClient.ts`), and as WebAssembly in the browser, in Web Workers and in
 * tests. This module only moves files and results across that boundary and asks the on-device
 * naming model for relation names between the two phases of a run.
 */
import { engine, enginePrepare, engineFinish, engineDiscard, type EngineFile } from '../../engine';
import { expandSpreadsheets, isSpreadsheetFile, isSupportedInputName, type SkippedInput } from '../processors/spreadsheetProcessor';
import { FileUtils } from '../utils/fileUtils';
import type { NamerFn } from '../naming/applyNames';
import { TableData } from '../../types';

export interface ProcessingOptions {
  maxLhsSize?: number;
  maxSampleSize?: number;
  /** Rows used to propose dependencies; every reported one is verified on all rows. */
  fdSampleSize?: number;
  /** Skip determinants that are nearly (but not exactly) unique on tables of 200+ rows; 1 disables. Default 0.9. */
  nearKeyRatio?: number;
  /** Never use columns with missing values as determinants (they cannot be keys). Default true. */
  completeDeterminantsOnly?: boolean;
  /** Below this many rows only superkey determinants are used (default 10). */
  minRowsForNonKeyDeterminants?: number;
  /** Time budget for functional-dependency mining, in milliseconds (default 30 000). */
  timeout?: number;
  enableParallelProcessing?: boolean;
  adaptiveThresholds?: boolean;
  /** Use dependencies that hold on most but not all rows for keys and normal forms (off by default). */
  acceptApproximateFDs?: boolean;
  /** SQL dialect for multi-table DDL (default `sqlite`). */
  sqlDialect?: SQLDialect;
  /** Writes relation names (the on-device model). Absent in tests and the CLI: relations stay "unnamed". */
  namer?: NamerFn | null;
  /** `model`: the on-device model names new relations; `rules`: names are built from the key columns (no model). */
  namingMode?: 'model' | 'rules';
  /** Files analysed at once in a multi-file run (desktop app; default: one per CPU core). */
  maxParallelFiles?: number;
  /** Names the user typed, by relation signature (see `relationSignature`). They always win over the model. */
  userNames?: Record<string, string>;
  /** Merge files with the same columns and mostly the same rows into one table (default: analyse them separately). */
  mergeOverlappingFiles?: boolean;
}

export interface TableAnalysis {
  rows: any[];
  attributes: string[];
  atomicityResult: any;
  dataProfile: any;
  nerTags: any[];
  functionalDependencies: any[];
  usedFDs: any[];
  fdSearch: {
    truncated: boolean;
    levelReached: number;
    maxLhsSize: number;
    exact: number;
    approximate: number;
    approximateUsed: boolean;
  } | null;
  keyResult: any;
  keyChoice: any;
  relations1NF: any[];
  relations2NF: any[];
  relations3NF: any[];
  relationsBCNF: any[];
  cardinalityAnalysis: any;
  losslessCheck: any;
  dependencyPreservation: any;
  notes: string[];
}

export interface ProcessingResult {
  success: boolean;
  stats: {
    originalRows?: number;
    cleanedRows?: number;
    dataRetention?: string;
    attributes?: number;
    functionalDependencies?: number;
    candidateKeys?: number;
    superkeys?: number;
    relations1NF?: number;
    relations2NF?: number;
    relations3NF?: number;
    relationsBCNF?: number;
    connectedEntities?: number;
    relationships?: number;
    cardinalityMappings?: number;
    losslessJoin?: boolean;
    dependencyPreservation?: boolean;
    processDuration?: string;
    memoryUsage?: string;
    performanceMetrics?: any;
  };
  outputFiles: OutputFile[];
  logs: string[];
  relations?: any[];
  relations1NF?: any[];
  relations2NF?: any[];
  relations3NF?: any[];
  relationsBCNF?: any[];
  bcnf?: any[];
  relationships?: any[];
  erdContent?: string;
  erdDiagram?: string;
  sqlStatements?: string;
  reportMarkdown?: string;
  cardinalityAnalysis?: any;
  losslessCheck?: any;
  dependencyPreservation?: any;
  duration: string;
  selectedOperations?: string[];
  isMultiTable?: boolean;
  tables?: TableData[];
  crossTableRelationships?: any[];
  nerResults?: any;
  error?: string;
  /** `rust-native` (desktop app) or `rust-wasm` (browser, worker, tests). */
  engine?: string;
  qualityAssessment?: {
    score: number;
    issues: string[];
    recommendations: string[];
  };
}

export interface OutputFile {
  name: string;
  type: 'json' | 'markdown' | 'csv' | 'sql' | 'mermaid' | 'txt' | 'svg';
  description: string;
  content: any;
  size: number;
  category: string;
}

export interface ProcessingState {
  phase: 'PREP' | 'ANALYZE' | 'NORMAL' | 'OUTPUT';
  operation: string;
  progress: number;
  message: string;
  estimatedTimeRemaining?: number;
  memoryUsage?: number;
}

export type SQLDialect = 'postgresql' | 'mysql' | 'sqlite' | 'ansi';

export interface DecomposedRelation {
  name: string;
  attributes: string[];
  primaryKey: string[];
  foreignKeys?: {
    attributes: string[];
    references: string;
    referencedKey: string[];
    cardinality?: string;
  }[];
  fds?: any[];
  columnTypes?: Record<string, string>;
  /** Columns with no missing value in the data; emitted as NOT NULL. */
  notNull?: Record<string, boolean>;
  /** Non-key columns that other tables reference; emitted as UNIQUE. */
  unique?: string[];
  sourceName?: string;
  columnSources?: Record<string, string>;
}

export const SQL_RESERVED_KEYWORDS = new Set([
  'ADD', 'ALL', 'ALTER', 'AND', 'AS', 'ASC', 'BETWEEN', 'BY', 'CASE', 'CAST',
  'CHECK', 'COLUMN', 'CONSTRAINT', 'CREATE', 'CROSS', 'CURRENT_DATE', 'CURRENT_TIME',
  'CURRENT_TIMESTAMP', 'DATABASE', 'DATE', 'DAY', 'DEFAULT', 'DELETE', 'DESC',
  'DISTINCT', 'DROP', 'ELSE', 'END', 'EXISTS', 'FOREIGN', 'FROM', 'FULL', 'GRANT',
  'GROUP', 'HAVING', 'IN', 'INDEX', 'INNER', 'INSERT', 'INTO', 'IS', 'JOIN', 'KEY',
  'LEFT', 'LIKE', 'LIMIT', 'MONTH', 'NOT', 'NULL', 'OFFSET', 'ON', 'OR', 'ORDER',
  'OUTER', 'PRIMARY', 'REFERENCES', 'REVOKE', 'RIGHT', 'ROW', 'ROWS', 'SCHEMA',
  'SELECT', 'SET', 'TABLE', 'THEN', 'TIME', 'TIMESTAMP', 'TO', 'TRIGGER', 'UNION',
  'UNIQUE', 'UPDATE', 'USER', 'VALUE', 'VALUES', 'VIEW', 'WHEN', 'WHERE', 'WITH',
  'YEAR'
]);

export function quoteIdentifier(ident: string, dialect: SQLDialect = 'ansi'): string {
  return engine('ddl.quoteIdentifier', ident ?? '', dialect);
}

export function mapDataTypeToDialect(inferredType: string, dialect: SQLDialect = 'ansi'): string {
  return engine('ddl.mapDataTypeToDialect', inferredType ?? '', dialect);
}

/** Files ready for the engine, with notes and the workbooks that could not be converted. */
export interface PreparedEngineFiles {
  files: EngineFile[];
  notes: string[];
  skipped: SkippedInput[];
}

/**
 * Read files into bytes for the engine, converting workbooks the Rust reader cannot open itself. A workbook that
 * cannot be read is left out and reported; the engine decides whether anything is left to analyse.
 */
export async function prepareEngineFiles(files: File[]): Promise<PreparedEngineFiles> {
  const notes: string[] = [];
  const skipped: SkippedInput[] = [];
  const expanded = await expandSpreadsheets(files, n => notes.push(n), s => skipped.push(s));
  const out: EngineFile[] = [];
  for (const f of expanded) out.push({ name: f.name, bytes: new Uint8Array(await f.arrayBuffer()) });
  return { files: out, notes, skipped };
}

/** Options as the engine reads them (functions removed, local time zone added for log stamps). */
export function engineOptions(options: ProcessingOptions, prepared: Pick<PreparedEngineFiles, 'notes' | 'skipped'> = { notes: [], skipped: [] }): Record<string, unknown> {
  const { namer: _namer, ...rest } = options;
  return { ...rest, inputNotes: prepared.notes, skippedInputs: prepared.skipped, tzOffsetMinutes: -new Date().getTimezoneOffset() };
}

function failure(message: string, operations: string[]): ProcessingResult {
  return {
    success: false,
    error: message,
    stats: { processDuration: '0.00s' },
    outputFiles: [],
    logs: [`[ERROR] ${message}`],
    duration: '0.00s',
    selectedOperations: operations,
  };
}

/** Ask the naming model; returns its raw answers or the reason it could not be used. */
export async function askNamer(
  namer: NamerFn | null | undefined,
  items: Array<{ key: string; columns: string[] }>,
  onProgress?: (state: ProcessingState) => void,
): Promise<{ answers: Record<string, string> | null; error: string | null }> {
  if (!namer || items.length === 0) return { answers: null, error: null };
  try {
    const answers = await namer(items, (done, total) =>
      onProgress?.({ phase: 'NORMAL', operation: 'naming', progress: 88, message: `Naming relations ${done}/${total}…` }),
    );
    return { answers, error: null };
  } catch (e) {
    return { answers: null, error: (e as Error)?.message ?? String(e) };
  }
}

export class DatabaseService {
  public formatCSVCell(val: any): string {
    return DatabaseService.formatCSVCell(val);
  }

  public formatCSVHeader(header: string): string {
    return DatabaseService.formatCSVHeader(header);
  }

  /** RFC 4180 cell formatting (keeps 0 and false, escapes commas, quotes and line breaks). */
  public static formatCSVCell(val: any): string {
    return engine('ddl.formatCSVCell', val === undefined ? null : val);
  }

  public static formatCSVHeader(header: string): string {
    return engine('ddl.formatCSVHeader', header ?? null);
  }

  public static generateTableDDL(relation: DecomposedRelation, dialect: SQLDialect = 'mysql'): string {
    return engine('ddl.generateTableDDL', relation, dialect);
  }

  public static generateDDL(relations: DecomposedRelation[], dialect: SQLDialect = 'mysql'): string {
    return engine('ddl.generateDDL', relations, dialect);
  }

  public generateDDL(relations: DecomposedRelation[], dialect: SQLDialect = 'mysql'): string {
    return DatabaseService.generateDDL(relations, dialect);
  }

  public generateTableDDL(relation: DecomposedRelation, dialect: SQLDialect = 'mysql'): string {
    return DatabaseService.generateTableDDL(relation, dialect);
  }

  async processFile(
    file: File,
    selectedOperations: string[],
    onProgress?: (state: ProcessingState) => void,
    options: ProcessingOptions = {},
  ): Promise<ProcessingResult> {
    return this.processFiles([file], selectedOperations, onProgress, options);
  }

  /**
   * Run the pipeline on one or more files with the Rust engine (WebAssembly in this context).
   * One file gets the single-table outputs; several files are joined into one schema.
   */
  async processFiles(
    files: File[],
    selectedOperations: string[],
    onProgress?: (state: ProcessingState) => void,
    options: ProcessingOptions = {},
  ): Promise<ProcessingResult> {
    let session: number | null = null;
    try {
      const prepared = await prepareEngineFiles(files ?? []);
      const progress = (p: unknown) => onProgress?.(p as ProcessingState);
      const run = enginePrepare(prepared.files, selectedOperations, engineOptions(options, prepared), progress);
      session = run.session;
      const { answers, error } = await askNamer(options.namer, run.naming, onProgress);
      const result = engineFinish<ProcessingResult>(run.session, answers, error, progress);
      session = null;
      return result;
    } catch (err) {
      return failure((err as Error)?.message || 'Pipeline execution failed.', selectedOperations);
    } finally {
      if (session !== null) engineDiscard(session);
    }
  }

  /** Runs are cancelled by stopping their worker or native task (see engineClient). */
  cancelProcessing(): void {}

  async downloadFile(file: OutputFile): Promise<void> {
    const content =
      typeof file.content === 'string'
        ? file.content
        : file.type === 'csv'
          ? engine<string>('ddl.rowsToCsv', file.content)
          : JSON.stringify(file.content, null, 2);
    FileUtils.downloadAsFile(content, file.name, this.getMimeType(file.type));
  }

  private getMimeType(fileType: string): string {
    const mimeTypes: Record<string, string> = {
      json: 'application/json',
      markdown: 'text/markdown',
      mermaid: 'text/plain',
      sql: 'application/sql',
      csv: 'text/csv',
      txt: 'text/plain',
      svg: 'image/svg+xml',
    };
    return mimeTypes[fileType] || 'text/plain';
  }

  async validateFile(file: File): Promise<{ valid: boolean; error?: string }> {
    try {
      if (file.size > 100 * 1024 * 1024) {
        return { valid: false, error: 'File size exceeds 100MB limit. Consider using streaming for larger files.' };
      }
      if (!isSupportedInputName(file.name, file.type)) {
        return { valid: false, error: 'Only CSV and spreadsheet files (Excel, OpenDocument) are supported' };
      }
      if (await isSpreadsheetFile(file)) return { valid: true };
      const sample = await file.slice(0, 2048).text();
      if (!sample.includes(',')) return { valid: false, error: 'File does not appear to be a valid CSV' };
      return { valid: true };
    } catch {
      return { valid: false, error: 'Failed to validate file' };
    }
  }
}

const databaseServiceInstance = new DatabaseService();
(databaseServiceInstance as any).formatCSVCell = DatabaseService.formatCSVCell;
(databaseServiceInstance as any).formatCSVHeader = DatabaseService.formatCSVHeader;
(databaseServiceInstance as any).generateDDL = DatabaseService.generateDDL;
(databaseServiceInstance as any).generateTableDDL = DatabaseService.generateTableDDL;

export default databaseServiceInstance;
