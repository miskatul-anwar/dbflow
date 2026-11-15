import { describe, expect, it } from 'vitest';
import { OPERATIONS } from '../config/operations';
import { dependencyClosure } from '../services/nlp/workflow/planner';
import { GOALS, goalOf, planForGoal, toggleStep } from './pipeline';

/** Every step's prerequisites are present. */
const closed = (steps: string[]) => steps.every(s => [...dependencyClosure([s])].every(d => steps.includes(d)));
const inOrder = (steps: string[]) => steps.every((s, i) => i === 0 || OPERATIONS[steps[i - 1]].order < OPERATIONS[s].order);

describe('phone pipeline', () => {
  it('gives every goal a valid pipeline in run order, and recognises it again', () => {
    for (const g of GOALS) {
      const plan = planForGoal(g);
      expect(closed(plan), g.id).toBe(true);
      expect(inOrder(plan), g.id).toBe(true);
      expect(goalOf(plan)?.id).toBe(g.id);
    }
    expect(planForGoal(GOALS.find(g => g.id === 'all')!)).toHaveLength(19);
  });

  it('adds a step with its prerequisites', () => {
    const t = toggleStep(['Data Loader'], '3NF');
    expect(t.steps).toContain('3NF');
    expect(t.alsoChanged).toEqual(expect.arrayContaining(['1NF', 'FD Miner', '2NF']));
    expect(closed(t.steps)).toBe(true);
    expect(inOrder(t.steps)).toBe(true);
  });

  it('removes a step with every step that needs it', () => {
    const full = planForGoal(GOALS[0]);
    const t = toggleStep(full, 'FD Miner');
    expect(t.steps).not.toContain('FD Miner');
    expect(t.steps).not.toContain('3NF');
    expect(t.alsoChanged).toEqual(expect.arrayContaining(['2NF', '3NF', 'Key Discovery']));
    expect(t.steps).toContain('Data Cleaner');
    expect(closed(t.steps)).toBe(true);
    expect(goalOf(t.steps)).toBeNull();
  });
});
