// The only module that talks to the Rust backend. Components call these typed
// functions and never use `invoke` directly.

import { invoke, type InvokeArgs } from '@tauri-apps/api/core';

import { ApiError, toAppError } from './errors';
import type {
  AdminConfiguration,
  DistributionMessage,
  Draft,
  DraftContent,
  DraftStep,
  DraftSummary,
  ExportedFile,
  InvestigationDetails,
  InvestigationNumber,
  InvestigationSummary,
  MailTemplate,
  PastedRows,
  PublicationSettings,
  PublishedInvestigation,
  Review,
  SectionDefinition,
  SectionInput,
  SentDistribution,
  Session,
  Station,
  StationInput,
  System,
  SystemInput,
  WorkMode,
  Workspace,
} from './types';

async function call<T>(command: string, args?: InvokeArgs): Promise<T> {
  try {
    return await invoke<T>(command, args);
  } catch (error) {
    throw new ApiError(toAppError(error));
  }
}

export const api = {
  getSession: () => call<Session>('get_session'),
  enterWorkMode: (mode: WorkMode) => call<Session>('enter_work_mode', { mode }),
  leaveWorkMode: () => call<Session>('leave_work_mode'),
  getWorkspace: () => call<Workspace>('get_workspace'),

  listDrafts: () => call<DraftSummary[]>('list_drafts'),
  getDraft: (id: string) => call<Draft>('get_draft', { id }),
  createDraft: (content: DraftContent, step: DraftStep) => call<Draft>('create_draft', { content, step }),
  saveDraft: (id: string, revision: number, content: DraftContent, step: DraftStep) =>
    call<Draft>('save_draft', { id, revision, content, step }),
  deleteDraft: (id: string, revision: number) => call<void>('delete_draft', { id, revision }),

  /** Parses text the operator pasted (rows copied from Excel). */
  parsePastedRows: (text: string) => call<PastedRows>('parse_pasted_rows', { text }),
  reviewDraft: (content: DraftContent) => call<Review>('review_draft', { content }),
  completeDraft: (id: string, revision: number) => call<PublishedInvestigation>('complete_draft', { id, revision }),

  listRecentInvestigations: () => call<InvestigationSummary[]>('list_recent_investigations'),
  searchInvestigations: (query: string) => call<InvestigationSummary[]>('search_investigations', { query }),
  getInvestigation: (number: InvestigationNumber) => call<InvestigationDetails>('get_investigation', { number }),

  composeDistribution: (number: InvestigationNumber) => call<DistributionMessage>('compose_distribution', { number }),
  distributeInvestigation: (number: InvestigationNumber) =>
    call<SentDistribution>('distribute_investigation', { number }),
  exportInvestigationPdf: (number: InvestigationNumber) => call<ExportedFile>('export_investigation_pdf', { number }),
  /** Requires the operator's confirmation that the file may be incomplete. */
  exportDraftPdf: (id: string, confirmIncomplete: boolean) =>
    call<ExportedFile>('export_draft_pdf', { id, confirmIncomplete }),

  adminGetConfiguration: () => call<AdminConfiguration>('admin_get_configuration'),
  adminSaveSystem: (input: SystemInput) => call<System>('admin_save_system', { input }),
  adminSetSystemActive: (id: string, active: boolean) => call<System>('admin_set_system_active', { id, active }),
  adminSaveStation: (input: StationInput) => call<Station>('admin_save_station', { input }),
  adminSetStationActive: (id: string, active: boolean) => call<Station>('admin_set_station_active', { id, active }),
  adminSaveSection: (input: SectionInput) => call<SectionDefinition>('admin_save_section', { input }),
  adminSetSectionActive: (id: string, active: boolean) =>
    call<SectionDefinition>('admin_set_section_active', { id, active }),
  adminMoveSection: (id: string, offset: -1 | 1) => call<string[]>('admin_move_section', { id, offset }),
  adminSaveMailTemplate: (input: MailTemplate) => call<MailTemplate>('admin_save_mail_template', { input }),
  adminSavePublication: (input: PublicationSettings) =>
    call<PublicationSettings>('admin_save_publication', { input }),
};
