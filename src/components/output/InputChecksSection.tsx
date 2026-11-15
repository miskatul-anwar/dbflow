import { AlertTriangle, Copy, FileX, GitMerge, Layers } from 'lucide-react';
import { Button } from '@/components/ui/button';
import type { InputChecks } from '../../types';

const plural = (n: number, one: string, many = `${one}s`) => `${n.toLocaleString()} ${n === 1 ? one : many}`;

function Group({ icon, title, children }: { icon: React.ReactNode; title: string; children: React.ReactNode }) {
  return (
    <div>
      <h4 className="flex items-center gap-2 text-xs font-semibold uppercase tracking-wider text-gray-600">
        {icon}
        {title}
      </h4>
      <ul className="mt-1.5 space-y-1 text-sm text-gray-800 [overflow-wrap:anywhere]">{children}</ul>
    </div>
  );
}

/**
 * What the engine found about the uploaded files before analysing them: files it left out, exact copies, and files
 * that overlap (with a way to merge them and run again).
 */
export function InputChecksSection({ checks, onMerge }: { checks?: InputChecks | null; onMerge?: () => void }) {
  const skipped = checks?.skipped ?? [];
  const duplicates = checks?.duplicates ?? [];
  const overlaps = checks?.overlaps ?? [];
  const merged = checks?.merged ?? [];
  if (!skipped.length && !duplicates.length && !overlaps.length && !merged.length) return null;

  return (
    <section aria-labelledby="input-checks-title" className="overflow-hidden rounded-lg border border-amber-200 bg-amber-50/60 shadow-sm">
      <div className="flex items-center gap-2 border-b border-amber-200 bg-amber-50 px-5 py-3">
        <AlertTriangle className="size-4 text-amber-600" aria-hidden />
        <h3 id="input-checks-title" className="text-sm font-semibold text-gray-900">Input checks</h3>
        <span className="text-xs text-gray-600">found before the analysis started</span>
      </div>
      <div className="space-y-4 px-5 py-4">
        {skipped.length > 0 && (
          <Group icon={<FileX className="size-3.5 text-red-600" aria-hidden />} title={`Not analysed · ${skipped.length}`}>
            {skipped.map((s, i) => <li key={i}>{s.reason}</li>)}
          </Group>
        )}
        {duplicates.length > 0 && (
          <Group icon={<Copy className="size-3.5 text-amber-700" aria-hidden />} title={`Identical files · ${duplicates.length}`}>
            {duplicates.map((d, i) => (
              <li key={i}>
                <span className="font-mono">{d.file}</span> has the same contents as <span className="font-mono">{d.sameAs}</span>, so it was
                analysed once.
              </li>
            ))}
          </Group>
        )}
        {overlaps.length > 0 && (
          <Group icon={<Layers className="size-3.5 text-amber-700" aria-hidden />} title={`Overlapping files · ${overlaps.length}`}>
            {overlaps.map((o, i) => (
              <li key={i}>
                <span className="font-mono">{o.files[0]}</span> and <span className="font-mono">{o.files[1]}</span> have the same columns and share{' '}
                {plural(o.sharedRows, 'row')} ({o.rows[0].toLocaleString()} and {o.rows[1].toLocaleString()} rows in all).
                {o.key && (
                  <>
                    {' '}{plural(o.sharedKeys, 'record')} match on <span className="font-mono">{o.key}</span>
                    {o.conflicts > 0
                      ? <>, and {plural(o.conflicts, 'has', 'have')} different values{o.conflictKeys.length > 0 && <> (for example {o.conflictKeys.join(', ')})</>}.</>
                      : '.'}
                  </>
                )}
              </li>
            ))}
          </Group>
        )}
        {merged.length > 0 && (
          <Group icon={<GitMerge className="size-3.5 text-teal-700" aria-hidden />} title="Merged">
            {merged.map((m, i) => (
              <li key={i}>
                <span className="font-mono">{m.from.join(', ')}</span> merged into <span className="font-mono">{m.into}</span>: {plural(m.rows, 'distinct row')}.
                {m.conflictsResolved > 0 && <> For {plural(m.conflictsResolved, 'conflicting record')} the version in <span className="font-mono">{m.into}</span> was kept.</>}
              </li>
            ))}
          </Group>
        )}
        {overlaps.length > 0 && merged.length === 0 && onMerge && (
          <div className="flex flex-wrap items-center gap-3 border-t border-amber-200 pt-3">
            <Button onClick={onMerge} title="Combine overlapping files into one table and run the pipeline again">
              <GitMerge />
              <span>Merge overlapping files and run again</span>
            </Button>
            <span className="text-xs text-gray-600">Rows that repeat are kept once. Where records conflict, the first file's version is kept.</span>
          </div>
        )}
      </div>
    </section>
  );
}
