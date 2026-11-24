import { useRef } from 'react';
import { Plus, X } from 'lucide-react';
import { INPUT_ACCEPT } from '../services/processors/spreadsheetProcessor';
import { isTauriMobile } from '../utils/platform';

export const fileSize = (bytes: number) =>
  bytes < 1024 ? `${bytes} B` : bytes < 1 << 20 ? `${(bytes / 1024).toFixed(0)} KB` : `${(bytes / (1 << 20)).toFixed(1)} MB`;

const extension = (name: string) => (name.includes('.') ? name.split('.').pop()!.toUpperCase().slice(0, 4) : '?');

export function DataScreen({
  files,
  onAdd,
  onRemove,
  busy,
}: {
  files: File[];
  onAdd: (files: File[]) => void;
  onRemove: (index: number) => void;
  busy: boolean;
}) {
  const input = useRef<HTMLInputElement>(null);
  return (
    <>
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
      <button className="m-btn m-btn-primary m-btn-block" onClick={() => input.current?.click()} disabled={busy}>
        <Plus /> Add files
      </button>

      <div className="m-label">Files</div>
      <div className="m-group">
        {files.length === 0 ? (
          <div className="m-empty-note">No files yet.</div>
        ) : (
          files.map((f, i) => (
            <div key={`${f.name}-${i}`} className="m-row">
              <span className="m-type" aria-hidden>
                {extension(f.name)}
              </span>
              <span className="m-row-text">
                <strong>{f.name}</strong>
                <small>{fileSize(f.size)}</small>
              </span>
              <button className="m-remove" onClick={() => onRemove(i)} aria-label={`Remove ${f.name}`} disabled={busy}>
                <X size={20} />
              </button>
            </div>
          ))
        )}
      </div>
    </>
  );
}
