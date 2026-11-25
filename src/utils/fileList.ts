/**
 * The uploaded files of a workspace, as an ordered list. Pure functions (each returns a new list), so the rules are
 * tested once and every screen applies them the same way.
 *
 * Order matters: when overlapping files are merged, the first file's version of a conflicting record is kept.
 */
import { isSupportedInputName } from '../services/processors/spreadsheetProcessor';

export interface AddResult {
  files: File[];
  added: File[];
  /** Already in the list (same name, size and modification time). */
  duplicates: File[];
  /** Not a CSV or spreadsheet. */
  unsupported: File[];
}

const sameFile = (a: File, b: File) => a.name === b.name && a.size === b.size && a.lastModified === b.lastModified;

/** Appends the files the pipeline can read, skipping ones already in the list. Several at once or one by one. */
export function addFiles(list: File[], incoming: File[]): AddResult {
  const files = [...list];
  const added: File[] = [];
  const duplicates: File[] = [];
  const unsupported: File[] = [];
  for (const f of incoming) {
    if (!isSupportedInputName(f.name, f.type)) unsupported.push(f);
    else if (files.some(g => sameFile(f, g))) duplicates.push(f);
    else {
      files.push(f);
      added.push(f);
    }
  }
  return { files, added, duplicates, unsupported };
}

export function extensionOf(name: string): string {
  const dot = name.lastIndexOf('.');
  return dot > 0 ? name.slice(dot) : '';
}

export function baseNameOf(name: string): string {
  const ext = extensionOf(name);
  return ext ? name.slice(0, -ext.length) : name;
}

export type RenameResult = { files: File[]; error?: undefined } | { files?: undefined; error: string };

/**
 * Renames a file. The extension is kept (it tells the engine how to read the file), so `newName` may be given with or
 * without it. Names must be unique within the workspace, ignoring case.
 */
export function renameFile(list: File[], index: number, newName: string): RenameResult {
  const file = list[index];
  if (!file) return { error: 'No such file.' };
  const ext = extensionOf(file.name);
  let base = newName.trim();
  if (ext && base.toLowerCase().endsWith(ext.toLowerCase())) base = base.slice(0, -ext.length).trim();
  if (!base) return { error: 'A name is needed.' };
  if (/[\\/:*?"<>|]/.test(base)) return { error: 'A name cannot contain \\ / : * ? " < > |' };
  const name = `${base}${ext}`;
  if (name === file.name) return { files: list };
  if (list.some((f, i) => i !== index && f.name.toLowerCase() === name.toLowerCase())) {
    return { error: `Another file is already named ${name}.` };
  }
  const renamed = new File([file], name, { type: file.type, lastModified: file.lastModified });
  return { files: list.map((f, i) => (i === index ? renamed : f)) };
}

/** Puts `replacement` in the place of the file at `index` (same position, so the merge order is kept). */
export function replaceFile(list: File[], index: number, replacement: File): File[] {
  if (!list[index]) return list;
  return list.map((f, i) => (i === index ? replacement : f));
}

/** Moves a file one place up (-1) or down (+1). */
export function moveFile(list: File[], index: number, by: -1 | 1): File[] {
  const to = index + by;
  if (!list[index] || to < 0 || to >= list.length) return list;
  const files = [...list];
  [files[index], files[to]] = [files[to], files[index]];
  return files;
}

export function removeFile(list: File[], index: number): File[] {
  return list.filter((_, i) => i !== index);
}

export function formatSize(bytes: number): string {
  if (bytes < 1024) return `${bytes} B`;
  if (bytes < 1024 * 1024) return `${(bytes / 1024).toFixed(bytes < 10 * 1024 ? 1 : 0)} KB`;
  return `${(bytes / (1024 * 1024)).toFixed(1)} MB`;
}
