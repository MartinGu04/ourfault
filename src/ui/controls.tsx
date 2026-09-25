// Basic building blocks shared by all screens.

import { useId, type ButtonHTMLAttributes, type ReactNode } from 'react';

import { Icon, type IconName } from './Icon';

/** primary: the one main action · secondary: normal · subtle: low emphasis · danger: destructive, low emphasis. */
type ButtonVariant = 'primary' | 'secondary' | 'subtle' | 'danger';

interface ButtonProps extends ButtonHTMLAttributes<HTMLButtonElement> {
  variant?: ButtonVariant;
  icon?: IconName;
  /** Icon after the label (e.g. "forward" on a continue button). */
  trailingIcon?: IconName;
  busy?: boolean;
  size?: 'normal' | 'large';
}

export function Button({
  variant = 'secondary',
  icon,
  trailingIcon,
  busy = false,
  size = 'normal',
  className,
  children,
  disabled,
  type = 'button',
  ...rest
}: ButtonProps) {
  const classes = ['btn', `btn-${variant}`, size === 'large' ? 'btn-large' : '', className ?? '']
    .filter(Boolean)
    .join(' ');
  return (
    <button type={type} className={classes} disabled={disabled || busy} aria-busy={busy || undefined} {...rest}>
      {busy ? <Spinner /> : icon && <Icon name={icon} />}
      {children}
      {trailingIcon && !busy && <Icon name={trailingIcon} />}
    </button>
  );
}

export function Spinner({ label }: { label?: string }) {
  return <span className="spinner" role={label ? 'status' : undefined} aria-label={label} />;
}

interface FieldProps {
  label: string;
  hint?: ReactNode;
  error?: string | undefined;
  /** Renders the control; receives ids for accessible wiring. */
  children: (ids: { id: string; describedBy: string | undefined; invalid: boolean }) => ReactNode;
}

export function Field({ label, hint, error, children }: FieldProps) {
  const id = useId();
  const hintId = `${id}-hint`;
  const errorId = `${id}-error`;
  const describedBy = [error ? errorId : null, hint ? hintId : null].filter(Boolean).join(' ') || undefined;
  return (
    <div className={error ? 'field field-invalid' : 'field'}>
      <label className="field-label" htmlFor={id}>
        {label}
      </label>
      {children({ id, describedBy, invalid: Boolean(error) })}
      {error && (
        <p className="field-error" id={errorId}>
          <Icon name="error" size={14} />
          {error}
        </p>
      )}
      {hint && (
        <p className="field-hint" id={hintId}>
          {hint}
        </p>
      )}
    </div>
  );
}

/** A read-only value laid out like a form field. */
export function StaticField({ label, children }: { label: string; children: ReactNode }) {
  return (
    <div className="field">
      <span className="field-label">{label}</span>
      {children}
    </div>
  );
}

export type Tone = 'error' | 'warning' | 'info' | 'success';

/** Each severity has its own icon, so colour is never the only signal. */
export const TONE_ICON: Record<Tone, IconName> = {
  error: 'error',
  warning: 'alert',
  info: 'info',
  success: 'check',
};

export function Banner({ tone = 'info', children }: { tone?: Tone; children: ReactNode }) {
  const icon = TONE_ICON[tone];
  return (
    <div className={`banner tone-${tone}`} role={tone === 'error' ? 'alert' : 'status'}>
      <Icon name={icon} />
      <div>{children}</div>
    </div>
  );
}

type BadgeTone = 'neutral' | 'accent' | 'success' | 'muted' | 'warning' | 'info';

export function Badge({ tone = 'neutral', children }: { tone?: BadgeTone; children: ReactNode }) {
  return <span className={`badge badge-${tone}`}>{children}</span>;
}

/** A compact, intentional empty state: icon, one line, optional hint and action. */
export function EmptyState({
  icon,
  title,
  children,
  action,
  compact,
}: {
  icon: IconName;
  title: string;
  children?: ReactNode;
  action?: ReactNode;
  compact?: boolean;
}) {
  return (
    <div className={compact ? 'empty-state empty-state-compact' : 'empty-state'}>
      <span className="empty-state-icon">
        <Icon name={icon} size={compact ? 18 : 22} />
      </span>
      <div className="empty-state-text">
        <p className="empty-state-title">{title}</p>
        {children && <p className="empty-state-hint">{children}</p>}
      </div>
      {action && <div className="empty-state-action">{action}</div>}
    </div>
  );
}

/** Left-to-right text (URLs, e-mail addresses, file names) inside RTL layout. */
export function Ltr({ children, className }: { children: ReactNode; className?: string }) {
  return (
    <bdi dir="ltr" className={className}>
      {children}
    </bdi>
  );
}

interface SegmentedProps<T extends string | boolean> {
  label: string;
  options: readonly { value: T; label: string }[];
  value: T | null;
  onChange: (value: T) => void;
  invalid?: boolean;
  describedBy?: string | undefined;
}

/** A small set of mutually exclusive choices (e.g. yes / no). Nothing is preselected. */
export function Segmented<T extends string | boolean>({
  label,
  options,
  value,
  onChange,
  invalid,
  describedBy,
}: SegmentedProps<T>) {
  return (
    <div
      className={invalid ? 'segmented segmented-invalid' : 'segmented'}
      role="radiogroup"
      aria-label={label}
      aria-invalid={invalid || undefined}
      aria-describedby={describedBy}
    >
      {options.map((option) => (
        <button
          key={String(option.value)}
          type="button"
          role="radio"
          aria-checked={value === option.value}
          className="segmented-option"
          onClick={() => onChange(option.value)}
        >
          {option.label}
        </button>
      ))}
    </div>
  );
}

export const YES_NO: readonly { value: boolean; label: string }[] = [
  { value: true, label: 'כן' },
  { value: false, label: 'לא' },
];

interface ChipsProps {
  label: string;
  options: readonly { value: string; label: string; disabled?: boolean }[];
  selected: readonly string[];
  onChange: (selected: string[]) => void;
  invalid?: boolean;
  describedBy?: string | undefined;
}

/** Multi-select as toggle chips, in the order the options are given. */
export function Chips({ label, options, selected, onChange, invalid, describedBy }: ChipsProps) {
  const toggle = (value: string) =>
    onChange(selected.includes(value) ? selected.filter((v) => v !== value) : [...selected, value]);
  return (
    <div
      className={invalid ? 'chips chips-invalid' : 'chips'}
      role="group"
      aria-label={label}
      aria-describedby={describedBy}
    >
      {options.map((option) => {
        const checked = selected.includes(option.value);
        return (
          <button
            key={option.value}
            type="button"
            role="checkbox"
            aria-checked={checked}
            className="chip"
            disabled={option.disabled && !checked}
            onClick={() => toggle(option.value)}
          >
            {checked && <Icon name="check" size={14} />}
            {option.label}
          </button>
        );
      })}
    </div>
  );
}

interface TabsProps<T extends string> {
  label: string;
  tabs: readonly { key: T; label: string; badge?: ReactNode }[];
  current: T;
  onChange: (key: T) => void;
}

export function Tabs<T extends string>({ label, tabs, current, onChange }: TabsProps<T>) {
  return (
    <div className="tabs" role="tablist" aria-label={label}>
      {tabs.map((tab) => (
        <button
          key={tab.key}
          type="button"
          role="tab"
          aria-selected={tab.key === current}
          className="tab"
          onClick={() => onChange(tab.key)}
        >
          {tab.label}
          {tab.badge}
        </button>
      ))}
    </div>
  );
}
