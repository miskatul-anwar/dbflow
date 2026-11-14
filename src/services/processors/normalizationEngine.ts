/**
 * Normalization engine: keys, closures, canonical cover, normal-form tests, 2NF/3NF/BCNF
 * decomposition, chase, dependency preservation, cardinalities and the single-file
 * ERD / relation-schema / SQL generators.
 *
 * Every algorithm is implemented in Rust (crates/dbflow-core/src/normalization.rs and legacy.rs);
 * this class keeps the original TypeScript API on top of it.
 */
import { engine } from '../../engine';
import type { SamplingInfo } from './fdMiner';

export interface FunctionalDependency {
  lhs: string[];
  rhs: string;
  confidence: number;
  coverage: number;
  violations: number;
  uniqueGroups: number;
}

export interface GroupedFunctionalDependency {
  lhs: string[];
  rhs: string[];
}

export interface CanonicalCoverResult {
  singletonCover: FunctionalDependency[];
  groupedCover: GroupedFunctionalDependency[];
  originalCount: number;
  canonicalCount: number;
  extraneousAttributesRemoved: number;
  redundantFDsRemoved: number;
}

export interface PrimeAttributeClassification {
  primeAttributes: string[];
  nonPrimeAttributes: string[];
  isPrime: (attr: string) => boolean;
}

export interface NormalFormViolation {
  type: '1NF_NON_ATOMIC' | '2NF_PARTIAL_DEPENDENCY' | '3NF_TRANSITIVE_DEPENDENCY' | 'BCNF_NON_SUPERKEY_DETERMINANT';
  culprit?: string;
  description: string;
  culpritFD?: { lhs: string[]; rhs: string };
  candidateKey?: string[];
  attributes?: string[];
  sampleValue?: any;
}

export interface NormalFormResult {
  nf: '1NF' | '2NF' | '3NF' | 'BCNF';
  isNormalized: boolean;
  violations: any[];
  violationDetails: NormalFormViolation[];
  primeAttributes?: string[];
  candidateKeys?: string[][];
  partialDependencies?: any[];
  transitiveDependencies?: any[];
  is1NF?: boolean;
  is2NF?: boolean;
  is3NF?: boolean;
  isBCNF?: boolean;
}

export interface NormalFormAssessment {
  highestNormalForm: 'UNNORMALIZED' | '1NF' | '2NF' | '3NF' | 'BCNF';
  details: { '1NF': NormalFormResult; '2NF': NormalFormResult; '3NF': NormalFormResult; BCNF: NormalFormResult };
}

export interface NormalForm1NFResult extends NormalFormResult {
  totalAttributes: number;
  atomicAttributes: string[];
  nonAtomicAttributes: string[];
}

export interface CandidateKey {
  attributes: string[];
  isMinimal: boolean;
}

export interface RelationSchema {
  name: string;
  attributes: string[];
  primaryKey: string[];
  foreignKeys: ForeignKey[];
  rows: any[];
  sourceType: string;
  metadata: { rowCount: number; attributeTypes: { [key: string]: string } };
  originalFD?: { lhs: string[]; rhs: string; confidence: number };
}

export interface ForeignKey {
  attributes: string[];
  references: string;
  referencedKey: string[];
  cardinality: string;
}

export interface Relationship {
  source: string;
  target: string;
  sourceKey: string[];
  targetKey: string[];
  cardinality: string;
  type: 'identifying' | 'non-identifying';
  strength: number;
}

export interface ChaseStepLog {
  iteration: number;
  fd: { lhs: string[]; rhs: string };
  equatedSymbol: string;
  replacedSymbol: string;
  affectedRows: number[];
}

export interface ChaseResult {
  isLossless: boolean;
  tableau: string[][];
  attributes: string[];
  iterations: number;
  fullyDistinguishedRowIndex: number;
  steps?: ChaseStepLog[];
}

export interface DependencyPreservationResult {
  isPreserved: boolean;
  preservationRatio: number;
  preserved: FunctionalDependency[];
  lost: FunctionalDependency[];
  totalDependencies: number;
  details: Array<{
    fd: FunctionalDependency;
    isPreserved: boolean;
    reason: 'single_relation' | 'projected_closure' | 'not_preserved';
    derivedClosure: string[];
  }>;
}

export interface BCNFDecompositionStep {
  sourceRelation: string;
  violatingFD: { lhs: string[]; rhs: string[] };
  decomposedRelations: [string, string];
}

export interface BCNFDecompositionResult {
  relations: RelationSchema[];
  isLossless: boolean;
  isDependencyPreserving: boolean;
  preservationDetails: DependencyPreservationResult;
  decompositionSteps: BCNFDecompositionStep[];
}

type AnyFD = { lhs: string[]; rhs: string | string[]; confidence?: number };

/** Violations come back as plain objects; the original API returned String objects carrying the details. */
function withStringViolations<T extends { violations: any[] }>(r: T): T {
  r.violations = r.violations.map(v => {
    if (typeof v === 'string' || !v || !v.__string) return v;
    const { __string: _flag, ...props } = v;
    return Object.assign(new String(props.description), props);
  });
  return r;
}

export class NormalizationEngine {
  static async mineFunctionalDependencies(data: any[], maxLhsSize: number = 4, mineOptions: Record<string, any> = {}): Promise<FunctionalDependency[]> {
    return (await this.mineFunctionalDependenciesDetailed(data, maxLhsSize, mineOptions)).fds;
  }

  static async mineFunctionalDependenciesDetailed(
    data: any[],
    maxLhsSize: number = 4,
    mineOptions: Record<string, any> = {},
  ): Promise<{ fds: FunctionalDependency[]; truncated: boolean; levelReached: number; maxLhsSize: number; sampling?: SamplingInfo; partitionCache?: { hits: number; misses: number } }> {
    if (!data || data.length === 0 || Object.keys(data[0] || {}).length < 2) {
      return { fds: [], truncated: false, levelReached: 0, maxLhsSize };
    }
    const { signal: _s, onProgress: _p, ...opts } = mineOptions;
    const r = engine<any>('nf.mineDetailed', data, maxLhsSize, opts);
    return { fds: r.fds, truncated: Boolean(r.truncated), levelReached: r.levelReached ?? 0, maxLhsSize, sampling: r.sampling, partitionCache: r.partitionCache };
  }

  static exactFDs(fds: FunctionalDependency[]): FunctionalDependency[] {
    return fds.filter(fd => fd.violations === 0 && fd.confidence >= 1);
  }

  static async findKeys(attributes: string[], fds: FunctionalDependency[]): Promise<{ candidateKeys: string[][]; superkeys: string[][]; searchMetrics?: any }> {
    return engine('nf.findKeys', attributes, fds);
  }

  public static findCandidateKeys(attributes: string[], fds: AnyFD[]): string[][] {
    return engine('nf.findCandidateKeys', attributes, fds);
  }

  public static computeClosure(attributes: string[], fds: Array<{ lhs: string[]; rhs: string | string[] }>): Set<string> {
    return new Set(engine<string[]>('nf.computeClosure', attributes, fds));
  }

  public static extractCandidateKeys(superkeys: string[][]): string[][] {
    return engine('nf.extractCandidateKeys', superkeys);
  }

  public static computeCanonicalCover(fds: FunctionalDependency[]): CanonicalCoverResult {
    return engine('nf.computeCanonicalCover', fds ?? []);
  }

  public static computeMinimalCover(fds: FunctionalDependency[]): FunctionalDependency[] {
    return this.computeCanonicalCover(fds).singletonCover;
  }

  public static classifyPrimeAttributes(attributes: string[], candidateKeys: string[][]): PrimeAttributeClassification {
    const r = engine<{ primeAttributes: string[]; nonPrimeAttributes: string[] }>('nf.classifyPrimeAttributes', attributes, candidateKeys);
    const prime = new Set(candidateKeys.flat());
    return { ...r, isPrime: (a: string) => prime.has(a) };
  }

  public static classifyAttributes(attributes: string[], candidateKeys: string[][]): { prime: string[]; nonPrime: string[] } {
    const { primeAttributes, nonPrimeAttributes } = this.classifyPrimeAttributes(attributes, candidateKeys);
    return { prime: primeAttributes, nonPrime: nonPrimeAttributes };
  }

  public static is1NF(rows: any[], attributes?: string[]): NormalForm1NFResult {
    return withStringViolations(engine('nf.is1NF', rows ?? [], attributes ?? null));
  }

  public static is2NF(attributes: string[], fds: FunctionalDependency[], candidateKeys?: string[][]): NormalFormResult {
    return withStringViolations(engine('nf.is2NF', attributes, fds, candidateKeys ?? null));
  }

  public static is3NF(attributes: string[], fds: FunctionalDependency[], candidateKeys?: string[][]): NormalFormResult {
    return withStringViolations(engine('nf.is3NF', attributes, fds, candidateKeys ?? null));
  }

  public static isBCNF(attributes: string[], fds: FunctionalDependency[], candidateKeys?: string[][]): NormalFormResult {
    return withStringViolations(engine('nf.isBCNF', attributes, fds, candidateKeys ?? null));
  }

  public static determineNormalForm(attributes: string[], fds: FunctionalDependency[], rows?: any[], candidateKeys?: string[][]): NormalFormAssessment {
    const r = engine<NormalFormAssessment>('nf.determineNormalForm', attributes, fds, rows ?? [], candidateKeys ?? null);
    for (const k of ['1NF', '2NF', '3NF', 'BCNF'] as const) withStringViolations(r.details[k]);
    return r;
  }

  static async apply1NF(data: any[], options?: { mode?: 'preserve_scalar' | 'unnest_repeating_groups'; multiValuedColumns?: string[]; delimiter?: RegExp | string }): Promise<any[]> {
    if (!data || data.length === 0) return [];
    const delimiter = typeof options?.delimiter === 'string' ? options.delimiter : null;
    return engine('nf.apply1NF', data, { ...options, delimiter });
  }

  public static hasAtomicViolations(value: string): boolean {
    return engine('nf.hasAtomicViolations', value ?? '');
  }

  public static extractAtomicValue(value: string): string {
    return value;
  }

  static async apply2NF(data: any[], fds: FunctionalDependency[]): Promise<RelationSchema[]> {
    return engine('nf.apply2NF', data ?? [], fds);
  }

  static async apply3NF(relations2NF: RelationSchema[], fds: FunctionalDependency[]): Promise<RelationSchema[]> {
    return engine('nf.apply3NF', relations2NF ?? [], fds);
  }

  static async synthesize3NF(fds: FunctionalDependency[], candidateKeys: string[][], data: any[], allAttributes?: string[]): Promise<RelationSchema[]> {
    return engine('nf.synthesize3NF', fds ?? [], candidateKeys ?? [], data ?? [], allAttributes ?? null);
  }

  public static async decomposeBCNF(relationOrAttributes: RelationSchema | string[], fds: FunctionalDependency[], data: any[] = []): Promise<RelationSchema[]> {
    return (await this.decomposeBCNFDetailed(relationOrAttributes, fds, data)).relations;
  }

  public static async decomposeBCNFDetailed(relationOrAttributes: RelationSchema | string[], fds: FunctionalDependency[], data: any[] = []): Promise<BCNFDecompositionResult> {
    return engine('nf.decomposeBCNFDetailed', relationOrAttributes, fds, data);
  }

  public static async applyBCNF(relations: RelationSchema[], fds: FunctionalDependency[]): Promise<RelationSchema[]> {
    return engine('nf.applyBCNF', relations ?? [], fds);
  }

  public static chaseAlgorithm(relations: RelationSchema[], fds: FunctionalDependency[]): ChaseResult {
    return engine('nf.chaseAlgorithm', relations ?? [], fds ?? []);
  }

  public static async checkLosslessJoin(relations: RelationSchema[], fds: FunctionalDependency[]): Promise<boolean> {
    return this.chaseAlgorithm(relations, fds).isLossless;
  }

  public static computeProjectedClosure(X: string[], relations: RelationSchema[], allFDs: FunctionalDependency[]): Set<string> {
    return new Set(engine<string[]>('nf.computeProjectedClosure', X, relations, allFDs));
  }

  public static checkDependencyPreservationDetails(relations: RelationSchema[], originalFDs: FunctionalDependency[]): DependencyPreservationResult {
    return engine('nf.checkDependencyPreservationDetails', relations ?? [], originalFDs ?? []);
  }

  public static async checkDependencyPreservation(relations: RelationSchema[], originalFDs: FunctionalDependency[]): Promise<boolean> {
    return this.checkDependencyPreservationDetails(relations, originalFDs).isPreserved;
  }

  static async analyzeCardinalities(data: any[]): Promise<{ [key: string]: any }> {
    return engine('nf.analyzeCardinalities', data ?? []);
  }

  public static extractRelationData(data: any[], attributes: string[]): any[] {
    return engine('nf.extractRelationData', data, attributes);
  }

  public static ensureCandidateKeyCoverage(relations: RelationSchema[], candidateKeys: string[][], data: any[], _relationNames: Set<string>): void {
    const out = engine<RelationSchema[]>('nf.ensureCandidateKeyCoverage', relations, candidateKeys, data);
    relations.push(...out.slice(relations.length));
  }

  public static removeRedundantRelationsPreservingLossless(relations: RelationSchema[], candidateKeys: string[][]): RelationSchema[] {
    const kept = new Set(engine<RelationSchema[]>('nf.removeRedundantRelationsPreservingLossless', relations, candidateKeys).map(r => r.name));
    return relations.filter(r => kept.has(r.name));
  }

  public static initializeTableau(relations: RelationSchema[], allAttributes: string[]): string[][] {
    return engine('nf.initializeTableau', relations, allAttributes);
  }

  public static hasAllEqualRow(tableau: string[][]): boolean {
    return engine('nf.hasAllEqualRow', tableau);
  }

  static async generateRelationSchemas(relations: RelationSchema[]): Promise<string> {
    return engine('nf.generateRelationSchemas', relations);
  }

  /** Single-file ERD. Like the original, it orders `relations` by importance in place. */
  static async generateERD(relations: RelationSchema[]): Promise<{ content: string; metadata: any }> {
    const r = engine<{ content: string; metadata: any; order: string[] }>('nf.generateERD', relations ?? []);
    if (relations && relations.length > 1) {
      const rank = new Map(r.order.map((n, i) => [n, i]));
      relations.sort((a, b) => (rank.get(a.name) ?? 0) - (rank.get(b.name) ?? 0));
    }
    return { content: r.content, metadata: r.metadata };
  }

  static async generateSQLDDL(relations: RelationSchema[]): Promise<string> {
    return engine('nf.generateSQLDDL', relations ?? []);
  }
}
