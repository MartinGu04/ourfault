import { useState } from 'react';

import { api } from '../../api/client';
import { errorMessage } from '../../api/errors';
import type { DistributionMessage, InvestigationNumber, SentDistribution } from '../../api/types';
import { formatTime, recipientsLabel } from '../../lib/format';
import { useResource } from '../../lib/useResource';
import { Banner, Button, Ltr, Spinner } from '../../ui/controls';
import { Dialog } from '../../ui/Dialog';
import { Icon } from '../../ui/Icon';

/**
 * Shows the message that will go to the distribution lists of every system in
 * the investigation, and sends it through the (mock) distribution adapter.
 * The real adapter sends as the signed-in Outlook user, with their signature.
 */
export function DistributionDialog({ number, onClose }: { number: InvestigationNumber; onClose: () => void }) {
  const [composed] = useResource(() => api.composeDistribution(number));
  const [sending, setSending] = useState(false);
  const [sent, setSent] = useState<SentDistribution | null>(null);
  const [error, setError] = useState<string | null>(null);

  async function send() {
    setSending(true);
    setError(null);
    try {
      setSent(await api.distributeInvestigation(number));
    } catch (caught) {
      setError(errorMessage(caught));
    } finally {
      setSending(false);
    }
  }

  const footer = sent ? (
    <Button variant="primary" onClick={onClose}>
      סגירה
    </Button>
  ) : (
    <>
      <span className="dialog-footer-note">סימולציה – לא תישלח הודעה בפועל</span>
      <Button onClick={onClose} disabled={sending}>
        ביטול
      </Button>
      <Button variant="primary" icon="mail" busy={sending} disabled={composed.status !== 'ready'} onClick={send}>
        שליחה
      </Button>
    </>
  );

  return (
    <Dialog
      title={`הפצת תחקיר ${number}`}
      subtitle="ההודעה נשלחת לרשימות התפוצה של כל המערכות בתחקיר, ללא כפילויות."
      onClose={onClose}
      footer={footer}
    >
      {error && <Banner tone="error">{error}</Banner>}
      {sent && (
        <Banner tone="success">
          ההודעה הופצה ל{recipientsLabel(sent.receipt.recipientCount)} בשעה {formatTime(sent.receipt.sentAt)} (סימולציה).
          מזהה: <Ltr>{sent.receipt.messageId}</Ltr>
        </Banner>
      )}
      {sent && !sent.investigation && (
        <Banner tone="error">ההודעה נשלחה, אך עדכון סטטוס התחקיר ל„הופץ” נכשל. פנו לתמיכה.</Banner>
      )}
      {composed.status === 'loading' && <Spinner label="מכין את ההודעה" />}
      {composed.status === 'error' && <Banner tone="error">{errorMessage(composed.error)}</Banner>}
      {composed.status === 'ready' && <EmailPreview message={sent?.message ?? composed.data} />}
    </Dialog>
  );
}

function EmailPreview({ message }: { message: DistributionMessage }) {
  return (
    <div className="email">
      <dl className="email-headers">
        <div>
          <dt>מאת</dt>
          <dd>המשתמש המחובר ב-Outlook</dd>
        </div>
        <div>
          <dt>אל</dt>
          <dd className="email-recipients">
            {message.to.map((address) => (
              <span key={address} className="recipient">
                <Icon name="mail" size={14} />
                <Ltr>{address}</Ltr>
              </span>
            ))}
          </dd>
        </div>
        <div>
          <dt>נושא</dt>
          <dd className="email-subject">{message.subject}</dd>
        </div>
      </dl>
      <div className="email-body">{message.bodyText}</div>
      <p className="email-signature">[חתימת Outlook של השולח]</p>
    </div>
  );
}
