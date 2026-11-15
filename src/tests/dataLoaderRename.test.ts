// @vitest-environment jsdom
import { describe, it, expect } from 'vitest';
import { renderHook, act } from '@testing-library/react';
import { OPERATIONS, ALL_OPERATION_NAMES, canonicalOperationName } from '../config/operations';
import { DatabaseService } from '../services/api/databaseService';
import { useWorkflowGraph } from '../hooks/useWorkflowGraph';
import { resolveLexical } from '../services/nlp/workflow/lexicalResolver';

const csv = (name: string, text: string) => new File([text], name, { type: 'text/csv' });

describe('Data Loader (formerly CSV Data Loader)', () => {
  it('is registered under the new name, with a description that names every input type', () => {
    expect(OPERATIONS['Data Loader']?.id).toBe('csv-loader');
    expect(OPERATIONS['Data Loader']?.description).toMatch(/CSV.*Excel.*ODS/);
    expect(ALL_OPERATION_NAMES).not.toContain('CSV Data Loader');
    expect(canonicalOperationName('CSV Data Loader')).toBe('Data Loader');
  });

  it('the engine accepts the old name', async () => {
    const rows = ['order_id,customer,city'];
    for (let i = 1; i <= 30; i++) rows.push(`${i},c${i % 5},city${i % 5}`);
    const r: any = await new DatabaseService().processFile(csv('orders.csv', rows.join('\n')), ['CSV Data Loader', 'FD Miner', 'Key Discovery', '3NF']);
    expect(r.success).toBe(true);
    expect(r.relations.length).toBeGreaterThan(1);
  });

  it('requests about loading CSV, Excel or spreadsheet files resolve to it', () => {
    for (const q of ['load the csv', 'import my excel workbook', 'open the spreadsheet']) {
      expect(resolveLexical(q).targets).toContain('Data Loader');
    }
  });

  it('a workflow saved with the old name loads under the new name', () => {
    const { result } = renderHook(() => useWorkflowGraph({ log: () => {} }));
    const snapshot = {
      format: 'dbflow-workflow', version: 2, nextId: 2,
      nodes: [
        { id: '0', type: 'start', position: { x: 0, y: 0 }, data: { label: 'Start' } },
        { id: '1', type: 'operation', position: { x: 0, y: 0 }, data: { label: 'CSV Data Loader', phase: 'PREP', operationId: 'csv-loader' } },
      ],
      edges: [],
    };
    let ok = false;
    act(() => { ok = result.current.loadSnapshot(snapshot as object); });
    expect(ok).toBe(true);
    expect(result.current.operationLabels).toEqual(['Data Loader']);
  });
});
