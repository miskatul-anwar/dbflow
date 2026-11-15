import { useRef, type ReactElement } from 'react';
import { ArrowRight, FilePlus2, FileSpreadsheet, FileText, Sheet as SheetIcon, X } from 'lucide-react';
import { INPUT_ACCEPT } from '../services/processors/spreadsheetProcessor';
import { isTauriMobile } from '../utils/platform';

const size = (bytes: number) =>
  bytes < 1024 ? `${bytes} B` : bytes < 1 << 20 ? `${(bytes / 1024).toFixed(0)} KB` : `${(bytes / (1 << 20)).toFixed(1)} MB`;

function fileLook(name: string): { icon: ReactElement; color: string; kind: string } {
  const ext = name.split('.').pop()?.toLowerCase() ?? '';
  if (['xlsx', 'xlsm', 'xlsb', 'xls', 'xltx', 'xltm', 'xlam', 'xlt', 'xla', 'xlw'].includes(ext))
    return { icon: <FileSpreadsheet size={22} />, color: 'var(--m-prep)', kind: 'Excel workbook' };
  if (['ods', 'ots', 'fods'].includes(ext)) return { icon: <SheetIcon size={22} />, color: 'var(--m-normal)', kind: 'OpenDocument sheet' };
  return { icon: <FileText size={22} />, color: 'var(--m-output)', kind: ext === 'xml' ? 'XML spreadsheet' : 'CSV table' };
}

export function DataScreen({
  files,
  onAdd,
  onRemove,
  onNext,
  busy,
}: {
  files: File[];
  onAdd: (files: File[]) => void;
  onRemove: (index: number) => void;
  onNext: () => void;
  busy: boolean;
}) {
  const input = useRef<HTMLInputElement>(null);
  const total = files.reduce((n, f) => n + f.size, 0);
  return (
    <div className="m-enter">
      <h1 className="m-h1">Your data</h1>
      <p className="m-lead">Add one table or many. DBFlow finds how they connect and builds a clean database from them, right on this phone.</p>

      {/* Android's picker filters by MIME type and phones label CSV files inconsistently, so the app offers every
          file; the engine's input checks decide what each one really is. */}
      <input
        ref={input}
        type="file"
        multiple
        hidden
        accept={isTauriMobile() ? undefined : INPUT_ACCEPT}
        onChange={e => {
          const picked = Array.from(e.target.files ?? []);
          e.target.value = '';
          if (picked.length) onAdd(picked);
        }}
      />
      <button className="m-drop m-press" onClick={() => input.current?.click()} disabled={busy}>
        <div className="m-drop-art" aria-hidden>
          <span />
          <span />
          <span>
            <FilePlus2 size={30} />
          </span>
        </div>
        <strong>{files.length ? 'Add more tables' : 'Add your tables'}</strong>
        <p>CSV, Excel or OpenDocument. Several at once is fine.</p>
        <span className="m-btn m-btn-primary">
          <FilePlus2 /> Choose files
        </span>
      </button>

      {files.length > 0 && (
        <>
          <div className="m-label">
            {files.length} {files.length === 1 ? 'file' : 'files'} · {size(total)}
          </div>
          {files.map((f, i) => {
            const look = fileLook(f.name);
            return (
              <div key={`${f.name}-${i}`} className="m-card m-file">
                <span className="m-file-icon" style={{ background: look.color }}>
                  {look.icon}
                </span>
                <div style={{ minWidth: 0 }}>
                  <div className="m-file-name">{f.name}</div>
                  <div className="m-file-meta">
                    {look.kind} · {size(f.size)}
                  </div>
                </div>
                <button className="m-remove m-press" onClick={() => onRemove(i)} aria-label={`Remove ${f.name}`} disabled={busy}>
                  <X size={20} />
                </button>
              </div>
            );
          })}
          <div style={{ marginTop: 16 }}>
            <button className="m-btn m-btn-soft m-press" onClick={onNext}>
              Choose what to build <ArrowRight />
            </button>
          </div>
        </>
      )}
    </div>
  );
}
