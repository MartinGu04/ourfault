import { describe, expect, it } from 'vitest';

import type { SectionDefinition } from '../../api/types';
import { addField, formFromSection, inputFromForm, moveField, removeField, updateField } from './sectionForm';

const section: SectionDefinition = {
  id: 'section-2',
  name: 'פרטי כלים',
  active: true,
  mode: 'repeating',
  fields: [
    { id: 'f1', label: 'כותרת 1', kind: 'text', required: true, active: true },
    { id: 'f2', label: 'כותרת 2', kind: 'text', required: false, active: true },
    { id: 'f5', label: 'מקטע', kind: 'singleSelect', required: false, active: true, options: ['א', 'ב'] },
  ],
};

const labels = (form: ReturnType<typeof formFromSection>) => inputFromForm(form).fields.map((f) => f.label);

describe('section builder', () => {
  it('round-trips a section without changes', () => {
    expect(inputFromForm(formFromSection(section))).toEqual({
      id: 'section-2',
      name: 'פרטי כלים',
      mode: 'repeating',
      fields: [
        { id: 'f1', label: 'כותרת 1', kind: 'text', required: true, active: true, options: [] },
        { id: 'f2', label: 'כותרת 2', kind: 'text', required: false, active: true, options: [] },
        { id: 'f5', label: 'מקטע', kind: 'singleSelect', required: false, active: true, options: ['א', 'ב'] },
      ],
    });
  });

  it('reorders fields and ignores moves past either end', () => {
    let form = formFromSection(section);
    form = moveField(form, 2, -1);
    expect(labels(form)).toEqual(['כותרת 1', 'מקטע', 'כותרת 2']);
    expect(moveField(form, 0, -1)).toBe(form);
    expect(moveField(form, 2, 1)).toBe(form);
  });

  it('adds, renames, retypes and removes fields', () => {
    let form = addField(formFromSection(section));
    form = updateField(form, 3, { label: 'מס׳ זנב', required: true });
    form = updateField(form, 1, { kind: 'multiSelect', options: ' ד \n\nה' });
    form = removeField(form, 0);
    const input = inputFromForm(form);
    expect(input.fields.map((f) => [f.id, f.label, f.kind, f.required])).toEqual([
      ['f2', 'כותרת 2', 'multiSelect', false],
      ['f5', 'מקטע', 'singleSelect', false],
      [null, 'מס׳ זנב', 'text', true],
    ]);
    expect(input.fields[0]!.options).toEqual(['ד', 'ה']);
  });

  it('drops options when a field is no longer a select', () => {
    const form = updateField(formFromSection(section), 2, { kind: 'station' });
    expect(inputFromForm(form).fields[2]!.options).toEqual([]);
  });
});
