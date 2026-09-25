import type { DraftStep } from '../../api/types';
import { STEPS } from '../../lib/labels';
import { Icon } from '../../ui/Icon';

interface Props {
  current: DraftStep;
  /** Number of known problems per step. */
  issues: Partial<Record<DraftStep, number>>;
  onSelect: (step: DraftStep) => void;
}

/** The wizard steps. Drafts may be incomplete, so every step can be opened. */
export function Stepper({ current, issues, onSelect }: Props) {
  const currentIndex = STEPS.findIndex((step) => step.key === current);
  return (
    <ol className="stepper" aria-label="שלבי התחקיר">
      {STEPS.map((step, index) => {
        const status = index < currentIndex ? 'complete' : index === currentIndex ? 'current' : 'upcoming';
        const count = issues[step.key] ?? 0;
        return (
          <li key={step.key} className={`stepper-item stepper-${status}`}>
            <button
              type="button"
              className="stepper-button"
              aria-current={status === 'current' ? 'step' : undefined}
              onClick={() => onSelect(step.key)}
            >
              <span className="stepper-marker">
                {status === 'complete' ? <Icon name="check" size={13} /> : index + 1}
              </span>
              <span className="stepper-label">{step.label}</span>
              {count > 0 && (
                <span className="stepper-issues" title={`${count} פריטים לתיקון בשלב זה`}>
                  <Icon name="error" size={11} />
                  {count}
                  <span className="visually-hidden"> פריטים לתיקון</span>
                </span>
              )}
            </button>
          </li>
        );
      })}
    </ol>
  );
}
