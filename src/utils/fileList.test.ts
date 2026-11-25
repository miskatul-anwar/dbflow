import { describe, expect, it } from 'vitest';
import { addFiles, baseNameOf, formatSize, moveFile, removeFile, renameFile, replaceFile } from './fileList';

const file = (name: string, body = 'a,b\n1,2\n', lastModified = 1) =>
  new File([body], name, { type: name.endsWith('.csv') ? 'text/csv' : '', lastModified });

describe('file list', () => {
  it('appends files one by one or many at a time, skipping duplicates and unsupported files', () => {
    const a = file('a.csv');
    let r = addFiles([], [a]);
    expect(r.files.map(f => f.name)).toEqual(['a.csv']);
    r = addFiles(r.files, [file('b.xlsx'), file('a.csv'), file('notes.txt'), file('c.csv')]);
    expect(r.files.map(f => f.name)).toEqual(['a.csv', 'b.xlsx', 'c.csv']);
    expect(r.added.map(f => f.name)).toEqual(['b.xlsx', 'c.csv']);
    expect(r.duplicates.map(f => f.name)).toEqual(['a.csv']);
    expect(r.unsupported.map(f => f.name)).toEqual(['notes.txt']);
    // the same name with different content is a different file
    expect(addFiles(r.files, [file('a.csv', 'x\n9\n', 2)]).added).toHaveLength(1);
  });

  it('renames, keeping the extension and the content, and refuses empty, unsafe or taken names', async () => {
    const list = [file('orders.csv', 'id\n1\n'), file('customers.xlsx')];
    const r = renameFile(list, 0, 'sales 2025');
    expect(r.files?.map(f => f.name)).toEqual(['sales 2025.csv', 'customers.xlsx']);
    expect(await r.files![0].text()).toBe('id\n1\n');
    expect(renameFile(list, 0, 'sales.CSV').files?.[0].name).toBe('sales.csv');
    expect(renameFile(list, 0, '  ').error).toMatch(/name is needed/);
    expect(renameFile(list, 0, 'a/b').error).toMatch(/cannot contain/);
    expect(renameFile([file('a.csv'), file('b.csv')], 1, 'A').error).toMatch(/already named a\.csv/i);
    expect(renameFile(list, 5, 'x').error).toBeTruthy();
  });

  it('replaces in place, moves up and down within bounds, and removes', () => {
    const [a, b, c] = [file('a.csv'), file('b.csv'), file('c.csv')];
    const list = [a, b, c];
    expect(replaceFile(list, 1, file('d.csv')).map(f => f.name)).toEqual(['a.csv', 'd.csv', 'c.csv']);
    expect(moveFile(list, 2, -1).map(f => f.name)).toEqual(['a.csv', 'c.csv', 'b.csv']);
    expect(moveFile(list, 0, -1)).toBe(list);
    expect(moveFile(list, 2, 1)).toBe(list);
    expect(removeFile(list, 0).map(f => f.name)).toEqual(['b.csv', 'c.csv']);
  });

  it('formats sizes and base names', () => {
    expect(formatSize(512)).toBe('512 B');
    expect(formatSize(6 * 1024)).toBe('6.0 KB');
    expect(formatSize(3.5 * 1024 * 1024)).toBe('3.5 MB');
    expect(baseNameOf('sales.2025.csv')).toBe('sales.2025');
  });
});
