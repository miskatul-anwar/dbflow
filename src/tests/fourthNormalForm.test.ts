// @vitest-environment jsdom
import { afterEach, describe, expect, it } from 'vitest';
import { cleanup, fireEvent, render, screen } from '@testing-library/react';
import { createElement } from 'react';
import { DatabaseService } from '../services/api/databaseService';
import { resolveLexical } from '../services/nlp/workflow/lexicalResolver';
import { planFor } from '../services/nlp/workflow/planner';
import { NormalFormSection } from '../components/output/NormalFormSection';

/** A course's teachers and books are independent and shared between courses: BCNF, but not 4NF. */
function courseTeacherBook(): File {
  const rows = ['course,teacher,book'];
  const data: Array<[string, string[], string[]]> = [
    ['Databases', ['Ann', 'Bob'], ['Date', 'Ullman', 'Lang']],
    ['Physics', ['Cy', 'Ann'], ['Feynman', 'Date']],
    ['Algebra', ['Bob', 'Dee'], ['Lang', 'Artin']],
    ['Biology', ['Cy', 'Eve', 'Dee'], ['Raven', 'Feynman']],
  ];
  for (const [course, teachers, books] of data) for (const t of teachers) for (const b of books) rows.push(`${course},${t},${b}`);
  return new File([rows.join('\n')], 'ctb.csv', { type: 'text/csv' });
}

const sorted = (rels: Array<{ attributes: string[] }>) => rels.map(r => [...r.attributes].sort().join(',')).sort();

describe('4NF', () => {
  afterEach(cleanup);

  it('is understood by the request box and planned after BCNF', () => {
    for (const q of ['normalize to 4NF', 'convert it to fourth normal form', 'remove multivalued dependencies']) {
      expect(resolveLexical(q).targets, q).toContain('4NF');
    }
    const plan = planFor(['4NF']).plan;
    expect(plan.indexOf('BCNF')).toBeLessThan(plan.indexOf('4NF'));
  });

  it('splits independent multivalued facts in a full run', async () => {
    const ops = planFor(['4NF', 'Lossless Check', 'SQL Generator']).plan;
    const res = (await new DatabaseService().processFiles([courseTeacherBook()], ops, { namingMode: 'rules' })) as unknown as {
      success: boolean;
      error?: string;
      bcnf: Array<{ attributes: string[] }>;
      relations4NF: Array<{ attributes: string[] }>;
      multivaluedDependencies: Array<{ lhs: string[] }>;
      losslessCheck: { isLossless: boolean };
    };
    expect(res.success, res.error).toBe(true);
    expect(sorted(res.bcnf)).toEqual(['book,course,teacher']);
    expect(sorted(res.relations4NF)).toEqual(['book,course', 'course,teacher']);
    expect(res.multivaluedDependencies[0].lhs).toEqual(['course']);
    expect(res.losslessCheck.isLossless).toBe(true);
  });

  it('shows the 4NF relations and the dependency they were split on', () => {
    const rel = (name: string, attributes: string[]) => ({ name, attributes, primaryKey: attributes, foreignKeys: [], rows: [], metadata: { rowCount: 0, attributeTypes: {} } });
    render(
      createElement(NormalFormSection, {
        relationsBCNF: [rel('ctb', ['course', 'teacher', 'book'])],
        relations4NF: [rel('Course_Teacher', ['course', 'teacher']), rel('Course_Book', ['course', 'book'])],
        mvds: [{ lhs: ['course'], rhs: ['teacher'], rest: ['book'], relation: 'ctb', witnessGroups: 4, witnessRows: 22 }],
      }),
    );
    const tab = screen.getByRole('tab', { name: /4NF/ });
    expect(tab.getAttribute('aria-selected')).toBe('true');
    expect(screen.getByText(/Split on one multivalued dependency/)).toBeTruthy();
    expect(screen.getByText(/\{course\} ↠ \{teacher\} \| \{book\}/)).toBeTruthy();
    fireEvent.click(screen.getByRole('tab', { name: /BCNF/ }));
    expect(screen.queryByText(/Split on one multivalued dependency/)).toBeNull();
  });
});
