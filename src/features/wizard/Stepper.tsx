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
              <span className={count > 0 ? 'stepper-marker stepper-marker-issue' : 'stepper-marker'}>
                {count > 0 ? count : status === 'complete' ? <Icon name="check" size={14} /> : index + 1}
              </span>
              <span className="stepper-label">{step.label}</span>
            </button>
          </li>
        );
      })}
    </ol>
  );
}
