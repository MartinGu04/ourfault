// Step 4: the preliminary-checks link, what is still missing, and a preview
// of the document exactly as it will be created (marked as a draft here).

import { useEffect, useRef, useState, type Dispatch, type ReactNode } from 'react';

import { api } from '../../api/client';
import { advisoryMessage, errorMessage, fieldMessage, MISSING_CODES } from '../../api/errors';
import type { Advisory, DraftContent, DraftStep, FieldError, Review, Severity, Workspace } from '../../api/types';
import { issuesLabel } from '../../lib/format';
import { describeField } from '../../lib/labels';
import { Banner, Button, Field, Ltr, Spinner } from '../../ui/controls';
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
          const issues = result.advisories
            .filter((advisory) => advisory.severity === 'error')
            .map(({ field, code }) => ({ field, code }));
          dispatch({ type: 'reviewed', issues, expectedNumber: result.expectedNumber });
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
  const { missing, invalid } = splitIssues(state.fieldErrors);
  const ready = state.fieldErrors.length === 0;
  const warnings = review?.advisories.filter((advisory) => advisory.severity === 'warning') ?? [];
  const notes = review?.advisories.filter((advisory) => advisory.severity === 'info') ?? [];
  const markNight = () => dispatch({ type: 'activityChanged', patch: { nightActivity: true } });

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
            <div className={ready ? 'review-status review-status-ok' : 'review-status'}>
              <Icon name={ready ? 'check' : 'alert'} size={20} />
              <div>
                <p className="review-status-title">{statusTitle(missing.length, invalid.length)}</p>
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
            <IssueGroup title="ערכי חובה חסרים" tone="error" issues={missing} workspace={workspace} onGoTo={onGoTo} />
            <IssueGroup title="ערכים לא תקינים" tone="error" issues={invalid} workspace={workspace} onGoTo={onGoTo} />
            <IssueGroup
              title="אזהרות (אינן חוסמות יצירה)"
              tone="warning"
              issues={warnings}
              workspace={workspace}
              onGoTo={onGoTo}
              action={(issue) =>
                issue.code === 'night_overlap_not_marked' ? (
                  <Button size="normal" onClick={markNight}>
                    סמן כמשימת לילה
                  </Button>
                ) : null
              }
            />
            <IssueGroup title="לידיעה" tone="info" issues={notes} workspace={workspace} onGoTo={onGoTo} />
          </div>

          <div className="preview-frame">
            <DocumentViewer document={review.document} />
          </div>
        </>
      )}
    </div>
  );
}

/**
 * Separates missing required values from values that are present but
 * invalid (bad format, an end before its start, an inactive reference...).
 */
export function splitIssues(issues: readonly FieldError[]): { missing: FieldError[]; invalid: FieldError[] } {
  return {
    missing: issues.filter((issue) => MISSING_CODES.includes(issue.code)),
    invalid: issues.filter((issue) => !MISSING_CODES.includes(issue.code)),
  };
}

function statusTitle(missing: number, invalid: number): string {
  if (missing === 0 && invalid === 0) return 'התחקיר מוכן ליצירה';
  const parts = [];
  if (missing > 0) parts.push(`חסרים ${issuesLabel(missing)}`);
  if (invalid > 0) parts.push(`${issuesLabel(invalid)} לא תקינים`);
  return `לא ניתן ליצור עדיין: ${parts.join(' · ')}`;
}

type Finding = FieldError | Advisory;

interface GroupProps {
  title: string;
  tone: Severity;
  issues: Finding[];
  workspace: Workspace;
  onGoTo: (step: DraftStep) => void;
  /** An optional quick fix shown next to a finding. */
  action?: (issue: Finding) => ReactNode;
}

function IssueGroup({ title, tone, issues, workspace, onGoTo, action }: GroupProps) {
  if (issues.length === 0) return null;
  return (
    <section className={`issue-group issue-group-${tone}`} aria-label={title}>
      <h3 className="issue-group-title">
        <Icon name={tone === 'info' ? 'info' : 'alert'} size={15} />
        {title} <span className="issue-group-count">{issues.length}</span>
      </h3>
      <ul className="issue-list">
        {issues.map((issue, index) => {
          const { label, step } = describeField(issue.field, workspace.sections);
          const fix = action?.(issue);
          return (
            <li key={index} className="issue-row">
              <button type="button" className="issue" onClick={() => onGoTo(step)}>
                <span className="issue-field">{label}</span>
                <span className="issue-message">{advisoryMessage({ severity: tone, ...issue })}</span>
                <Icon name="forward" size={16} />
              </button>
              {fix && <span className="issue-action">{fix}</span>}
            </li>
          );
        })}
      </ul>
    </section>
  );
}
