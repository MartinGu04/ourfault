// Shows a DocumentView exactly as the backend composed it: the same view the
// PDF and the SharePoint HTML are rendered from. Values are plain text only.

import type { DocumentBlock, DocumentView } from '../../api/types';
import { Badge, Ltr } from '../../ui/controls';
import { StatusBadge } from './StatusBadge';

interface Props {
  document: DocumentView;
  /** Shown next to the title of a draft (e.g. the expected number). */
  numberNote?: string | undefined;
}

export function DocumentViewer({ document, numberNote }: Props) {
  return (
    <article className="document" aria-label={document.title}>
      {document.draftNotice && (
        <p className="document-draft-notice" role="note">
          {document.draftNotice}
        </p>
      )}
      <header className="document-header">
        <div className="document-title-row">
          <h1 className="document-title">{document.title}</h1>
          <StatusBadge status={document.status} />
        </div>
        <p className="document-subtitle">{document.subtitle}</p>
        {numberNote && (
          <p className="document-number-note">
            <Badge tone="muted">{numberNote}</Badge>
          </p>
        )}
      </header>
      {document.blocks.map((block, index) => (
        <Block key={index} block={block} />
      ))}
    </article>
  );
}

function Value({ text, ltr }: { text: string; ltr: boolean }) {
  return ltr ? <Ltr>{text}</Ltr> : <>{text}</>;
}

function Block({ block }: { block: DocumentBlock }) {
  if (block.type === 'fields') {
    return (
      <section className="document-section">
        <h2>{block.title}</h2>
        <dl className="document-fields">
          {block.fields.map((field, index) => (
            <div key={index} className={field.wide ? 'document-field-wide' : undefined}>
              <dt>{field.label}</dt>
              <dd>
                <Value text={field.value} ltr={field.ltr} />
              </dd>
            </div>
          ))}
        </dl>
      </section>
    );
  }
  return (
    <section className="document-section">
      <h2>
        {block.title}
        <span className="document-section-count">{block.rows.length}</span>
      </h2>
      {block.rows.length === 0 ? (
        <p className="document-placeholder">{block.emptyText}</p>
      ) : (
        <div className="document-table-wrap">
          <table className="document-table">
            <thead>
              <tr>
                {block.columns.map((column, index) => (
                  <th key={index} scope="col">
                    {column.label}
                  </th>
                ))}
              </tr>
            </thead>
            <tbody>
              {block.rows.map((row, rowIndex) => (
                <tr key={rowIndex}>
                  {row.map((cell, cellIndex) => (
                    <td key={cellIndex} className="cell-text">
                      <Value text={cell} ltr={block.columns[cellIndex]?.ltr ?? false} />
                    </td>
                  ))}
                </tr>
              ))}
            </tbody>
          </table>
        </div>
      )}
    </section>
  );
}
