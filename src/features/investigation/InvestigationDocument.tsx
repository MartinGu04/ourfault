// Renders the fields of an investigation as they are created in SharePoint.
// Used for the preview before creation and for the (mock) SharePoint view
// afterwards, so both always match. All values are rendered as plain text.

import type { Investigation } from '../../api/types';
import { formatDate } from '../../lib/format';
import { Badge, Ltr } from '../../ui/controls';
import { CopyButton } from './CopyButton';

interface Props {
  investigation: Investigation;
  /** Marks the number as provisional (preview only). */
  provisionalNumber?: boolean;
}

export function InvestigationDocument({ investigation, provisionalNumber = false }: Props) {
  const { template } = investigation;
  return (
    <article className="document" aria-label={`${template.title} ${investigation.number}`}>
      <header className="document-header">
        <p className="document-kicker">{template.name}</p>
        <h1 className="document-title">{template.title}</h1>
        <p className="document-number">
          <Ltr>{investigation.number}</Ltr>
          {provisionalNumber && <Badge tone="muted">המספר יאושר בעת היצירה</Badge>}
        </p>
      </header>

      <dl className="document-meta">
        <div>
          <dt>מספר תחקיר</dt>
          <dd>
            <Ltr>{investigation.number}</Ltr>
          </dd>
        </div>
        <div>
          <dt>תאריך</dt>
          <dd>{formatDate(investigation.date)}</dd>
        </div>
        <div>
          <dt>מערכת</dt>
          <dd>{investigation.system.name}</dd>
        </div>
        <div className="document-meta-wide">
          <dt>קישור לבדיקות מקדימות</dt>
          <dd className="document-link">
            <Ltr className="url">{investigation.preliminaryCheckUrl}</Ltr>
            <CopyButton value={investigation.preliminaryCheckUrl} label="העתקת הקישור" />
          </dd>
        </div>
      </dl>

      <section className="document-section">
        <h2>
          יומן מבצעים
          <span className="document-section-count">{investigation.rows.length}</span>
        </h2>
        <table className="document-table">
          <thead>
            <tr>
              <th scope="col" className="col-time">
                שעה
              </th>
              <th scope="col" className="col-party">
                ממי
              </th>
              <th scope="col" className="col-party">
                למי
              </th>
              <th scope="col">תוכן</th>
            </tr>
          </thead>
          <tbody>
            {investigation.rows.map((row, index) => (
              <tr key={index}>
                <td className="col-time">
                  <Ltr>{row.time}</Ltr>
                </td>
                <td>{row.from}</td>
                <td>{row.to}</td>
                <td className="cell-text">{row.description}</td>
              </tr>
            ))}
          </tbody>
        </table>
      </section>

      {template.sections.map((section) => (
        <section key={section} className="document-section">
          <h2>{section}</h2>
          <p className="document-placeholder">שדה לעריכה ב-SharePoint – יושלם על ידי המתחקר.</p>
        </section>
      ))}
    </article>
  );
}
