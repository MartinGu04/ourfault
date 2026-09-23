import { describe, expect, it } from 'vitest';

import type { System } from '../../api/types';
import { formFromSystem, inputFromForm } from './systemForm';

const system: System = {
  id: 'system-1',
  name: 'מערכת אלפא',
  active: false,
  template: { name: 'תבנית', title: 'תחקיר', sections: ['רקע', 'ממצאים'] },
  distributionList: ['a@example.com', 'b@example.com'],
  sharepoint: { siteUrl: 'https://sharepoint.example.com/sites/alpha', library: 'Investigations' },
};

describe('systemForm', () => {
  it('round-trips a system through the form', () => {
    expect(inputFromForm(system.id, system.active, formFromSystem(system))).toEqual(system);
  });

  it('splits multi-line fields and ignores blank lines', () => {
    const values = {
      ...formFromSystem(system),
      sections: ' רקע \n\n לקחים ',
      distributionList: 'a@example.com, b@example.com;\nc@example.com\n',
    };
    const input = inputFromForm(null, true, values);
    expect(input.id).toBeNull();
    expect(input.template.sections).toEqual(['רקע', 'לקחים']);
    expect(input.distributionList).toEqual(['a@example.com', 'b@example.com', 'c@example.com']);
  });
});
