// Data shapes exchanged with the Rust backend. They mirror the serde types in
// src-tauri/src (camelCase). Keep both sides in sync when changing either.

/** Canonical investigation number, e.g. "056-2026". */
export type InvestigationNumber = string;

/** ISO date "YYYY-MM-DD". */
export type IsoDate = string;

export interface CurrentUser {
  displayName: string;
  isAdmin: boolean;
}

export interface InvestigationTemplate {
  name: string;
  title: string;
  sections: string[];
}

/** The SharePoint list whose items are a system's investigations. */
export interface SharePointDestination {
  siteUrl: string;
  list: string;
}

export interface System {
  id: string;
  name: string;
  active: boolean;
  template: InvestigationTemplate;
  distributionList: string[];
  sharepoint: SharePointDestination;
}

export interface SystemInput extends Omit<System, 'id'> {
  /** null creates a new system. */
  id: string | null;
}

/** One operations-log row as pasted and reviewed by the operator. */
export interface LogRow {
  time: string;
  from: string;
  to: string;
  description: string;
}

export interface PastedRows {
  rows: LogRow[];
  /** The first pasted line was the column-title row and was skipped. */
  headerSkipped: boolean;
}

export interface InvestigationDraft {
  systemId: string;
  preliminaryCheckUrl: string;
  rows: LogRow[];
}

export interface Investigation {
  number: InvestigationNumber;
  date: IsoDate;
  system: { id: string; name: string };
  template: InvestigationTemplate;
  preliminaryCheckUrl: string;
  rows: LogRow[];
}

/** An investigation as it exists in SharePoint (the source of truth). */
export interface StoredInvestigation extends Investigation {
  /** RFC 3339 local timestamp. */
  createdAt: string;
  createdBy: string;
  /** SharePoint list item id. */
  itemId: number;
  /** Address of the editable SharePoint item. */
  url: string;
}

export interface InvestigationSummary {
  number: InvestigationNumber;
  date: IsoDate;
  systemName: string;
  rowCount: number;
}

export interface DistributionMessage {
  to: string[];
  subject: string;
  body: string;
}

export interface DistributionReceipt {
  messageId: string;
  sentAt: string;
  recipientCount: number;
}

export interface SentDistribution {
  message: DistributionMessage;
  receipt: DistributionReceipt;
}

export interface FieldError {
  field: string;
  code: string;
}

export type AppError =
  | { kind: 'validation'; errors: FieldError[] }
  | { kind: 'paste'; code: string }
  | { kind: 'notFound' }
  | { kind: 'forbidden' }
  | { kind: 'internal' };
