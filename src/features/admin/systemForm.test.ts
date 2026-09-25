import { describe, expect, it } from 'vitest';

import type { System } from '../../api/types';
import { formFromSystem, inputFromForm } from './systemForm';

const system: System = {
  id: 'system-1',
  name: 'מערכת אלפא',
  active: false,
  distributionList: ['a@example.com', 'b@example.com'],
};

describe('systemForm', () => {
  it('round-trips a system through the form', () => {
    expect(inputFromForm(system.id, formFromSystem(system))).toEqual({
      id: 'system-1',
      name: 'מערכת אלפא',
      distributionList: ['a@example.com', 'b@example.com'],
    });
  });

  it('accepts addresses separated by lines, commas or semicolons', () => {
    const input = inputFromForm(null, { name: 'x', distributionList: 'a@example.com, b@example.com;\nc@example.com\n' });
    expect(input.id).toBeNull();
    expect(input.distributionList).toEqual(['a@example.com', 'b@example.com', 'c@example.com']);
  });
});
