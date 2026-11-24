/**
 * One figure with a label. Every summary strip on the Output page uses this, so they all look alike:
 * a neutral card, the number in ink, and a small dot that carries the meaning (good / watch / problem).
 */
import { CountUp } from '../motion/CountUp';

export type Tone = 'good' | 'watch' | 'bad' | 'info' | 'neutral';

const DOT: Record<Tone, string> = {
  good: 'bg-green-500',
  watch: 'bg-amber-500',
  bad: 'bg-red-500',
  info: 'bg-blue-500',
  neutral: 'bg-gray-300',
};

export function StatCard({ label, value, note, tone = 'neutral' }: { label: string; value: React.ReactNode; note?: string; tone?: Tone }) {
  return (
    <div className="rounded-xl border border-gray-200 bg-white p-4">
      <div className="flex items-center gap-2 text-xs text-gray-500">
        {tone !== 'neutral' && <span className={`size-1.5 shrink-0 rounded-full ${DOT[tone]}`} aria-hidden />}
        {label}
      </div>
      <div className="mt-1 text-2xl font-semibold tracking-tight tabular-nums text-gray-900">
        {typeof value === 'string' ? <CountUp value={value} /> : value}
      </div>
      {note && <div className="mt-0.5 text-xs text-gray-500">{note}</div>}
    </div>
  );
}
