/* eslint-disable @typescript-eslint/no-explicit-any -- engine results are untyped (see useResultViews) */
import { useMemo, useState, type ReactNode } from 'react';
import { Check, Copy, Download, Rocket } from 'lucide-react';
import { useResultViews } from '../hooks/useResultViews';
import { useCopy } from '../hooks/useCopy';
import { saveFileInBackground } from '../utils/saveFile';
import { InputChecksSection } from '../components/output/InputChecksSection';
import { NormalFormSection } from '../components/output/NormalFormSection';
import { FunctionalDependenciesSection } from '../components/output/FunctionalDependenciesSection';
import { KeysSection } from '../components/output/KeysSection';
import { KeysByTable, QualityByTable, RelationshipsSection } from '../components/output/MultiTableSections';
import { CardinalitySection } from '../components/output/CardinalitySection';
import { QualityAssuranceSection } from '../components/output/QualityAssuranceSection';
import { ErdSection } from '../components/output/ErdSection';
import { CodeViewer } from '../components/output/CodeViewer';
import { ReportView } from '../components/output/ReportView';
import { GeneratedFilesSection } from '../components/output/GeneratedFilesSection';

type Tab = 'overview' | 'tables' | 'diagram' | 'sql' | 'keys' | 'links' | 'deps' | 'checks' | 'report' | 'files';

const titleOf = (r: any): string => r.displayName ?? (r.nameSource === 'placeholder' ? 'Unnamed table' : r.name);

function Card({ children }: { children: ReactNode }) {
  return <div className="m-card m-section">{children}</div>;
}

export function ResultsScreen({
  results,
  onGoData,
  onMerge,
}: {
  results: any | null;
  onGoData: () => void;
  onMerge: () => void;
}) {
  const v = useResultViews(results);
  const [tab, setTab] = useState<Tab>('overview');
  const { copied, copy } = useCopy();

  const finalRelations: any[] = useMemo(() => (Array.isArray(v.erdRelations) ? v.erdRelations : []), [v.erdRelations]);

  if (!results) {
    return (
      <div className="m-enter m-empty">
        <div className="m-empty-art">
          <Rocket size={40} />
        </div>
        <h2>Nothing here yet</h2>
        <p>Add your tables, pick a goal and press Run. Your new database appears here.</p>
        <button className="m-btn m-btn-primary m-press" onClick={onGoData}>
          Get started
        </button>
      </div>
    );
  }

  const links = v.isMultiTable ? v.crossTableRelationships.length : Object.keys(v.cardinalityAnalysis).length;
  const fdCount = results.stats?.functionalDependencies || v.fdData.length;
  const tabs: { id: Tab; label: string; show: boolean }[] = [
    { id: 'overview', label: 'Overview', show: true },
    { id: 'tables', label: 'Tables', show: v.relations1NF.length + v.relations2NF.length + v.relationData.length + v.relationsBCNF.length > 0 },
    { id: 'diagram', label: 'Diagram', show: Boolean(v.erdContent) },
    { id: 'sql', label: 'SQL', show: Boolean(v.sqlFile.content) },
    { id: 'keys', label: 'Keys', show: Boolean(v.keyData) || v.isMultiTable },
    { id: 'links', label: 'Links', show: links > 0 },
    { id: 'deps', label: 'Dependencies', show: v.fdData.length > 0 },
    { id: 'checks', label: 'Checks', show: Boolean(v.losslessCheck || v.dependencyPreservation) || v.isMultiTable },
    { id: 'report', label: 'Report', show: Boolean(v.reportFile.content) },
    { id: 'files', label: 'Files', show: Boolean(results.outputFiles?.length) },
  ];

  return (
    <div className="m-enter">
      <section className="m-hero" aria-label="Summary">
        <h2>Your database is ready</h2>
        <p>
          {v.isMultiTable ? `${v.multiTables.length} files` : '1 file'} analysed in {results.duration ?? '—'}
        </p>
        <div className="m-stats">
          <div>
            <b>{finalRelations.length}</b>
            <span>tables in the new schema</span>
          </div>
          <div>
            <b>{links}</b>
            <span>{v.isMultiTable ? 'links between files' : 'relationships'}</span>
          </div>
          <div>
            <b>{fdCount}</b>
            <span>dependencies found</span>
          </div>
          <div>
            <b>{v.isMultiTable ? v.multiTables.reduce((n: number, t: any) => n + (t.rowCount || 0), 0).toLocaleString() : (results.stats?.originalRows ?? '—')}</b>
            <span>rows read</span>
          </div>
        </div>
      </section>

      <div className="m-tabs" role="tablist" aria-label="Result sections">
        {tabs
          .filter(t => t.show)
          .map(t => (
            <button key={t.id} role="tab" aria-selected={tab === t.id} className="m-press" onClick={() => setTab(t.id)}>
              {t.label}
            </button>
          ))}
      </div>

      <div key={tab} className="m-enter">
        {tab === 'overview' && (
          <>
            <InputChecksSection checks={results.inputChecks} onMerge={onMerge} />
            <div className="m-label">New tables</div>
            {finalRelations.map((r: any, i: number) => {
              const pk = new Set<string>(r.primaryKey ?? []);
              const fk = new Set<string>((r.foreignKeys ?? []).flatMap((f: any) => f.attributes ?? []));
              const rows = r.rowCount ?? r.metadata?.rowCount ?? r.rows?.length;
              return (
                <article key={`${r.name}-${i}`} className="m-card m-table-card">
                  <header>
                    <h3>{titleOf(r)}</h3>
                    <span>{typeof rows === 'number' ? `${rows.toLocaleString()} rows` : ''}</span>
                  </header>
                  <div className="m-cols">
                    {(r.attributes ?? []).map((a: string) => (
                      <span key={a} className={`m-col${pk.has(a) ? ' m-pk' : fk.has(a) ? ' m-fk' : ''}`}>
                        {pk.has(a) ? '🔑 ' : fk.has(a) ? '↗ ' : ''}
                        {a}
                      </span>
                    ))}
                  </div>
                </article>
              );
            })}
          </>
        )}
        {tab === 'tables' && (
          <Card>
            <NormalFormSection relations1NF={v.relations1NF} relations2NF={v.relations2NF} relations3NF={v.relationData} relationsBCNF={v.relationsBCNF} />
          </Card>
        )}
        {tab === 'diagram' && (
          <Card>
            <ErdSection relations={v.erdRelations} erdContent={v.erdContent} />
          </Card>
        )}
        {tab === 'sql' && (
          <Card>
            <div style={{ display: 'flex', gap: 8, marginBottom: 12 }}>
              <button className="m-btn m-btn-soft m-btn-small m-press" onClick={() => void copy(v.sqlFile.content)}>
                {copied ? <Check /> : <Copy />} {copied ? 'Copied' : 'Copy'}
              </button>
              <button
                className="m-btn m-btn-primary m-btn-small m-press"
                onClick={() => saveFileInBackground('schema.sql', v.sqlFile.content, 'application/sql')}
              >
                <Download /> Save .sql
              </button>
            </div>
            <CodeViewer code={v.sqlFile.content} language="sql" />
          </Card>
        )}
        {tab === 'keys' && <Card>{v.isMultiTable ? <KeysByTable tables={v.multiTables} /> : <KeysSection keyData={v.keyData} />}</Card>}
        {tab === 'links' && (
          <Card>
            {v.isMultiTable ? (
              <RelationshipsSection relationships={v.crossTableRelationships} />
            ) : (
              <CardinalitySection cardinalityAnalysis={v.cardinalityAnalysis} />
            )}
          </Card>
        )}
        {tab === 'deps' && (
          <Card>
            <FunctionalDependenciesSection
              fdData={v.fdData}
              tableRows={v.isMultiTable ? Object.fromEntries(v.multiTables.map((t: any) => [t.tableName, t.rowCount])) : undefined}
            />
          </Card>
        )}
        {tab === 'checks' && (
          <Card>
            {v.isMultiTable ? (
              <QualityByTable tables={v.multiTables} />
            ) : (
              <QualityAssuranceSection losslessCheck={v.losslessCheck} dependencyPreservation={v.dependencyPreservation} />
            )}
          </Card>
        )}
        {tab === 'report' && (
          <Card>
            <ReportView
              results={results}
              markdown={v.reportFile.content}
              fdData={v.fdData}
              levels={[
                { id: '1NF', relations: v.relations1NF },
                { id: '2NF', relations: v.relations2NF },
                { id: '3NF', relations: v.relationData },
                { id: 'BCNF', relations: v.relationsBCNF },
              ].filter(l => l.relations.length > 0)}
            />
          </Card>
        )}
        {tab === 'files' && (
          <Card>
            <GeneratedFilesSection outputFiles={results.outputFiles} />
          </Card>
        )}
      </div>
    </div>
  );
}
