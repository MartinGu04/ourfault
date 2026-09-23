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

export interface SharePointDestination {
  siteUrl: string;
  library: string;
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

export interface OperationsLogRow {
  /** Worksheet row number. */
  id: number;
  time: string;
  from: string;
  to: string;
  description: string;
  eventType: string;
  highlighted: boolean;
}

export interface ImportedLog {
  importId: number;
  sourceFileName: string;
  sheetName: string;
  rows: OperationsLogRow[];
  highlightDetectionAvailable: boolean;
}

export interface InvestigationDraft {
  importId: number;
  selectedRowIds: number[];
  systemId: string;
  preliminaryCheckUrl: string;
}

export interface InvestigationRow {
  time: string;
  from: string;
  to: string;
  description: string;
}

export interface Investigation {
  number: InvestigationNumber;
  date: IsoDate;
  system: { id: string; name: string };
  template: InvestigationTemplate;
  preliminaryCheckUrl: string;
  rows: InvestigationRow[];
  sourceFileName: string;
}

export interface StoredInvestigation extends Investigation {
  /** RFC 3339 local timestamp. */
  createdAt: string;
  createdBy: string;
  location: string;
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
  | { kind: 'import'; code: string; missingColumns?: string[] }
  | { kind: 'notFound' }
  | { kind: 'forbidden' }
  | { kind: 'internal' };
