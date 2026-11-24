import { motion } from 'framer-motion';
import { useEffect, useRef, type ReactNode, type RefObject } from 'react';
import { createPortal } from 'react-dom';

interface ModalProps {
  /** Id of the element that names the dialog (its heading). */
  labelledBy: string;
  /** Id of the element that describes it. */
  describedBy?: string;
  /** Escape and a click on the backdrop call this; omit it where the dialog must be answered. */
  onCancel?: () => void;
  /** Element focused when the dialog opens (default: the first focusable one). */
  initialFocus?: RefObject<HTMLElement | null>;
  /** `alertdialog` for questions about destructive actions. */
  role?: 'dialog' | 'alertdialog';
  children: ReactNode;
}

const FOCUSABLE = 'input:not([disabled]), button:not([disabled]), textarea:not([disabled]), select:not([disabled]), [tabindex]:not([tabindex="-1"])';

/**
 * The one modal shell of the app, so every dialog looks and behaves the same: centred card over a dimmed page,
 * focus kept inside (Tab wraps), Escape and a backdrop click cancel where allowed, and the page behind made inert
 * for the keyboard and screen readers. Focus returns to where it was when the dialog closes.
 * It renders on `<body>` (a portal), outside the app root, so everything else on the page can be made inert.
 */
export function Modal({ labelledBy, describedBy, onCancel, initialFocus, role = 'dialog', children }: ModalProps) {
  const boxRef = useRef<HTMLDivElement>(null);

  useEffect(() => {
    const previous = document.activeElement as HTMLElement | null;
    const target = initialFocus?.current ?? boxRef.current?.querySelector<HTMLElement>(FOCUSABLE);
    target?.focus();
    if (target instanceof HTMLInputElement) target.select();
    // Everything on the page except this dialog (it is portalled to <body>) becomes inert.
    const backdrop = boxRef.current?.parentElement;
    const behind = Array.from(document.body.children).filter(c => c !== backdrop && !c.hasAttribute('inert'));
    behind.forEach(el => el.setAttribute('inert', ''));
    return () => {
      behind.forEach(el => el.removeAttribute('inert'));
      if (previous && document.contains(previous)) previous.focus();
    };
    // Runs once per opening; the focus target is read when the dialog appears.
  }, []);

  const onKeyDown = (e: React.KeyboardEvent) => {
    if (e.key === 'Escape') {
      e.stopPropagation();
      onCancel?.();
    } else if (e.key === 'Tab') {
      const focusable = boxRef.current?.querySelectorAll<HTMLElement>(FOCUSABLE);
      if (!focusable || focusable.length === 0) return;
      const first = focusable[0];
      const last = focusable[focusable.length - 1];
      if (e.shiftKey && document.activeElement === first) {
        e.preventDefault();
        last.focus();
      } else if (!e.shiftKey && document.activeElement === last) {
        e.preventDefault();
        first.focus();
      }
    }
  };

  return createPortal(
    <motion.div
      className="nf-modal-backdrop"
      initial={{ opacity: 0 }}
      animate={{ opacity: 1 }}
      transition={{ duration: 0.15 }}
      onMouseDown={e => e.target === e.currentTarget && onCancel?.()}
    >
      <motion.div
        initial={{ opacity: 0, scale: 0.97, y: 6 }}
        animate={{ opacity: 1, scale: 1, y: 0 }}
        transition={{ duration: 0.2, ease: [0.16, 1, 0.3, 1] }}
        ref={boxRef}
        role={role}
        aria-modal="true"
        aria-labelledby={labelledBy}
        aria-describedby={describedBy}
        className="nf-modal"
        onKeyDown={onKeyDown}
      >
        {children}
      </motion.div>
    </motion.div>,
    document.body,
  );
}
