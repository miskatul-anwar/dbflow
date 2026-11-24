import React from 'react';
import { motion } from 'framer-motion';
import { PHASE_ACCENTS } from '../../config/operationVisuals';

/** Which section of the sidebar an item belongs to (same grouping style as the operations palette). */
export type SidebarGroup = 'summary' | 'analysis' | 'generated';

export interface SidebarItem {
  id: string;
  label: string;
  icon: React.ReactNode;
  /** One line under the label, like the operations palette. */
  description?: string;
  count?: number;
  hasData?: boolean;
  group?: SidebarGroup;
}

export interface CustomSidebarProps {
  items: SidebarItem[];
  activeTab: string;
  onTabChange: (tab: string) => void;
}

const GROUPS: Array<{ id: SidebarGroup; label: string; accent: keyof typeof PHASE_ACCENTS }> = [
  { id: 'summary', label: 'Summary', accent: 'PREP' },
  { id: 'analysis', label: 'Analysis', accent: 'ANALYZE' },
  { id: 'generated', label: 'Generated', accent: 'OUTPUT' },
];

/**
 * Results navigation. Mirrors the operations palette on the pipeline page: a titled header, grouped sections with
 * a coloured dot, and rows with an icon and a name (the description is the tooltip). The active row's highlight
 * glides between rows.
 */
export function CustomSidebar({ items, activeTab, onTabChange }: CustomSidebarProps) {
  const available = items.filter(i => i.hasData).length;

  return (
    <nav className="flex h-full min-h-0 flex-col overflow-hidden rounded-[inherit]" aria-label="Results" data-testid="results-sidebar">
      <div className="flex shrink-0 items-baseline gap-2 border-b border-gray-200 px-3 py-2.5">
        <h2 className="text-sm font-semibold text-gray-900">Results</h2>
        <span className="text-xs tabular-nums text-gray-500" aria-label={`${available} of ${items.length} sections have data`}>
          {available}/{items.length}
        </span>
      </div>

      <div className="min-h-0 flex-1 overflow-y-auto overscroll-contain px-2 pb-2">
        {GROUPS.map(group => {
          const groupItems = items.filter(i => (i.group ?? 'analysis') === group.id);
          if (groupItems.length === 0) return null;
          const meta = PHASE_ACCENTS[group.accent];
          return (
            <section key={group.id} aria-label={group.label} className="mt-2 first:mt-0">
              <h3 className="sticky top-0 z-10 flex items-center gap-2 bg-[var(--nf-surface)] px-1.5 pb-1 pt-2 text-xs font-medium text-gray-500">
                <span className="size-2 rounded-full" style={{ backgroundColor: meta.accent }} aria-hidden />
                {group.label}
              </h3>
              <ul className="space-y-0.5 pt-1">
                {groupItems.map(item => {
                  const active = activeTab === item.id;
                  const enabled = Boolean(item.hasData);
                  return (
                    <li key={item.id}>
                      <button
                        type="button"
                        onClick={() => onTabChange(item.id)}
                        disabled={!enabled}
                        aria-current={active ? 'page' : undefined}
                        title={enabled ? item.description : 'No data'}
                        className={[
                          'group relative flex w-full items-center gap-2 rounded-lg px-2 py-1.5 text-left transition-colors',
                          'focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-gray-400',
                          active ? '' : enabled ? 'hover:bg-gray-100' : 'cursor-not-allowed opacity-40',
                        ].join(' ')}
                      >
                        {active && (
                          <motion.span
                            layoutId="nf-results-active"
                            className="absolute inset-0 rounded-lg bg-gray-100"
                            transition={{ type: 'spring', stiffness: 500, damping: 40 }}
                            aria-hidden
                          />
                        )}
                        <span className={`relative flex size-5 shrink-0 items-center justify-center [&_svg]:size-4 ${active ? 'text-gray-900' : 'text-gray-500'}`} aria-hidden>
                          {item.icon}
                        </span>
                        <span className={`relative min-w-0 flex-1 truncate text-sm ${active ? 'font-medium text-gray-900' : 'text-gray-700'}`}>{item.label}</span>
                        {enabled && item.count !== undefined && (
                          <span className="relative shrink-0 text-xs tabular-nums text-gray-500">{item.count}</span>
                        )}
                      </button>
                    </li>
                  );
                })}
              </ul>
            </section>
          );
        })}
      </div>
    </nav>
  );
}
