// Editing state of the section builder. Pure functions, so the rules the
// administrator relies on (order, add, remove, required, type) are tested
// without rendering. Validation happens in the backend.

import type { FieldKind, SectionDefinition, SectionInput, SectionMode } from '../../api/types';

export interface FieldForm {
  /** Stable key for React; equals the field id for saved fields. */
  key: string;
  /** null for fields added in this editing session. */
  id: string | null;
  label: string;
  kind: FieldKind;
  required: boolean;
  active: boolean;
  /** One option per line (select types only). */
  options: string;
}

export interface SectionForm {
  id: string | null;
  name: string;
  mode: SectionMode;
  fields: FieldForm[];
}

let newKeys = 0;
const newKey = () => `new-${++newKeys}`;

export function newField(): FieldForm {
  return { key: newKey(), id: null, label: '', kind: 'text', required: false, active: true, options: '' };
}

export function emptySectionForm(): SectionForm {
  return { id: null, name: '', mode: 'repeating', fields: [newField()] };
}

export function formFromSection(section: SectionDefinition): SectionForm {
  return {
    id: section.id,
    name: section.name,
    mode: section.mode,
    fields: section.fields.map((field) => ({
      key: field.id,
      id: field.id,
      label: field.label,
      kind: field.kind,
      required: field.required,
      active: field.active,
      options: (field.options ?? []).join('\n'),
    })),
  };
}

/** Moves the field at `index` one place up (-1) or down (+1). */
export function moveField(form: SectionForm, index: number, offset: -1 | 1): SectionForm {
  const target = index + offset;
  if (target < 0 || target >= form.fields.length) return form;
  const fields = [...form.fields];
  [fields[index], fields[target]] = [fields[target]!, fields[index]!];
  return { ...form, fields };
}

export function updateField(form: SectionForm, index: number, patch: Partial<FieldForm>): SectionForm {
  return { ...form, fields: form.fields.map((field, i) => (i === index ? { ...field, ...patch } : field)) };
}

export function addField(form: SectionForm): SectionForm {
  return { ...form, fields: [...form.fields, newField()] };
}

/**
 * Removes a field from the definition. Completed investigations keep their
 * own copy of the section, so history is not affected; to keep a field for
 * later, deactivate it instead.
 */
export function removeField(form: SectionForm, index: number): SectionForm {
  return { ...form, fields: form.fields.filter((_, i) => i !== index) };
}

export const hasOptions = (kind: FieldKind) => kind === 'singleSelect' || kind === 'multiSelect';

export function inputFromForm(form: SectionForm): SectionInput {
  return {
    id: form.id,
    name: form.name,
    mode: form.mode,
    fields: form.fields.map((field) => ({
      id: field.id,
      label: field.label,
      kind: field.kind,
      required: field.required,
      active: field.active,
      options: hasOptions(field.kind)
        ? field.options
            .split('\n')
            .map((option) => option.trim())
            .filter(Boolean)
        : [],
    })),
  };
}
