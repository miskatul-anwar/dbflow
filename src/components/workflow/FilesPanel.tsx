/**
 * FilesPanel — the workspace's uploaded files, in the left column above the operations (same card, same rows).
 *
 * Add one file or many at a time (the + button, or drop files on the card); each file has a menu to rename, replace,
 * move up or down (the merge keeps the first file's version of a conflicting record), save a copy, or remove it.
 * Double-click or F2 renames in place. In the narrow rail the card shows the file count only.
 */
import { useRef, useState } from 'react';
import { AnimatePresence, motion } from 'framer-motion';
import {
  ArrowDown,
  ArrowUp,
  Download,
  FileSpreadsheet,
  FileText,
  MoreHorizontal,
  Pencil,
  Plus,
  RefreshCw,
  Trash2,
  Upload,
} from 'lucide-react';
import { IconButton } from '@/components/ui/button';
import { DropdownMenu, DropdownMenuContent, DropdownMenuItem, DropdownMenuSeparator, DropdownMenuTrigger } from '@/components/ui/dropdown-menu';
import { INPUT_ACCEPT } from '@/services/processors/spreadsheetProcessor';
import { baseNameOf, extensionOf, formatSize } from '@/utils/fileList';

export interface FilesPanelProps {
  files: File[];
  /** Narrow rail: the count only. */
  compact?: boolean;
  /** A run is in progress: the list is read-only. */
  busy?: boolean;
  onAdd: (files: File[]) => void;
  /** Returns an error message when the name is refused. */
  onRename: (index: number, name: string) => string | undefined;
  onReplace: (index: number, file: File) => void;
  onMove: (index: number, by: -1 | 1) => void;
  onRemove: (index: number) => void;
  onClear: () => void;
  onDownload: (index: number) => void;
}

const iconFor = (name: string) => (extensionOf(name).toLowerCase() === '.csv' ? FileText : FileSpreadsheet);

export function FilesPanel({ files, compact, busy, onAdd, onRename, onReplace, onMove, onRemove, onClear, onDownload }: FilesPanelProps) {
  const addInput = useRef<HTMLInputElement>(null);
  const replaceInput = useRef<HTMLInputElement>(null);
  const replaceIndex = useRef(-1);
  const [editing, setEditing] = useState<number | null>(null);
  const [draft, setDraft] = useState('');
  const [error, setError] = useState<string | null>(null);
  const [dragging, setDragging] = useState(false);

  const startRename = (i: number) => {
    if (busy) return;
    setEditing(i);
    setDraft(baseNameOf(files[i].name));
    setError(null);
  };
  const commitRename = () => {
    if (editing === null) return;
    const err = onRename(editing, draft);
    if (err) {
      setError(err);
      return;
    }
    setEditing(null);
    setError(null);
  };

  const pick = (input: HTMLInputElement | null) => input?.click();

  return (
    <section
      className={`flex min-h-0 flex-col rounded-[inherit] ${dragging ? 'nf-drop-target' : ''}`}
      aria-label="Files"
      data-testid="files-panel"
      onDragOver={e => {
        if (busy || !e.dataTransfer.types.includes('Files')) return;
        e.preventDefault();
        setDragging(true);
      }}
      onDragLeave={e => {
        if (!e.currentTarget.contains(e.relatedTarget as Node)) setDragging(false);
      }}
      onDrop={e => {
        e.preventDefault();
        setDragging(false);
        if (!busy && e.dataTransfer.files.length) onAdd(Array.from(e.dataTransfer.files));
      }}
    >
      <input
        ref={addInput}
        type="file"
        multiple
        accept={INPUT_ACCEPT}
        className="hidden"
        data-testid="files-add-input"
        onChange={e => {
          const picked = Array.from(e.target.files ?? []);
          e.target.value = '';
          if (picked.length) onAdd(picked);
        }}
      />
      <input
        ref={replaceInput}
        type="file"
        accept={INPUT_ACCEPT}
        className="hidden"
        data-testid="files-replace-input"
        onChange={e => {
          const f = e.target.files?.[0];
          e.target.value = '';
          if (f && replaceIndex.current >= 0) onReplace(replaceIndex.current, f);
          replaceIndex.current = -1;
        }}
      />

      <div className={`flex shrink-0 items-center gap-1 border-b border-gray-200 px-2 py-2 ${compact ? 'justify-center' : 'justify-between'}`}>
        {!compact && (
          <div className="flex min-w-0 items-baseline gap-2 pl-1">
            <h2 className="text-sm font-semibold text-gray-900">Files</h2>
            <span className="text-xs tabular-nums text-gray-500">{files.length}</span>
          </div>
        )}
        <div className="flex items-center gap-0.5">
          {!compact && files.length > 0 && (
            <IconButton size="icon-sm" label="Remove all files" icon={<Trash2 />} onClick={onClear} disabled={busy} />
          )}
          <IconButton size="icon-sm" label="Add files" icon={<Plus />} onClick={() => pick(addInput.current)} disabled={busy} />
        </div>
      </div>

      {!compact && (
        <div className="min-h-0 overflow-y-auto overscroll-contain p-1.5">
          {files.length === 0 ? (
            <button
              type="button"
              onClick={() => pick(addInput.current)}
              disabled={busy}
              className="flex w-full flex-col items-center gap-1.5 rounded-lg border border-dashed border-gray-300 px-3 py-4 text-sm text-gray-500 transition-colors hover:border-gray-400 hover:text-gray-700"
            >
              <Upload className="size-4" aria-hidden />
              Add or drop files
            </button>
          ) : (
            <ul className="space-y-0.5" aria-label="Uploaded files">
              <AnimatePresence initial={false}>
                {files.map((f, i) => {
                  const Icon = iconFor(f.name);
                  const isEditing = editing === i;
                  return (
                    <motion.li
                      key={`${f.name}-${f.size}-${f.lastModified}`}
                      layout
                      initial={{ opacity: 0, y: -4 }}
                      animate={{ opacity: 1, y: 0 }}
                      exit={{ opacity: 0, height: 0 }}
                      transition={{ duration: 0.16 }}
                      className="group flex items-center gap-2 rounded-lg px-1.5 py-1 hover:bg-gray-100"
                      onDoubleClick={() => startRename(i)}
                      onKeyDown={e => {
                        if (e.key === 'F2' && !isEditing) startRename(i);
                      }}
                    >
                      <Icon className="size-4 shrink-0 text-gray-500" aria-hidden />
                      {isEditing ? (
                        <div className="min-w-0 flex-1">
                          <div className="flex items-center gap-1">
                            <input
                              autoFocus
                              value={draft}
                              onChange={e => {
                                setDraft(e.target.value);
                                setError(null);
                              }}
                              onFocus={e => e.currentTarget.select()}
                              onKeyDown={e => {
                                if (e.key === 'Enter') commitRename();
                                if (e.key === 'Escape') setEditing(null);
                              }}
                              onBlur={commitRename}
                              aria-label={`New name for ${f.name}`}
                              aria-invalid={Boolean(error)}
                              className="h-6 min-w-0 flex-1 rounded-md border border-gray-300 bg-white px-1.5 text-sm text-gray-900 outline-none focus:border-gray-500"
                            />
                            <span className="shrink-0 text-xs text-gray-500">{extensionOf(f.name)}</span>
                          </div>
                          {error && (
                            <p role="alert" className="mt-0.5 text-xs text-red-700">
                              {error}
                            </p>
                          )}
                        </div>
                      ) : (
                        <span className="min-w-0 flex-1" title={`${f.name} · ${formatSize(f.size)}`}>
                          <span className="block truncate text-sm text-gray-900">{f.name}</span>
                        </span>
                      )}
                      {!isEditing && (
                        <DropdownMenu>
                          <DropdownMenuTrigger asChild disabled={busy}>
                            <button
                              type="button"
                              aria-label={`Actions for ${f.name}`}
                              className="flex size-6 shrink-0 items-center justify-center rounded-md text-gray-500 opacity-0 transition-opacity hover:bg-gray-200 hover:text-gray-900 focus-visible:opacity-100 group-hover:opacity-100 data-[state=open]:opacity-100 disabled:hidden"
                            >
                              <MoreHorizontal className="size-4" />
                            </button>
                          </DropdownMenuTrigger>
                          <DropdownMenuContent align="end">
                            <DropdownMenuItem onSelect={() => startRename(i)}>
                              <Pencil /> Rename
                            </DropdownMenuItem>
                            <DropdownMenuItem
                              onSelect={() => {
                                replaceIndex.current = i;
                                pick(replaceInput.current);
                              }}
                            >
                              <RefreshCw /> Replace…
                            </DropdownMenuItem>
                            <DropdownMenuItem onSelect={() => onDownload(i)}>
                              <Download /> Save a copy
                            </DropdownMenuItem>
                            <DropdownMenuSeparator />
                            <DropdownMenuItem disabled={i === 0} onSelect={() => onMove(i, -1)}>
                              <ArrowUp /> Move up
                            </DropdownMenuItem>
                            <DropdownMenuItem disabled={i === files.length - 1} onSelect={() => onMove(i, 1)}>
                              <ArrowDown /> Move down
                            </DropdownMenuItem>
                            <DropdownMenuSeparator />
                            <DropdownMenuItem destructive onSelect={() => onRemove(i)}>
                              <Trash2 /> Remove
                            </DropdownMenuItem>
                          </DropdownMenuContent>
                        </DropdownMenu>
                      )}
                    </motion.li>
                  );
                })}
              </AnimatePresence>
            </ul>
          )}
        </div>
      )}
      {compact && files.length > 0 && (
        <span className="py-1 text-center text-xs tabular-nums text-gray-500" aria-label={`${files.length} files`}>
          {files.length}
        </span>
      )}
    </section>
  );
}
