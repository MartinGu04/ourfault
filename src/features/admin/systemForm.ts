// Conversion between the admin form (plain strings, one entry per line) and
// the backend's System shape. Validation itself happens in the backend.

import type { System, SystemInput } from '../../api/types';

export interface SystemFormValues {
  name: string;
  templateName: string;
  templateTitle: string;
  /** One section heading per line. */
  sections: string;
  /** One address per line (commas and semicolons also accepted). */
  distributionList: string;
  siteUrl: string;
  library: string;
}

export const emptySystemForm: SystemFormValues = {
  name: '',
  templateName: '',
  templateTitle: 'תחקיר אירוע',
  sections: ['רקע', 'השתלשלות האירוע', 'ממצאים', 'לקחים'].join('\n'),
  distributionList: '',
  siteUrl: 'https://sharepoint.example.com/sites/',
  library: 'Investigations',
};

export function formFromSystem(system: System): SystemFormValues {
  return {
    name: system.name,
    templateName: system.template.name,
    templateTitle: system.template.title,
    sections: system.template.sections.join('\n'),
    distributionList: system.distributionList.join('\n'),
    siteUrl: system.sharepoint.siteUrl,
    library: system.sharepoint.library,
  };
}

const lines = (value: string) =>
  value
    .split('\n')
    .map((line) => line.trim())
    .filter(Boolean);

const addresses = (value: string) =>
  value
    .split(/[\n,;]+/)
    .map((address) => address.trim())
    .filter(Boolean);

export function inputFromForm(id: string | null, active: boolean, values: SystemFormValues): SystemInput {
  return {
    id,
    name: values.name,
    active,
    template: { name: values.templateName, title: values.templateTitle, sections: lines(values.sections) },
    distributionList: addresses(values.distributionList),
    sharepoint: { siteUrl: values.siteUrl, library: values.library },
  };
}

/** Backend field names mapped to the form field that shows their error. */
export const FORM_FIELD_FOR: Record<string, keyof SystemFormValues> = {
  name: 'name',
  templateName: 'templateName',
  templateTitle: 'templateTitle',
  templateSections: 'sections',
  distributionList: 'distributionList',
  sharepointSiteUrl: 'siteUrl',
  sharepointLibrary: 'library',
};
