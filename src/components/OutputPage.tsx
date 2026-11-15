import { useMemo, useState } from 'react';
import { useLocation, useNavigate } from 'react-router-dom';
import {
  ArrowLeft, Check, Copy, Download, Layers, GitBranch,
  Code, Activity, Database, FileText, BarChart3, Shield, Key, Cpu
} from 'lucide-react';

import { CustomSidebar, SidebarItem } from './output/CustomSidebar';
import { FunctionalDependenciesSection } from './output/FunctionalDependenciesSection';
import { NormalFormSection } from './output/NormalFormSection';
import { CardinalitySection } from './output/CardinalitySection';
import { QualityAssuranceSection } from './output/QualityAssuranceSection';
import { KeysSection } from './output/KeysSection';
import { RelationshipsSection, KeysByTable, QualityByTable } from './output/MultiTableSections';
import { GeneratedFilesSection } from './output/GeneratedFilesSection';
import { ReportView, NormalizationJourney } from './output/ReportView';
import { ErdSection } from './output/ErdSection';
import { InputChecksSection } from './output/InputChecksSection';
import { saveFileInBackground } from '../utils/saveFile';
import { useResultViews } from '../hooks/useResultViews';
import { StatCard } from './output/StatCard';
import { CodeViewer } from './output/CodeViewer';
import { Button } from '@/components/ui/button';
import type { WorkspaceTab } from '../hooks/useWorkspaceTabs';
import { useCopy } from '../hooks/useCopy';
import { loadCachedResult, saveCachedResult } from '../utils/resultCache';

export default function OutputPage({ tabId, tabs = [] }: { tabId: string; tabs?: WorkspaceTab[] }) {
  const location = useLocation();
  const navigate = useNavigate();
  // Results normally come from the tab's cache (the workspace stores them before navigating); results passed in the
  // navigation state are still accepted, for the tab that ran.
  // Results are untyped throughout this page (the engine's result shape differs for one file and many).
  // eslint-disable-next-line @typescript-eslint/no-explicit-any
  const state = location.state as { results?: any; tabId?: string } | null;
  const results = state?.tabId === tabId ? state.results : undefined;
  /** The workspace these results came from, so several open tabs cannot be confused. */
  const sourceTab = tabs.find(t => t.id === tabId) ?? null;

  const currentDateTime = useMemo(() => {
    const now = new Date();
    return {
      formatted: now.toLocaleString('en-US', {
        year: 'numeric',
        month: '2-digit',
        day: '2-digit',
        hour: '2-digit',
        minute: '2-digit',
        second: '2-digit',
        hour12: false
      }),
      utc: now.toISOString().replace('T', ' ').substring(0, 19) + ' UTC'
    };
  }, []);

  const safeResults = useMemo(() => {
    const isUsable = (r: any) =>
      r && r.success !== false && (r.success || r.tables || r.outputFiles);
    if (isUsable(results)) {
      saveCachedResult(results, tabId);
      return results;
    }
    return loadCachedResult(tabId);
  }, [results, tabId]);

  const {
    isMultiTable, multiTables, crossTableRelationships, relationData, relations1NF, relations2NF, relationsBCNF,
    relations4NF, mvds, erdRelations, erdContent, cardinalityAnalysis, losslessCheck, dependencyPreservation, fdData, keyData, sqlFile, reportFile,
  } = useResultViews(safeResults);

  const [activeTab, setActiveTab] = useState("overview");
  const { copied: sqlCopied, copy: copySql } = useCopy();

  const sidebarItems: SidebarItem[] = [
    {
      id: "overview",
      group: "summary",
      description: "Summary of this run",
      label: "Overview",
      icon: <Activity className="w-4 h-4" />,
      hasData: true
    },
    {
      id: "normal-forms",
      group: "analysis",
      description: "1NF to 4NF relations",
      label: "Normal Forms",
      icon: <Layers className="w-4 h-4" />,
      // Relations in the final schema (the deepest normal form reached), not a sum over all levels.
      count: relations4NF.length || relationsBCNF.length || relationData.length || relations2NF.length || relations1NF.length || 0,
      hasData: relations1NF.length > 0 || relations2NF.length > 0 || relationData.length > 0 || relationsBCNF.length > 0 || relations4NF.length > 0
    },
    {
      id: "functional-dependencies",
      group: "analysis",
      description: "Mined dependencies",
      label: "Functional Dependencies",
      icon: <GitBranch className="w-4 h-4" />,
      count: fdData.length,
      hasData: fdData.length > 0 || isMultiTable
    },
    {
      id: "cardinality",
      group: "analysis",
      description: "Links between tables",
      label: "Cardinality & Relations",
      icon: <BarChart3 className="w-4 h-4" />,
      count: crossTableRelationships.length || Object.keys(cardinalityAnalysis).length,
      hasData: crossTableRelationships.length > 0 || Object.keys(cardinalityAnalysis).length > 0
    },
    {
      id: "keys",
      group: "analysis",
      description: "Candidate keys",
      label: "Keys & Superkeys",
      icon: <Key className="w-4 h-4" />,
      hasData: Boolean(keyData) || isMultiTable
    },
    {
      id: "quality",
      group: "analysis",
      description: "Lossless and preservation",
      label: "Quality Assurance",
      icon: <Shield className="w-4 h-4" />,
      hasData: Boolean(losslessCheck || dependencyPreservation) || isMultiTable
    },
    {
      id: "erd",
      group: "generated",
      description: "Entity-relationship diagram",
      label: "ERD Diagram",
      icon: <GitBranch className="w-4 h-4" />,
      hasData: Boolean(erdContent)
    },
    {
      id: "sql",
      group: "generated",
      description: "DDL for your schema",
      label: "SQL Schema",
      icon: <Database className="w-4 h-4" />,
      hasData: Boolean(sqlFile?.content)
    },
    {
      id: "report",
      group: "generated",
      description: "Full written report",
      label: "Full Report",
      icon: <FileText className="w-4 h-4" />,
      hasData: Boolean(reportFile?.content)
    },
    {
      id: "files",
      group: "generated",
      description: "Everything generated",
      label: "Generated Files",
      icon: <Code className="w-4 h-4" />,
      count: safeResults?.outputFiles?.length || 0,
      hasData: Boolean(safeResults?.outputFiles?.length)
    }
  ];

  if (!safeResults) {
    return (
      <div className="nf-output-page flex items-center justify-center p-4" style={{ background: "#e5e7eb" }}>
        <div className="bg-white rounded-xl border border-gray-200 p-8 max-w-md w-full text-center shadow-sm">
          <div className="w-12 h-12 bg-gray-100 rounded-full flex items-center justify-center text-gray-500 mx-auto mb-4">
            <Activity className="w-6 h-6" />
          </div>
          <h3 className="text-xl font-bold text-gray-900 mb-2">
            {sourceTab ? `No results yet for “${sourceTab.title}”` : 'No Output Available'}
          </h3>
          <p className="text-gray-600 mb-6">
            Run this workspace's pipeline to see its results here. Use the tabs above to switch to another workspace.
          </p>
          <Button variant="primary" className="w-full" onClick={() => navigate('/')}>
            <ArrowLeft />
            Back to workflow
          </Button>
        </div>
      </div>
    );
  }

  return (
    // Same shell as the pipeline page: header card, sidebar card, content panel.
    <div
      className="nf-shell nf-output-page"
      style={{ ['--nf-palette-width' as string]: '300px' }}
    >
      <header className="nf-header">
        <div className="flex w-full items-center justify-between gap-3 px-3 py-1.5">
          <div className="flex min-w-0 items-center gap-3">
            <img src="/icon.png" alt="DBFlow" title="DBFlow" className="h-9 w-9 shrink-0 object-contain drop-shadow-xs" />
            <Button onClick={() => navigate('/')} title="Return to the workflow canvas" aria-label="Back to the workflow canvas" className="shrink-0">
              <ArrowLeft />
              <span className="hidden sm:inline">Back to workflow</span>
            </Button>
          </div>
          <div className="min-w-0 text-right leading-tight">
            <h1 className="truncate text-sm font-semibold text-gray-900">
              Analysis results{isMultiTable ? ` · ${multiTables.length} tables` : ''}
            </h1>
            <p className="truncate text-xs tabular-nums text-gray-500">Opened {currentDateTime.utc}</p>
          </div>
        </div>
      </header>

      <aside className="nf-palette" aria-label="Result sections">
        <CustomSidebar items={sidebarItems} activeTab={activeTab} onTabChange={setActiveTab} />
      </aside>

      <main className="nf-canvas-panel">
        <div key={activeTab} className="nf-fade-in min-h-0 flex-1 overflow-y-auto p-6" role="region" aria-label="Result details" tabIndex={0}>
          {activeTab === 'overview' && (
            <div className="space-y-6">
              <InputChecksSection
                checks={safeResults.inputChecks}
                onMerge={sourceTab ? () => navigate('/', { state: { rerun: { tabId, mergeOverlappingFiles: true } } }) : undefined}
              />
              <div className="grid grid-cols-2 gap-3 xl:grid-cols-4">
                {(isMultiTable
                  ? [
                      { label: 'Dataset tables', value: String(multiTables.length) },
                      { label: 'Cross-table foreign keys', value: String(crossTableRelationships.length) },
                      { label: 'Functional dependencies', value: String(safeResults.stats?.functionalDependencies || fdData.length) },
                      { label: 'Pipeline duration', value: safeResults.duration || '—' },
                    ]
                  : [
                      { label: 'Rows', value: String(safeResults.stats?.cleanedRows ?? safeResults.stats?.originalRows ?? '—') },
                      { label: 'Columns', value: String(safeResults.stats?.attributes ?? '—') },
                      { label: 'Functional dependencies', value: String(safeResults.stats?.functionalDependencies || fdData.length) },
                      { label: 'Pipeline duration', value: safeResults.duration || '—' },
                    ]
                ).map(card => <StatCard key={card.label} label={card.label} value={card.value} />)}
              </div>

              {!isMultiTable && (
                <NormalizationJourney
                  levels={[
                    { id: '1NF', relations: relations1NF },
                    { id: '2NF', relations: relations2NF },
                    { id: '3NF', relations: relationData },
                    { id: 'BCNF', relations: relationsBCNF },
                    { id: '4NF', relations: relations4NF },
                  ].filter(l => l.relations.length > 0).map(({ id, relations }) => ({ id, relations }))}
                />
              )}

              {/* Multi-table column classification and relational overview */}
              {isMultiTable && multiTables.length > 0 && (
                <section className="overflow-hidden rounded-lg border border-gray-200 bg-white shadow-sm">
                  <div className="flex items-center gap-2 border-b border-gray-200 bg-gray-50 px-5 py-3">
                    <Cpu className="size-4 text-teal-600" aria-hidden />
                    <h3 className="text-sm font-semibold text-gray-900">Column classification</h3>
                    <span className="text-xs text-gray-500">entity type detected for every column</span>
                  </div>
                  <div className="space-y-3 p-4">
                    {multiTables.map((t: any) => (
                      <div key={t.tableName} className="rounded-lg border border-gray-200 p-3 transition-shadow hover:shadow-sm">
                        <div className="mb-2 flex items-center justify-between">
                          <span className="font-mono text-sm font-semibold text-teal-800">{t.tableName} <span className="font-normal text-gray-500">({t.fileName})</span></span>
                          <span className="text-xs text-gray-500">{t.rowCount} rows • {t.headers.length} columns</span>
                        </div>
                        <div className="flex flex-wrap gap-2">
                          {t.nerTags?.map((tag: any) => (
                            <span
                              key={tag.columnName}
                              className={`rounded-md border px-2 py-0.5 font-mono text-xs font-medium ${
                                tag.entityType === 'PRIMARY_KEY_ID' ? 'bg-yellow-100 text-yellow-800 border-yellow-300' :
                                tag.entityType === 'FOREIGN_KEY_REF' ? 'bg-blue-100 text-blue-800 border-blue-300' :
                                'bg-gray-50 text-gray-700 border-gray-200'
                              }`}
                              title={tag.description}
                            >
                              {tag.columnName} <span className="text-gray-500">·</span> <span className="font-semibold">{tag.entityType.replace(/_/g, ' ').toLowerCase()}</span>
                            </span>
                          ))}
                        </div>
                      </div>
                    ))}
                  </div>
                </section>
              )}

            </div>
          )}

          {activeTab === 'normal-forms' && (
            <NormalFormSection
              relations1NF={relations1NF}
              relations2NF={relations2NF}
              relations3NF={relationData}
              relationsBCNF={relationsBCNF}
              relations4NF={relations4NF}
              mvds={mvds}
            />
          )}

          {activeTab === 'functional-dependencies' && (
            <FunctionalDependenciesSection fdData={fdData} tableRows={isMultiTable ? Object.fromEntries(multiTables.map((t: any) => [t.tableName, t.rowCount])) : undefined} />
          )}

          {activeTab === 'cardinality' && (
            isMultiTable
              ? <RelationshipsSection relationships={crossTableRelationships} />
              : <CardinalitySection cardinalityAnalysis={cardinalityAnalysis} />
          )}

          {activeTab === 'keys' && (
            isMultiTable ? <KeysByTable tables={multiTables} /> : <KeysSection keyData={keyData} />
          )}

          {activeTab === 'quality' && (
            isMultiTable
              ? <QualityByTable tables={multiTables} />
              : <QualityAssuranceSection losslessCheck={losslessCheck} dependencyPreservation={dependencyPreservation} />
          )}

          {activeTab === 'erd' && erdContent && (
            <ErdSection relations={erdRelations} erdContent={erdContent} />
          )}

          {activeTab === 'sql' && sqlFile?.content && (
            <div className="overflow-hidden rounded-lg border border-gray-200 bg-white shadow-sm">
              <div className="flex items-center justify-between border-b border-gray-200 bg-gray-50 px-5 py-3">
                <h4 className="text-sm font-semibold text-gray-900">
                  SQL schema <span className="hidden font-normal text-gray-500 lg:inline">{isMultiTable ? '· consolidated, all tables' : '· DDL'}</span>
                </h4>
                <div className="flex gap-2">
                  <Button size="sm" onClick={() => void copySql(sqlFile.content)}>
                    {sqlCopied ? <Check className="text-teal-600" /> : <Copy />} {sqlCopied ? 'Copied' : 'Copy SQL'}
                  </Button>
                  <Button
                    variant="primary"
                    size="sm"
                    onClick={() => saveFileInBackground('schema.sql', sqlFile.content, 'application/sql')}
                  >
                    <Download /> Download .sql
                  </Button>
                </div>
              </div>
              <div className="p-4">
                <CodeViewer code={sqlFile.content} language="sql" />
              </div>
            </div>
          )}

          {activeTab === 'report' && reportFile?.content && (
            <ReportView
              results={safeResults}
              markdown={reportFile.content}
              fdData={fdData}
              levels={[
                { id: '1NF', relations: relations1NF },
                { id: '2NF', relations: relations2NF },
                { id: '3NF', relations: relationData },
                { id: 'BCNF', relations: relationsBCNF },
                    { id: '4NF', relations: relations4NF },
              ].filter(l => l.relations.length > 0)}
            />
          )}

          {activeTab === 'files' && safeResults?.outputFiles && (
            <GeneratedFilesSection outputFiles={safeResults.outputFiles} />
          )}
        </div>
      </main>
    </div>
  );
}