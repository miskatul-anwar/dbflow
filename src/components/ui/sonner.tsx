/**
 * Toaster — shadcn's Sonner wrapper, styled with the app's zinc tokens and following its light/dark setting.
 * Desktop: bottom right. Phones: bottom centre, above the bottom bar (and the Run button). Toasts are raised with
 * `toast()` / `notify()` from services/notify, never by components directly.
 */
import { useEffect, useState } from 'react';
import { Toaster as Sonner } from 'sonner';
import { AlertCircle, CheckCircle2, Info, TriangleAlert } from 'lucide-react';

function useDocumentTheme(): 'light' | 'dark' {
  const read = () => (document.documentElement.dataset.theme === 'dark' ? 'dark' : 'light');
  const [theme, setTheme] = useState<'light' | 'dark'>(read);
  useEffect(() => {
    const mo = new MutationObserver(() => setTheme(read()));
    mo.observe(document.documentElement, { attributes: true, attributeFilter: ['data-theme'] });
    return () => mo.disconnect();
  }, []);
  return theme;
}

const MOBILE_BOTTOM = 'calc(env(safe-area-inset-bottom, 0px) + var(--nf-toast-lift, 152px))';

export function Toaster({ mobile = false }: { mobile?: boolean }) {
  const theme = useDocumentTheme();
  return (
    <Sonner
      theme={theme}
      position={mobile ? 'bottom-center' : 'bottom-right'}
      // Phones: clear of the bottom bar and the Run button, or of the progress card while a run is on screen
      // (it sets --nf-toast-lift to its own height).
      offset={mobile ? { bottom: MOBILE_BOTTOM } : 16}
      mobileOffset={mobile ? { bottom: MOBILE_BOTTOM, left: 12, right: 12 } : undefined}
      visibleToasts={3}
      gap={8}
      closeButton={!mobile}
      icons={{
        success: <CheckCircle2 className="size-4 text-emerald-600" />,
        error: <AlertCircle className="size-4 text-red-600" />,
        warning: <TriangleAlert className="size-4 text-amber-600" />,
        info: <Info className="size-4 text-gray-500" />,
      }}
      toastOptions={{
        classNames: {
          // Colours come from .nf-toast in theme.css (light and dark), not from utilities the dark theme remaps.
          toast: 'nf-toast !rounded-[12px] !border !shadow-lg !px-3.5 !py-3 !gap-2.5 !font-sans',
          title: '!text-sm !font-medium !leading-snug',
          description: 'nf-toast-desc !text-xs',
          actionButton: 'nf-toast-action !rounded-[8px] !text-xs !font-medium !h-7 !px-2.5',
          closeButton: 'nf-toast-close',
        },
      }}
    />
  );
}
