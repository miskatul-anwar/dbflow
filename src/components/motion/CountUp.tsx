import { useLayoutEffect, useRef } from 'react';
import gsap from 'gsap';

const reducedMotion = () =>
  typeof window !== 'undefined' && typeof window.matchMedia === 'function' && window.matchMedia('(prefers-reduced-motion: reduce)').matches;

/**
 * A figure that counts up to its value once (GSAP). Text that is not a plain number (e.g. "1.2 s", "—") is shown as
 * it is, and so is everything when the system asks for reduced motion.
 */
export function CountUp({ value, duration = 0.7 }: { value: string; duration?: number }) {
  const ref = useRef<HTMLSpanElement>(null);
  useLayoutEffect(() => {
    const el = ref.current;
    if (!el) return;
    const target = /^\d+$/.test(value) ? Number(value) : NaN;
    if (Number.isNaN(target) || target === 0 || reducedMotion()) {
      el.textContent = value;
      return;
    }
    const state = { n: 0 };
    const tween = gsap.to(state, {
      n: target,
      duration,
      ease: 'power2.out',
      onUpdate: () => {
        el.textContent = Math.round(state.n).toLocaleString();
      },
      onComplete: () => {
        el.textContent = target.toLocaleString();
      },
    });
    return () => {
      tween.kill();
    };
  }, [value, duration]);
  return (
    <span ref={ref} aria-label={value}>
      {value}
    </span>
  );
}
