// Data shapes exchanged with the Rust backend. They mirror the serde types in
// src-tauri/src (camelCase). Keep both sides in sync when changing either.

/** Canonical investigation number, e.g. "056-2026". */
export type InvestigationNumber = string;

/** ISO date "YYYY-MM-DD". */
export type IsoDate = string;

// ---- Session ----------------------------------------------------------------

export type WorkMode = 'regular' | 'admin';

export interface Session {
  /** null until a mode is chosen on the entry screen. */
  mode: WorkMode | null;
  adminAvailable: boolean;
  operatorName: string;
}

// ---- Configuration ----------------------------------------------------------

export type FieldKind = 'text' | 'number' | 'boolean' | 'singleSelect' | 'multiSelect' | 'station' | 'system';
export type SectionMode = 'single' | 'repeating';

export interface FieldDefinition {
  id: string;
  label: string;
  kind: FieldKind;
  required: boolean;
  active: boolean;
  options?: string[];
}

export interface SectionDefinition {
  id: string;
  name: string;
  active: boolean;
  mode: SectionMode;
  fields: FieldDefinition[];
}

/** A selectable system or station. Inactive ones are listed for labels only. */
export interface Choice {
  id: string;
  name: string;
  active: boolean;
}

export interface SetupItem {
  key: string;
  done: boolean;
  required: boolean;
}

export interface SetupStatus {
  items: SetupItem[];
  ready: boolean;
}

export interface Workspace {
  systems: Choice[];
  stations: Choice[];
  /** Active sections with their active fields, in order. */
  sections: SectionDefinition[];
  setup: SetupStatus;
}

export interface System {
  id: string;
  name: string;
  active: boolean;
  distributionList: string[];
  metadata?: Record<string, string>;
}

export interface SystemInput {
  /** null creates a new system. */
  id: string | null;
  name: string;
  distributionList: string[];
}

export interface Station {
  id: string;
  name: string;
  active: boolean;
}

export interface StationInput {
  id: string | null;
  name: string;
}

export interface FieldInput {
  id: string | null;
  label: string;
  kind: FieldKind;
  required: boolean;
  active: boolean;
  options: string[];
}

export interface SectionInput {
  id: string | null;
  name: string;
  mode: SectionMode;
  fields: FieldInput[];
}

export interface MailTemplate {
  subject: string;
  body: string;
  linkText: string;
}

export interface PublicationSettings {
  siteUrl: string;
  list: string;
}

export interface Configuration {
  systems: System[];
  stations: Station[];
  sections: SectionDefinition[];
  mail: MailTemplate;
  publication: PublicationSettings;
}

export interface AdminConfiguration {
  configuration: Configuration;
  setup: SetupStatus;
}

// ---- Drafts -------------------------------------------------------------------

export type ActivityTypeKind = 'mission' | 'experiment' | 'training' | 'other';
export type ActivityStatus = 'active' | 'completed';
export type InvestigationStatus = 'draft' | 'completed' | 'distributed';

export interface ActivityInput {
  name: string;
  activityType: ActivityTypeKind | null;
  activityTypeOther: string;
  systemIds: string[];
  status: ActivityStatus | null;
  /** "YYYY-MM-DDTHH:MM" (datetime-local) or empty. */
  plannedStart: string;
  plannedEnd: string;
  actualStart: string;
  actualEnd: string;
  nightActivity: boolean | null;
  seniorStaffing: boolean | null;
}

/** text / number / choice / station id / system id: string; boolean; multi-select: string[]. */
export type FieldValue = string | boolean | string[];
export type SectionRecord = Record<string, FieldValue>;

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

export interface DraftContent {
  activity: ActivityInput;
  /** Section id → records (one for single-record sections). */
  sections: Record<string, SectionRecord[]>;
  rows: LogRow[];
  preliminaryCheckUrl: string;
}

export type DraftStep = 'activity' | 'technical' | 'chronology' | 'review';

export interface Draft {
  id: string;
  revision: number;
  createdAt: string;
  createdBy: string;
  updatedAt: string;
  step: DraftStep;
  content: DraftContent;
  converted?: { number: InvestigationNumber; at: string };
}

export interface DraftSummary {
  id: string;
  revision: number;
  activityName: string;
  systemNames: string[];
  rowCount: number;
  step: DraftStep;
  updatedAt: string;
  createdBy: string;
}

// ---- Documents & investigations -----------------------------------------------

export interface FieldLine {
  label: string;
  value: string;
  ltr: boolean;
  wide: boolean;
}

export interface ColumnView {
  label: string;
  ltr: boolean;
}

export type DocumentBlock =
  | { type: 'fields'; title: string; fields: FieldLine[] }
  | { type: 'table'; title: string; columns: ColumnView[]; rows: string[][]; emptyText: string };

/** The rendered document (same view the PDF and SharePoint HTML are made from). */
export interface DocumentView {
  title: string;
  subtitle: string;
  number: InvestigationNumber | null;
  status: InvestigationStatus;
  draftNotice: string | null;
  blocks: DocumentBlock[];
}

export interface FieldError {
  field: string;
  code: string;
}

export type Severity = 'error' | 'warning' | 'info';

/** A finding about a draft. Errors block completion; warnings and info do not. */
export interface Advisory {
  severity: Severity;
  /** Language-neutral message key. */
  code: string;
  field: string;
}

export interface Review {
  /** Errors first, then warnings, then information. */
  advisories: Advisory[];
  document: DocumentView;
  /** null when the destination cannot be reached. */
  expectedNumber: InvestigationNumber | null;
}

export interface InvestigationSummary {
  number: InvestigationNumber;
  activityName: string;
  systems: string[];
  date: IsoDate;
  activityType: ActivityTypeKind;
  status: InvestigationStatus;
}

export interface StatusEvent {
  status: InvestigationStatus;
  at: string;
  by: string;
}

export interface Lifecycle {
  status: InvestigationStatus;
  completedAt: string;
  completedBy: string;
  distributedAt: string | null;
  history: StatusEvent[];
}

export interface Publication {
  destination: 'sharePoint' | 'sharedFolder';
  url: string;
  reference: string;
}

export interface PublishedInvestigation {
  investigation: {
    number: InvestigationNumber;
    activity: { name: string; systems: { id: string; name: string }[] };
  };
  lifecycle: Lifecycle;
  publication: Publication;
}

/** Result of completing a draft (idempotent). */
export interface Completion {
  investigation: PublishedInvestigation;
  /** The draft had already been published; nothing new was created. */
  alreadyExisted: boolean;
}

export interface InvestigationDetails {
  investigation: PublishedInvestigation;
  summary: InvestigationSummary;
  document: DocumentView;
}

export interface DistributionMessage {
  to: string[];
  subject: string;
  bodyText: string;
  /** For the mail client only; never rendered by OurFault. */
  bodyHtml: string;
}

export interface DistributionReceipt {
  messageId: string;
  sentAt: string;
  recipientCount: number;
}

export interface SentDistribution {
  message: DistributionMessage;
  receipt: DistributionReceipt;
  /** null when the message was sent but the status could not be recorded. */
  investigation: PublishedInvestigation | null;
}

export interface ExportedFile {
  fileName: string;
  folder: string;
}

export type AppError =
  | { kind: 'validation'; errors: FieldError[] }
  | { kind: 'paste'; code: string }
  | { kind: 'notFound' }
  | { kind: 'conflict'; code: string }
  | { kind: 'unavailable' }
  | { kind: 'forbidden' }
  | { kind: 'internal' };
