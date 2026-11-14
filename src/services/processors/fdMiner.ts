/**
 * Functional-dependency mining by partition refinement (TANE-style), plus sampled mining with
 * full-data validation. Implemented in Rust: crates/dbflow-core/src/fd_miner.rs.
 */
import { engine } from '../../engine';

export interface MinedFD {
  lhs: string[];
  rhs: string;
  confidence: number;
  coverage: number;
  violations: number;
  uniqueGroups: number;
}

export type NullSemantics = 'equal' | 'distinct';

export interface MineOptions {
  maxLhsSize?: number;
  minConfidence?: number;
  nullSemantics?: NullSemantics;
  nearKeyRatio?: number;
  completeDeterminantsOnly?: boolean;
  nearKeyMinRows?: number;
  minRowsForNonKeyDeterminants?: number;
  /** Ignored here: a run is cancelled by stopping its worker or the native task. */
  signal?: AbortSignal;
  timeBudgetMs?: number;
  /** Ignored by the Rust miner (progress is reported per pipeline step). */
  onProgress?: (level: number, fdsFound: number) => void;
}

export interface MineResult {
  fds: MinedFD[];
  durationMs: number;
  combinationsEvaluated: number;
  exactCount: number;
  truncated?: boolean;
  levelReached?: number;
  partitionCache?: { hits: number; misses: number };
}

export interface SamplingInfo {
  sampled: true;
  totalRows: number;
  sampleSize: number;
  rounds: number;
  rowsAdded: number;
  refuted: number;
}

export interface SampledMineOptions extends MineOptions {
  sampleSize?: number;
  maxRounds?: number;
  seed?: number;
}

export interface FDValidation {
  fd: MinedFD;
  witness: number[];
}

/** An already-aborted signal becomes an exhausted time budget, so the result is flagged truncated. */
const opts = ({ signal, onProgress: _p, ...rest }: SampledMineOptions) => (signal?.aborted ? { ...rest, timeBudgetMs: -1 } : rest);

export function mineFunctionalDependencies(data: Record<string, unknown>[], options: MineOptions = {}): MineResult {
  if (!Array.isArray(data) || data.length === 0) return { fds: [], durationMs: 0, combinationsEvaluated: 0, exactCount: 0 };
  return engine('fd.mine', data, opts(options));
}

export function reservoirSampleIndices(total: number, size: number, seed = 42): number[] {
  return engine('fd.reservoirSampleIndices', total, size, seed);
}

export function validateFunctionalDependencies(
  data: Record<string, unknown>[],
  fds: MinedFD[],
  nullSemantics: NullSemantics = 'equal',
): FDValidation[] {
  return engine('fd.validate', data, fds, nullSemantics);
}

export function mineFunctionalDependenciesSampled(
  data: Record<string, unknown>[],
  options: SampledMineOptions = {},
): MineResult & { sampling?: SamplingInfo } {
  if (!Array.isArray(data) || data.length === 0) return { fds: [], durationMs: 0, combinationsEvaluated: 0, exactCount: 0 };
  return engine('fd.mineSampled', data, opts(options));
}
