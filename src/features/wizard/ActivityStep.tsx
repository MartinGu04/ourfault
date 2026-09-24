// Step 1: the activity the investigation belongs to.

import type { Dispatch } from 'react';

import { fieldMessage } from '../../api/errors';
import type { ActivityInput, Workspace } from '../../api/types';
import { ACTIVITY_STATUSES, ACTIVITY_TYPES } from '../../lib/labels';
import { Chips, Field, Segmented, YES_NO } from '../../ui/controls';
import { errorFor, type WizardAction, type WizardState } from './wizardState';

interface Props {
  state: WizardState;
  dispatch: Dispatch<WizardAction>;
  workspace: Workspace;
}

export function ActivityStep({ state, dispatch, workspace }: Props) {
  const activity = state.activity;
  const change = (patch: Partial<ActivityInput>) => dispatch({ type: 'activityChanged', patch });
  const message = (field: string) => {
    const error = errorFor(state, field);
    return error ? fieldMessage(error) : undefined;
  };

  // Active systems can be chosen; inactive ones stay visible only if a
  // reopened draft already references them.
  const systemOptions = workspace.systems
    .filter((system) => system.active || activity.systemIds.includes(system.id))
    .map((system) => ({
      value: system.id,
      label: system.active ? system.name : `${system.name} (לא פעילה)`,
      disabled: !system.active,
    }));

  const dateTime = (key: 'plannedStart' | 'plannedEnd' | 'actualStart' | 'actualEnd', label: string, hint?: string) => (
    <Field label={label} error={message(key)} hint={hint}>
      {({ id, describedBy, invalid }) => (
        <input
          id={id}
          className="input input-ltr input-datetime"
          type="datetime-local"
          dir="ltr"
          value={activity[key]}
          aria-describedby={describedBy}
          aria-invalid={invalid || undefined}
          onChange={(event) => change({ [key]: event.target.value })}
        />
      )}
    </Field>
  );

  return (
    <div className="form-page form-page-wide">
      <section className="form-card" aria-labelledby="activity-general">
        <h2 id="activity-general" className="form-card-title">
          פרטי הפעילות
        </h2>
        <Field label="שם משימה / פעילות" error={message('activityName')}>
          {({ id, describedBy, invalid }) => (
            <input
              id={id}
              className="input"
              value={activity.name}
              maxLength={120}
              autoFocus={!activity.name}
              placeholder="לדוגמה: בלט רומני"
              aria-describedby={describedBy}
              aria-invalid={invalid || undefined}
              onChange={(event) => change({ name: event.target.value })}
            />
          )}
        </Field>

        <div className="field-row">
          <Field label="סוג פעילות" error={message('activityType')}>
            {({ describedBy, invalid }) => (
              <Segmented
                label="סוג פעילות"
                options={ACTIVITY_TYPES}
                value={activity.activityType}
                invalid={invalid}
                describedBy={describedBy}
                onChange={(activityType) => change({ activityType })}
              />
            )}
          </Field>
          <Field label="סטטוס פעילות" error={message('activityStatus')}>
            {({ describedBy, invalid }) => (
              <Segmented
                label="סטטוס פעילות"
                options={ACTIVITY_STATUSES}
                value={activity.status}
                invalid={invalid}
                describedBy={describedBy}
                onChange={(status) => change({ status })}
              />
            )}
          </Field>
        </div>

        {activity.activityType === 'other' && (
          <Field label="תיאור סוג הפעילות" error={message('activityTypeOther')}>
            {({ id, describedBy, invalid }) => (
              <input
                id={id}
                className="input"
                value={activity.activityTypeOther}
                maxLength={80}
                aria-describedby={describedBy}
                aria-invalid={invalid || undefined}
                onChange={(event) => change({ activityTypeOther: event.target.value })}
              />
            )}
          </Field>
        )}

        <Field
          label="מערכות מופעלות"
          error={message('systemIds')}
          hint={systemOptions.length === 0 ? 'אין מערכות פעילות. מנהל צריך להגדיר מערכת.' : 'ניתן לבחור מערכת אחת או יותר.'}
        >
          {({ describedBy, invalid }) => (
            <Chips
              label="מערכות מופעלות"
              options={systemOptions}
              selected={activity.systemIds}
              invalid={invalid}
              describedBy={describedBy}
              onChange={(systemIds) => change({ systemIds })}
            />
          )}
        </Field>

        <div className="field-row">
          <Field label="משימת לילה" error={message('nightActivity')}>
            {({ describedBy, invalid }) => (
              <Segmented
                label="משימת לילה"
                options={YES_NO}
                value={activity.nightActivity}
                invalid={invalid}
                describedBy={describedBy}
                onChange={(nightActivity) => change({ nightActivity })}
              />
            )}
          </Field>
          <Field label="איוש בכיר (נגד)" error={message('seniorStaffing')}>
            {({ describedBy, invalid }) => (
              <Segmented
                label="איוש בכיר (נגד)"
                options={YES_NO}
                value={activity.seniorStaffing}
                invalid={invalid}
                describedBy={describedBy}
                onChange={(seniorStaffing) => change({ seniorStaffing })}
              />
            )}
          </Field>
        </div>
      </section>

      <div className="time-groups">
        <section className="form-card" aria-labelledby="times-planned">
          <h2 id="times-planned" className="form-card-title">
            תכנון
          </h2>
          {dateTime('plannedStart', 'התחלה')}
          {dateTime('plannedEnd', 'סיום')}
        </section>
        <section className="form-card" aria-labelledby="times-actual">
          <h2 id="times-actual" className="form-card-title">
            ביצוע בפועל
          </h2>
          {dateTime('actualStart', 'התחלה')}
          {dateTime(
            'actualEnd',
            'סיום',
            activity.status === 'completed' ? undefined : 'ניתן להשאיר ריק כל עוד הפעילות פעילה.',
          )}
        </section>
      </div>
    </div>
  );
}
