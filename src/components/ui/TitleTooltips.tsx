import { useEffect, useRef, useState } from 'react';
import { createPortal } from 'react-dom';
import { AnimatePresence, motion } from 'framer-motion';

/**
 * Shows every `title` attribute in the app as the app's own tooltip instead of the operating system's (a white box
 * that matches nothing else). One listener on the document covers every component, including ones from libraries.
 *
 * While the pointer is over an element, its `title` is moved to `data-nf-title` so the native tooltip does not appear
 * as well; it is put back when the pointer leaves, so the attribute stays for assistive technology and tests.
 */
const DELAY_MS = 350;
const GAP = 8;

interface Tip {
  text: string;
  x: number;
  y: number;
  below: boolean;
}

function titled(target: EventTarget | null): HTMLElement | null {
  if (!(target instanceof Element)) return null;
  const el = target.closest<HTMLElement>('[title], [data-nf-title]');
  if (!el || el.closest('[data-native-title]')) return null;
  return el;
}

export function TitleTooltips() {
  const [tip, setTip] = useState<Tip | null>(null);
  const current = useRef<HTMLElement | null>(null);
  const timer = useRef<number | undefined>(undefined);

  useEffect(() => {
    const restore = (el: HTMLElement | null) => {
      if (!el) return;
      const text = el.getAttribute('data-nf-title');
      if (text !== null) {
        el.setAttribute('title', text);
        el.removeAttribute('data-nf-title');
      }
    };
    const hide = () => {
      window.clearTimeout(timer.current);
      restore(current.current);
      current.current = null;
      setTip(null);
    };
    const show = (el: HTMLElement) => {
      if (current.current === el) return;
      hide();
      const text = (el.getAttribute('title') ?? '').trim();
      if (!text) return;
      el.setAttribute('data-nf-title', text);
      el.removeAttribute('title');
      current.current = el;
      timer.current = window.setTimeout(() => {
        if (current.current !== el || !el.isConnected) return;
        const r = el.getBoundingClientRect();
        const below = r.top < 48;
        const x = Math.min(Math.max(r.left + r.width / 2, 96), window.innerWidth - 96);
        setTip({ text, x, y: below ? r.bottom + GAP : r.top - GAP, below });
      }, DELAY_MS);
    };

    const onOver = (e: Event) => {
      const el = titled(e.target);
      if (el) show(el);
    };
    const onOut = (e: Event) => {
      const el = current.current;
      const to = (e as MouseEvent).relatedTarget;
      if (el && !(to instanceof Node && el.contains(to))) hide();
    };
    const onFocus = (e: Event) => {
      const el = titled(e.target);
      if (el && (e.target as Element).matches(':focus-visible')) show(el);
    };

    document.addEventListener('pointerover', onOver, true);
    document.addEventListener('pointerout', onOut, true);
    document.addEventListener('focusin', onFocus, true);
    document.addEventListener('focusout', hide, true);
    document.addEventListener('pointerdown', hide, true);
    document.addEventListener('keydown', hide, true);
    window.addEventListener('scroll', hide, true);
    window.addEventListener('blur', hide);
    return () => {
      hide();
      document.removeEventListener('pointerover', onOver, true);
      document.removeEventListener('pointerout', onOut, true);
      document.removeEventListener('focusin', onFocus, true);
      document.removeEventListener('focusout', hide, true);
      document.removeEventListener('pointerdown', hide, true);
      document.removeEventListener('keydown', hide, true);
      window.removeEventListener('scroll', hide, true);
      window.removeEventListener('blur', hide);
    };
  }, []);

  return createPortal(
    <AnimatePresence>
      {tip && (
        <div
          key={`${tip.text}-${tip.x}-${tip.y}`}
          className="pointer-events-none fixed z-[100]"
          style={{ left: tip.x, top: tip.y, transform: `translate(-50%, ${tip.below ? '0' : '-100%'})` }}
        >
          <motion.div
            role="tooltip"
            data-native-title
            initial={{ opacity: 0, y: tip.below ? -2 : 2 }}
            animate={{ opacity: 1, y: 0 }}
            exit={{ opacity: 0 }}
            transition={{ duration: 0.12 }}
            className="nf-tooltip max-w-72 rounded-md bg-zinc-900 px-2.5 py-1.5 text-xs leading-snug text-zinc-50 shadow-md"
          >
            {tip.text}
          </motion.div>
        </div>
      )}
    </AnimatePresence>,
    document.body,
  );
}
