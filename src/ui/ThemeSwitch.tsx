// Light / Dark / System, as a compact segmented control in the top bar.

import type { ThemePreference } from '../lib/theme';
import { Icon, type IconName } from './Icon';

const OPTIONS: { value: ThemePreference; label: string; icon: IconName }[] = [
  { value: 'light', label: 'בהיר', icon: 'sun' },
  { value: 'dark', label: 'כהה', icon: 'moon' },
  { value: 'system', label: 'לפי המערכת', icon: 'monitor' },
];

export function ThemeSwitch({ value, onChange }: { value: ThemePreference; onChange: (value: ThemePreference) => void }) {
  return (
    <div className="theme-switch" role="radiogroup" aria-label="ערכת נושא">
      {OPTIONS.map((option) => (
        <button
          key={option.value}
          type="button"
          role="radio"
          aria-checked={value === option.value}
          aria-label={option.label}
          title={option.label}
          className="theme-switch-option"
          onClick={() => onChange(option.value)}
        >
          <Icon name={option.icon} size={16} />
        </button>
      ))}
    </div>
  );
}
