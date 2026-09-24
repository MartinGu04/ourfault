// Step 4: the preliminary-checks link, what is still missing, and a preview
// of the document exactly as it will be created (marked as a draft here).

import { useEffect, useRef, useState, type Dispatch } from 'react';

import { api } from '../../api/client';
import { errorMessage, fieldMessage } from '../../api/errors';
import type { DraftContent, DraftStep, Review, Workspace } from '../../api/types';
import { issuesLabel } from '../../lib/format';
import { describeField } from '../../lib/labels';
import { Banner, Field, Ltr, Spinner } from '../../ui/controls';
import { Icon } from '../../ui/Icon';
import { DocumentViewer } from '../investigation/DocumentViewer';
import { errorFor, type WizardAction, type WizardState } from './wizardState';

interface Props {
  state: WizardState;
  content: DraftContent;
  dispatch: Dispatch<WizardAction>;
  workspace: Workspace;
  onGoTo: (step: DraftStep) => void;
  onReview: (review: Review | null) => void;
}

export function ReviewStep({ state, content, dispatch, workspace, onGoTo, onReview }: Props) {
  const [review, setReview] = useState<Review | null>(null);
  const [error, setError] = useState<string | null>(null);
  const generation = useRef(0);
  const contentJson = JSON.stringify(content);

  // Re-checks shortly after every change on this step (the URL).
  useEffect(() => {
    const current = ++generation.current;
    const timer = window.setTimeout(() => {
      api.reviewDraft(JSON.parse(contentJson) as DraftContent).then(
        (result) => {
          if (current !== generation.current) return;
          setReview(result);
          setError(null);
          onReview(result);
          dispatch({ type: 'reviewed', issues: result.issues, expectedNumber: result.expectedNumber });
        },
        (caught: unknown) => {
          if (current !== generation.current) return;
          setError(errorMessage(caught));
          onReview(null);
        },
      );
    }, 350);
    return () => window.clearTimeout(timer);
    // `dispatch` and `onReview` are stable for the purpose of this effect.
  }, [contentJson]);

  const urlError = errorFor(state, 'preliminaryCheckUrl');
  const issues = state.fieldErrors;

  return (
    <div className="review">
      <div className="form-card">
        <Field
          label="קישור לבדיקות מקדימות"
          error={urlError ? fieldMessage(urlError) : undefined}
          hint="הדביקו את הקישור לבדיקות המקדימות שבוצעו. הקישור נשמר בתחקיר ואינו נפתח מתוך OurFault."
        >
          {({ id, describedBy, invalid }) => (
            <input
              id={id}
              className="input input-ltr"
              type="url"
              dir="ltr"
              inputMode="url"
              autoComplete="off"
              spellCheck={false}
              maxLength={2048}
              placeholder="https://"
              value={state.preliminaryCheckUrl}
              aria-describedby={describedBy}
              aria-invalid={invalid || undefined}
              onChange={(event) => dispatch({ type: 'urlChanged', url: event.target.value })}
            />
          )}
        </Field>
      </div>

      {error && <Banner tone="error">{error}</Banner>}
      {!review && !error && <Spinner label="בודק את הטיוטה" />}

      {review && (
        <>
          <div className="review-summary">
            <div className={issues.length === 0 ? 'review-status review-status-ok' : 'review-status'}>
              <Icon name={issues.length === 0 ? 'check' : 'alert'} size={20} />
              <div>
                <p className="review-status-title">
                  {issues.length === 0 ? 'התחקיר מוכן ליצירה' : `חסרים או שגויים ${issuesLabel(issues.length)}`}
                </p>
                <p className="review-status-number">
                  {review.expectedNumber ? (
                    <>
                      מספר צפוי: <Ltr>{review.expectedNumber}</Ltr> · המספר הסופי יוקצה ברגע היצירה
                    </>
                  ) : (
                    'יעד הפרסום אינו זמין כרגע. המספר יוקצה ברגע היצירה.'
                  )}
                </p>
              </div>
            </div>
            {issues.length > 0 && (
              <ul className="issue-list">
                {issues.map((issue, index) => {
                  const { label, step } = describeField(issue.field, workspace.sections);
                  return (
                    <li key={index}>
                      <button type="button" className="issue" onClick={() => onGoTo(step)}>
                        <span className="issue-field">{label}</span>
                        <span className="issue-message">{fieldMessage(issue)}</span>
                        <Icon name="forward" size={16} />
                      </button>
                    </li>
                  );
                })}
              </ul>
            )}
          </div>

          <div className="preview-frame">
            <DocumentViewer document={review.document} />
          </div>
        </>
      )}
    </div>
  );
}
