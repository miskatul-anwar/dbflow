/**
 * The phone app's pipeline: a set of operations, always closed under dependencies, shown as a list
 * instead of a canvas. Goals give a whole pipeline in one tap; tapping a step adds it with what it needs,
 * or removes it with what needs it, so the pipeline is valid after every tap.
 */
import { OPERATIONS } from '../config/operations';
import { dependencyClosure, planFor } from '../services/nlp/workflow/planner';

export type Phase = 'PREP' | 'ANALYZE' | 'NORMAL' | 'OUTPUT';

export interface Goal {
  id: string;
  title: string;
  blurb: string;
  targets: string[];
}

/** One tap, one sensible pipeline. */
export const GOALS: Goal[] = [
  {
    id: '3nf',
    title: '3NF schema',
    blurb: 'Normalized tables, checks, diagram, SQL, report',
    targets: ['3NF', 'Relation Detector', 'Lossless Check', 'Dependency Preservation', 'ERD Generator', 'SQL Generator', 'Report Generator'],
  },
  {
    id: 'bcnf',
    title: 'BCNF schema',
    blurb: 'Stricter than 3NF; may lose dependencies',
    targets: ['BCNF', 'Relation Detector', 'Lossless Check', 'Dependency Preservation', 'ERD Generator', 'SQL Generator', 'Report Generator'],
  },
  {
    id: '4nf',
    title: '4NF schema',
    blurb: 'BCNF, then splits multivalued dependencies',
    targets: ['4NF', 'Relation Detector', 'Lossless Check', 'Dependency Preservation', 'ERD Generator', 'SQL Generator', 'Report Generator'],
  },
  {
    id: 'keys',
    title: 'Keys and foreign keys',
    blurb: 'Dependencies, keys, links between files, diagram',
    targets: ['Key Discovery', 'Relation Detector', 'Cardinality Analyzer', 'ERD Generator'],
  },
  {
    id: 'profile',
    title: 'Clean and profile',
    blurb: 'Cleaning, types and column meaning only',
    targets: ['Column Classifier'],
  },
  {
    id: 'all',
    title: 'All steps',
    blurb: 'Every operation',
    targets: Object.keys(OPERATIONS),
  },
];

export const DEFAULT_GOAL = GOALS[0];

export const PHASES: { id: Phase; title: string }[] = [
  { id: 'PREP', title: 'Prepare' },
  { id: 'ANALYZE', title: 'Analyze' },
  { id: 'NORMAL', title: 'Normalize' },
  { id: 'OUTPUT', title: 'Generate' },
];

/** Operations in run order. */
export function ordered(ops: Iterable<string>): string[] {
  return [...new Set(ops)].filter(o => OPERATIONS[o]).sort((a, b) => OPERATIONS[a].order - OPERATIONS[b].order);
}

export function planForGoal(goal: Goal): string[] {
  return planFor(goal.targets).plan;
}

/** The goal whose pipeline is exactly `steps`, if any. */
export function goalOf(steps: readonly string[]): Goal | null {
  const key = ordered(steps).join('|');
  return GOALS.find(g => planForGoal(g).join('|') === key) ?? null;
}

export interface Toggle {
  steps: string[];
  /** Steps that came along: prerequisites added, or dependents removed. */
  alsoChanged: string[];
}

/** Tap on a step: add it with its prerequisites, or remove it with every step that depends on it. */
export function toggleStep(steps: readonly string[], op: string): Toggle {
  const current = new Set(steps);
  if (current.has(op)) {
    const removed = [...current].filter(s => s === op || dependencyClosure([s]).has(op));
    return {
      steps: ordered([...current].filter(s => !removed.includes(s))),
      alsoChanged: ordered(removed.filter(s => s !== op)),
    };
  }
  const added = [...dependencyClosure([op])].filter(s => !current.has(s));
  return {
    steps: ordered([...current, ...added]),
    alsoChanged: ordered(added.filter(s => s !== op)),
  };
}

export const phaseOf = (op: string): Phase => (OPERATIONS[op]?.phase ?? 'PREP') as Phase;
