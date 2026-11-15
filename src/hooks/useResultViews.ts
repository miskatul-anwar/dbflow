/**
 * Everything the results pages show, derived once from an engine result: the relations at each normal
 * form, the ER diagram's relations, dependencies, keys, checks and the SQL and report files. Shared by
 * the desktop Output page and the phone results screen, so both always show the same numbers.
 *
 * Results are untyped here (the engine's result shape differs for one file and many).
 */
/* eslint-disable @typescript-eslint/no-explicit-any */
import { useMemo } from 'react';

export function useResultViews(safeResults: any) {
  const isMultiTable = safeResults?.isMultiTable || false;
  const multiTables = safeResults?.tables || [];
  const crossTableRelationships = safeResults?.crossTableRelationships || [];

  const relationData = useMemo(() => {
    if (safeResults?.relations && safeResults.relations.length > 0) return safeResults.relations;
    if (multiTables.length > 0) {
      return multiTables.flatMap((t: any) => t.thirdNormalForm || []);
    }
    return [];
  }, [safeResults, multiTables]);

  const relations1NF = useMemo(() => {
    if (safeResults?.relations1NF && safeResults.relations1NF.length > 0) return safeResults.relations1NF;
    if (multiTables.length > 0) {
      return multiTables.flatMap((t: any) => t.firstNormalForm || []);
    }
    return [];
  }, [safeResults, multiTables]);

  const relations2NF = useMemo(() => {
    if (safeResults?.relations2NF && safeResults.relations2NF.length > 0) return safeResults.relations2NF;
    if (multiTables.length > 0) {
      return multiTables.flatMap((t: any) => t.secondNormalForm || []);
    }
    return [];
  }, [safeResults, multiTables]);

  const relationsBCNF = useMemo(() => {
    if (safeResults?.bcnf && safeResults.bcnf.length > 0) return safeResults.bcnf;
    if (multiTables.length > 0) {
      return multiTables.flatMap((t: any) => t.bcnf || []);
    }
    return [];
  }, [safeResults, multiTables]);

  const relations4NF = useMemo(() => {
    if (safeResults?.relations4NF && safeResults.relations4NF.length > 0) return safeResults.relations4NF;
    if (multiTables.length > 0) {
      return multiTables.flatMap((t: any) => t.fourthNormalForm || []);
    }
    return [];
  }, [safeResults, multiTables]);

  /** Multivalued dependencies the 4NF step split on (one file, or every table of a multi-file run). */
  const mvds = useMemo(() => {
    if (Array.isArray(safeResults?.multivaluedDependencies)) return safeResults.multivaluedDependencies;
    return multiTables.flatMap((t: any) => t.multivaluedDependencies || []);
  }, [safeResults, multiTables]);

  // The ERD follows the final schema (BCNF if it ran, otherwise 3NF).
  const erdRelations = useMemo(() => {
    const final = safeResults?.relations;
    if (Array.isArray(final) && final.length > 0) return final;
    return relationsBCNF.length > 0 ? relationsBCNF : relationData;
  }, [safeResults, relationsBCNF, relationData]);

  const erdContent = safeResults?.erdContent || safeResults?.erdDiagram || '';
  const cardinalityAnalysis = safeResults?.cardinalityAnalysis || {};
  const losslessCheck = safeResults?.losslessCheck || null;
  const dependencyPreservation = safeResults?.dependencyPreservation || null;

  const fdData = useMemo(() => {
    if (safeResults?.functionalDependencies && Array.isArray(safeResults.functionalDependencies)) {
      return safeResults.functionalDependencies;
    }
    if (multiTables.length > 0) {
      return multiTables.flatMap((t: any) => (t.functionalDependencies || []).map((fd: any) => ({ ...fd, table: t.tableName })));
    }
    if (safeResults?.outputFiles && Array.isArray(safeResults.outputFiles)) {
      const fdFile = safeResults.outputFiles.find((f: any) => f?.name?.includes('functional_dependencies') && f?.type === 'json');
      if (fdFile?.content && Array.isArray(fdFile.content)) {
        return fdFile.content;
      }
    }
    return [];
  }, [safeResults, multiTables]);

  const keyData = useMemo(() => {
    if (safeResults?.candidateKeys || safeResults?.primaryKey) {
      return {
        candidateKeys: safeResults.candidateKeys?.map((k: any) => k.attributes || k) || [],
        superkeys: []
      };
    }
    if (multiTables.length > 0) {
      const keys = multiTables.flatMap((t: any) => t.candidateKeys || []);
      if (keys.length > 0) {
        return {
          candidateKeys: keys.map((k: any) => k.attributes || k),
          superkeys: []
        };
      }
    }
    if (safeResults?.outputFiles && Array.isArray(safeResults.outputFiles)) {
      const keyFile = safeResults.outputFiles.find((f: any) => f?.name?.includes('keys_analysis') && f?.type === 'json');
      return keyFile?.content || null;
    }
    return null;
  }, [safeResults, multiTables]);

  const sqlFile = useMemo(() => {
    const files = safeResults?.outputFiles || [];
    const found = files.find((f: any) => f?.type === "sql");
    const rawContent = found?.content || safeResults?.sqlStatements || '';
    const content = typeof rawContent === 'string' ? rawContent : JSON.stringify(rawContent, null, 2);
    return { content };
  }, [safeResults]);

  const reportFile = useMemo(() => {
    const files = safeResults?.outputFiles || [];
    const found = files.find((f: any) => f?.name?.includes("report"));
    const rawContent = found?.content || safeResults?.reportMarkdown || '';
    const content = typeof rawContent === 'string' ? rawContent : JSON.stringify(rawContent, null, 2);
    return { content };
  }, [safeResults]);


  return {
    isMultiTable,
    multiTables,
    crossTableRelationships,
    relationData,
    relations1NF,
    relations2NF,
    relationsBCNF,
    relations4NF,
    mvds,
    erdRelations,
    erdContent,
    cardinalityAnalysis,
    losslessCheck,
    dependencyPreservation,
    fdData,
    keyData,
    sqlFile,
    reportFile,
  };
}

export type ResultViews = ReturnType<typeof useResultViews>;
