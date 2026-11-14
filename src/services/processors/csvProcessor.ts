/**
 * CSV parsing (delimiter sniffing, quoted fields, header repair, null tokens).
 * Implemented in Rust: crates/dbflow-core/src/csv.rs.
 */
import { engine, parseCsvBytes } from '../../engine';

export interface CSVParseOptions {
  delimiter?: string;
  maxRows?: number;
  preserveIndexColumns?: boolean;
  fileName?: string;
}

export interface CSVParseResult {
  data: Record<string, any>[];
  headers: string[];
  metadata: {
    originalRows: number;
    finalRows: number;
    originalColumns: number;
    finalColumns: number;
    fileName?: string;
    delimiter?: string;
    hasMultilineFields?: boolean;
  };
}

export interface StreamingCSVParserOptions {
  delimiter?: string;
  maxRows?: number;
  onRow: (row: Record<string, any>, index: number) => void;
  onHeaders?: (headers: string[]) => void;
}

export function sanitizeHeaders(raw: string[]): string[] {
  return engine('csv.sanitizeHeaders', raw.map(r => (typeof r === 'string' ? r : String(r ?? ''))));
}

/**
 * Incremental interface over the Rust large-file reader: chunks are collected and parsed when the
 * stream finishes, so quotes, escapes and CRLF pairs split across chunks are always handled.
 */
export class StreamingCSVParser {
  private chunks: string[] = [];
  private delimiter: string | null;
  private headers: string[] = [];
  private rowCount = 0;
  private readonly maxRows: number;
  private readonly onRow: StreamingCSVParserOptions['onRow'];
  private readonly onHeaders?: StreamingCSVParserOptions['onHeaders'];

  constructor(options: StreamingCSVParserOptions) {
    this.delimiter = options.delimiter || null;
    this.maxRows = options.maxRows ?? Infinity;
    this.onRow = options.onRow;
    this.onHeaders = options.onHeaders;
  }

  public feed(chunk: string): boolean {
    this.chunks.push(chunk);
    return true;
  }

  public finish(): void {
    const text = this.chunks.join('');
    this.chunks = [];
    const max = Number.isFinite(this.maxRows) ? this.maxRows : null;
    const r = engine<{ rows: Record<string, any>[]; headers: string[]; delimiter: string }>(
      'csv.parseStreaming',
      text,
      max,
      this.delimiter,
    );
    this.delimiter = r.delimiter;
    this.headers = r.headers;
    this.onHeaders?.(r.headers);
    r.rows.forEach(row => this.onRow(row, ++this.rowCount));
  }

  public getHeaders(): string[] {
    return this.headers;
  }
  public getDelimiter(): string {
    return this.delimiter || ',';
  }
  public getRowCount(): number {
    return this.rowCount;
  }
}

export class CSVProcessor {
  static readonly SUPPORTED_DELIMITERS = [',', ';', '\t', '|'] as const;
  static readonly StreamingParser = StreamingCSVParser;

  static sanitizeHeaders(raw: string[]): string[] {
    return sanitizeHeaders(raw);
  }

  static detectDelimiter(sample: string): string {
    if (!sample || typeof sample !== 'string') return ',';
    return engine('csv.detectDelimiter', sample);
  }

  static parseCSVCells(text: string, delimiter: string): { rows: { value: string; isQuoted: boolean }[][]; hasMultiline: boolean } {
    return engine('csv.parseCells', text, delimiter);
  }

  static parseCSVRows(text: string, delimiter: string): { rows: string[][]; hasMultiline: boolean } {
    const { rows, hasMultiline } = this.parseCSVCells(text, delimiter);
    return { rows: rows.map(r => r.map(c => c.value)), hasMultiline };
  }

  static parse(text: string, options: CSVParseOptions = {}): CSVParseResult {
    return engine('csv.parse', text ?? '', options);
  }

  static parseCSV(text: string, options: CSVParseOptions = {}): CSVParseResult {
    return this.parse(text, options);
  }

  static async processFile(file: File, options: CSVParseOptions = {}): Promise<CSVParseResult> {
    const bytes = new Uint8Array(await file.arrayBuffer());
    return parseCsvBytes<CSVParseResult>(bytes, { ...options, fileName: file.name });
  }

  static createStreamingParser(options: StreamingCSVParserOptions): StreamingCSVParser {
    return new StreamingCSVParser(options);
  }
}
