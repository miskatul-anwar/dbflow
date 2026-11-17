/* eslint-disable @typescript-eslint/no-explicit-any -- engine results are untyped (see useResultViews) */
import { useMemo, useState, type ReactNode } from 'react';
import { Check, Copy, Download } from 'lucide-react';
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
import { NormalizationJourney, ReportView } from '../components/output/ReportView';
import { GeneratedFilesSection } from '../components/output/GeneratedFilesSection';

type Tab = 'overview' | 'tables' | 'diagram' | 'sql' | 'keys' | 'links' | 'deps' | 'checks' | 'report' | 'files';

const titleOf = (r: any): string => r.displayName ?? (r.nameSource === 'placeholder' ? 'Unnamed table' : r.name);

/** A result view on a tab. The desktop views bring their own cards, so the tab adds no frame around them. */
function Card({ children }: { children: ReactNode }) {
  return <div className="m-panel">{children}</div>;
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
      <div className="m-group m-empty-note">
        <p style={{ marginTop: 0 }}>No results yet. Add files, choose a goal, then run.</p>
        <button className="m-btn m-btn-outline" onClick={onGoData}>
          Go to Data
        </button>
      </div>
    );
  }

  const links = v.isMultiTable ? v.crossTableRelationships.length : Object.keys(v.cardinalityAnalysis).length;
  const fdCount = results.stats?.functionalDependencies || v.fdData.length;
  const tabs: { id: Tab; label: string; show: boolean }[] = [
    { id: 'overview', label: 'Overview', show: true },
    { id: 'tables', label: 'Tables', show: v.relations1NF.length + v.relations2NF.length + v.relationData.length + v.relationsBCNF.length + v.relations4NF.length > 0 },
    { id: 'diagram', label: 'Diagram', show: Boolean(v.erdContent) },
    { id: 'sql', label: 'SQL', show: Boolean(v.sqlFile.content) },
    { id: 'keys', label: 'Keys', show: Boolean(v.keyData) || v.isMultiTable },
    { id: 'links', label: 'Links', show: links > 0 },
    { id: 'deps', label: 'Dependencies', show: v.fdData.length > 0 },
    { id: 'checks', label: 'Checks', show: Boolean(v.losslessCheck || v.dependencyPreservation) || v.isMultiTable },
    { id: 'report', label: 'Report', show: Boolean(v.reportFile.content) },
    { id: 'files', label: 'Files', show: Boolean(results.outputFiles?.length) },
  ];

  const levels = [
    { id: '1NF', relations: v.relations1NF },
    { id: '2NF', relations: v.relations2NF },
    { id: '3NF', relations: v.relationData },
    { id: 'BCNF', relations: v.relationsBCNF },
    { id: '4NF', relations: v.relations4NF },
  ].filter(l => l.relations.length > 0);
  const rowsRead: number = v.isMultiTable ? v.multiTables.reduce((n: number, t: any) => n + (t.rowCount || 0), 0) : Number(results.stats?.originalRows ?? 0);
  return (
    <>
      <div className="m-group m-summary" aria-label="Summary">
        <div>
          <b>{finalRelations.length}</b>
          <span>tables</span>
        </div>
        <div>
          <b>{links}</b>
          <span>links</span>
        </div>
        <div>
          <b>{fdCount}</b>
          <span>FDs</span>
        </div>
        <div>
          <b>{rowsRead.toLocaleString()}</b>
          <span>rows</span>
        </div>
      </div>
      <p className="m-hint">
        {v.isMultiTable ? `${v.multiTables.length} files analysed` : '1 file analysed'}
      </p>

      <div className="m-tabs" role="tablist" aria-label="Result sections">
        {tabs
          .filter(t => t.show)
          .map(t => (
            <button key={t.id} role="tab" aria-selected={tab === t.id} onClick={() => setTab(t.id)}>
              {t.label}
            </button>
          ))}
      </div>

      <div role="tabpanel">
        {tab === 'overview' && (
          <>
            <InputChecksSection checks={results.inputChecks} onMerge={onMerge} />
            {!v.isMultiTable && (
              <div className="m-group m-section" style={{ marginTop: 12 }}>
                <NormalizationJourney levels={levels} />
              </div>
            )}
            <div className="m-label">New schema</div>
            <div className="m-group">
              {finalRelations.map((r: any, i: number) => {
                const pk = new Set<string>(r.primaryKey ?? []);
                const fk = new Set<string>((r.foreignKeys ?? []).flatMap((f: any) => f.attributes ?? []));
                const rows = r.rowCount ?? r.metadata?.rowCount ?? r.rows?.length;
                return (
                  <article key={`${r.name}-${i}`} className="m-table">
                    <header>
                      <h3>{titleOf(r)}</h3>
                      <span>{typeof rows === 'number' ? `${rows.toLocaleString()} rows` : ''}</span>
                    </header>
                    <div className="m-cols">
                      {(r.attributes ?? []).map((a: string) => (
                        <span key={a} className={`m-col${pk.has(a) ? ' m-pk' : fk.has(a) ? ' m-fk' : ''}`}>
                          {pk.has(a) ? <b>PK</b> : fk.has(a) ? <b>FK</b> : null}
                          {a}
                        </span>
                      ))}
                    </div>
                  </article>
                );
              })}
            </div>
            {v.isMultiTable && v.multiTables.some((t: any) => t.nerTags?.length) && (
              <>
                <div className="m-label">Column meaning</div>
                <div className="m-group">
                  {v.multiTables.map((t: any) => (
                    <article key={t.tableName} className="m-table">
                      <header>
                        <h3 className="m-mono">{t.tableName}</h3>
                        <span>
                          {t.fileName} · {Number(t.rowCount ?? 0).toLocaleString()} rows
                        </span>
                      </header>
                      <div className="m-cols">
                        {(t.nerTags ?? []).map((tag: any) => (
                          <span
                            key={tag.columnName}
                            className={`m-col${tag.entityType === 'PRIMARY_KEY_ID' ? ' m-pk' : tag.entityType === 'FOREIGN_KEY_REF' ? ' m-fk' : ''}`}
                            title={tag.description}
                          >
                            {tag.columnName}: {String(tag.entityType).replace(/_/g, ' ').toLowerCase()}
                          </span>
                        ))}
                      </div>
                    </article>
                  ))}
                </div>
              </>
            )}
          </>
        )}
        {tab === 'tables' && (
          <Card>
            <NormalFormSection relations1NF={v.relations1NF} relations2NF={v.relations2NF} relations3NF={v.relationData} relationsBCNF={v.relationsBCNF} relations4NF={v.relations4NF} mvds={v.mvds} />
          </Card>
        )}
        {tab === 'diagram' && (
          <Card>
            <ErdSection relations={v.erdRelations} erdContent={v.erdContent} />
          </Card>
        )}
        {tab === 'sql' && (
          <Card>
            <div className="m-actions">
              <button className="m-btn m-btn-outline" onClick={() => void copy(v.sqlFile.content)}>
                {copied ? <Check /> : <Copy />} {copied ? 'Copied' : 'Copy'}
              </button>
              <button
                className="m-btn m-btn-primary"
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
              levels={levels}
            />
          </Card>
        )}
        {tab === 'files' && (
          <Card>
            <GeneratedFilesSection outputFiles={results.outputFiles} />
          </Card>
        )}
      </div>
    </>
  );
}
