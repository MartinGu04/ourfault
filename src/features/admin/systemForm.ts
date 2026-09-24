// Conversion between the system form (plain strings, one address per line)
// and the backend's input shape. Validation itself happens in the backend.

import type { System, SystemInput } from '../../api/types';

export interface SystemFormValues {
  name: string;
  /** One address per line (commas and semicolons also accepted). */
  distributionList: string;
}

export const emptySystemForm: SystemFormValues = { name: '', distributionList: '' };

export function formFromSystem(system: System): SystemFormValues {
  return { name: system.name, distributionList: system.distributionList.join('\n') };
}

const addresses = (value: string) =>
  value
    .split(/[\n,;]+/)
    .map((address) => address.trim())
    .filter(Boolean);

export function inputFromForm(id: string | null, values: SystemFormValues): SystemInput {
  return { id, name: values.name, distributionList: addresses(values.distributionList) };
}
