import * as React from 'react';
import { Slot } from '@radix-ui/react-slot';
import { cva, type VariantProps } from 'class-variance-authority';
import { Loader2 } from 'lucide-react';

import { cn } from '@/lib/utils';
import { WithTooltip } from './tooltip';

/**
 * The one button of the app. Every clickable action uses it, so shape, height,
 * colour, focus ring and disabled state are identical everywhere.
 *
 *  - `primary`   solid ink (near-black; near-white in dark): the single main action of a region (Run, Apply)
 *  - `secondary` a hairline border: every other action (Open, Save, Copy, Output …)
 *  - `ghost`     no border: low-emphasis actions (Cancel, Show all, row actions, undo/redo)
 *
 * Labelling rule: actions show an icon AND a text label. Icon-only buttons
 * (`size="icon"`/`"icon-sm"`) are for undo/redo, tab chrome and per-row actions, and must go
 * through `IconButton`, which requires a label for the tooltip and screen readers.
 */
export const buttonVariants = cva(
  [
    'inline-flex shrink-0 items-center justify-center gap-2 whitespace-nowrap rounded-lg',
    'text-sm font-medium transition-[color,background-color,border-color,transform] duration-150 select-none active:scale-[0.97]',
    'focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-gray-400 focus-visible:ring-offset-1',
    'disabled:cursor-not-allowed disabled:opacity-50',
    '[&_svg]:pointer-events-none [&_svg]:shrink-0 [&_svg:not([class*="size-"])]:size-4',
  ],
  {
    variants: {
      variant: {
        // Ink on paper: near-black with white text (near-white with black text in dark, see theme.css).
        primary: 'nf-btn-primary bg-[#18181b] text-white hover:bg-[#27272a]',
        secondary: 'border border-gray-200 bg-white text-gray-800 hover:bg-gray-100',
        ghost: 'text-gray-700 hover:bg-gray-100 hover:text-gray-900',
        // Destructive confirmations (6.2:1 against white text, WCAG AA); the same colour in both themes.
        danger: 'bg-[#b42318] text-white shadow-xs hover:bg-[#912018]',
      },
      size: {
        sm: 'h-8 px-3',
        md: 'h-9 px-3.5',
        'icon-sm': 'size-8',
        icon: 'size-9',
      },
    },
    defaultVariants: { variant: 'secondary', size: 'md' },
  },
);

export interface ButtonProps extends React.ComponentProps<'button'>, VariantProps<typeof buttonVariants> {
  asChild?: boolean;
  /** Shows a spinner in place of the icon and disables the button. */
  loading?: boolean;
}

export function Button({ className, variant, size, asChild = false, loading = false, children, disabled, ...props }: ButtonProps) {
  const Comp = asChild ? Slot : 'button';
  return (
    <Comp
      data-slot="button"
      type={asChild ? undefined : (props.type ?? 'button')}
      className={cn(buttonVariants({ variant, size }), className)}
      disabled={disabled || loading}
      aria-busy={loading || undefined}
      {...props}
    >
      {loading && !asChild ? <Loader2 className="animate-spin" aria-hidden /> : null}
      {children}
    </Comp>
  );
}

export interface IconButtonProps extends Omit<ButtonProps, 'size' | 'children' | 'aria-label'> {
  /** Required: becomes both the tooltip and the accessible name. */
  label: string;
  icon: React.ReactNode;
  size?: 'icon' | 'icon-sm';
}

export function IconButton({ label, icon, size = 'icon', variant = 'ghost', title, ...props }: IconButtonProps) {
  const button = (
    // A disabled button gets no pointer events, so it keeps a native tooltip instead.
    <Button variant={variant} size={size} aria-label={label} title={props.disabled ? (title ?? label) : undefined} {...props}>
      {icon}
    </Button>
  );
  return props.disabled ? button : <WithTooltip content={title ?? label}>{button}</WithTooltip>;
}
