import { Icon } from '../../ui/Icon';
import type { WizardStep } from './wizardState';

const STEPS: { key: Exclude<WizardStep, 'done'>; label: string }[] = [
  { key: 'rows', label: 'הדבקת שורות' },
  { key: 'details', label: 'פרטי התחקיר' },
  { key: 'preview', label: 'תצוגה מקדימה' },
];

export function Stepper({ current }: { current: WizardStep }) {
  const currentIndex = current === 'done' ? STEPS.length : STEPS.findIndex((step) => step.key === current);
  return (
    <ol className="stepper" aria-label="שלבי יצירת התחקיר">
      {STEPS.map((step, index) => {
        const status = index < currentIndex ? 'complete' : index === currentIndex ? 'current' : 'upcoming';
        return (
          <li key={step.key} className={`stepper-item stepper-${status}`} aria-current={status === 'current' ? 'step' : undefined}>
            <span className="stepper-marker">{status === 'complete' ? <Icon name="check" size={14} /> : index + 1}</span>
            <span className="stepper-label">{step.label}</span>
          </li>
        );
      })}
    </ol>
  );
}
