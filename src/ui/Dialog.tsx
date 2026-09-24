import { useEffect, useRef, type ReactNode } from 'react';

import { Icon } from './Icon';

interface DialogProps {
  title: string;
  subtitle?: ReactNode;
  onClose: () => void;
  footer?: ReactNode;
  children: ReactNode;
}

/** Modal dialog on the native <dialog> element (focus trap and Esc built in). */
export function Dialog({ title, subtitle, onClose, footer, children }: DialogProps) {
  const ref = useRef<HTMLDialogElement>(null);

  useEffect(() => {
    const dialog = ref.current;
    if (dialog && !dialog.open) dialog.showModal();
    return () => dialog?.close();
  }, []);

  return (
    <dialog
      ref={ref}
      className="dialog"
      aria-labelledby="dialog-title"
      onCancel={(event) => {
        event.preventDefault();
        onClose();
      }}
    >
      <header className="dialog-header">
        <div>
          <h2 id="dialog-title">{title}</h2>
          {subtitle && <p className="dialog-subtitle">{subtitle}</p>}
        </div>
        <button type="button" className="icon-button" onClick={onClose} aria-label="סגירה">
          <Icon name="close" />
        </button>
      </header>
      <div className="dialog-body">{children}</div>
      {footer && <footer className="dialog-footer">{footer}</footer>}
    </dialog>
  );
}
