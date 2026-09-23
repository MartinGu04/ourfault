// Basic building blocks shared by all screens.

import { useId, type ButtonHTMLAttributes, type ReactNode } from 'react';

import { Icon, type IconName } from './Icon';

type ButtonVariant = 'primary' | 'secondary' | 'subtle';

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
          <Icon name="alert" size={14} />
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

type BannerTone = 'info' | 'error' | 'success';

export function Banner({ tone = 'info', children }: { tone?: BannerTone; children: ReactNode }) {
  const icon: IconName = tone === 'error' ? 'alert' : tone === 'success' ? 'check' : 'info';
  return (
    <div className={`banner banner-${tone}`} role={tone === 'error' ? 'alert' : 'status'}>
      <Icon name={icon} />
      <div>{children}</div>
    </div>
  );
}

export function Badge({ tone = 'neutral', children }: { tone?: 'neutral' | 'accent' | 'success' | 'muted'; children: ReactNode }) {
  return <span className={`badge badge-${tone}`}>{children}</span>;
}

/** Left-to-right text (URLs, e-mail addresses, file names) inside RTL layout. */
export function Ltr({ children, className }: { children: ReactNode; className?: string }) {
  return (
    <bdi dir="ltr" className={className}>
      {children}
    </bdi>
  );
}
