import { renderToStaticMarkup } from 'react-dom/server';
import { describe, expect, it } from 'vitest';

import type { DocumentView } from '../../api/types';
import { DocumentViewer } from './DocumentViewer';

describe('DocumentViewer', () => {
  it('renders pasted markup as literal text', () => {
    const document: DocumentView = {
      title: 'תחקיר 056-2026',
      subtitle: '<b>בלט רומני</b>',
      number: '056-2026',
      status: 'completed',
      draftNotice: null,
      blocks: [
        {
          type: 'table',
          title: 'השתלשלות אירועים',
          columns: [
            { label: 'שעה', ltr: true },
            { label: 'תוכן', ltr: false },
          ],
          rows: [['08:00', "<script>alert('x')</script>"]],
          emptyText: 'אין שורות',
        },
      ],
    };
    const html = renderToStaticMarkup(<DocumentViewer document={document} />);
    expect(html).not.toContain('<script>');
    expect(html).not.toContain('<b>בלט');
    expect(html).toContain('&lt;script&gt;alert(&#x27;x&#x27;)&lt;/script&gt;');
    expect(html).toContain('&lt;b&gt;בלט רומני&lt;/b&gt;');
  });
});
