// The only module that talks to the Rust backend. Components call these typed
// functions and never use `invoke` directly.

import { invoke, type InvokeArgs } from '@tauri-apps/api/core';

import { ApiError, toAppError } from './errors';
import type {
  CurrentUser,
  DistributionMessage,
  Investigation,
  InvestigationDraft,
  InvestigationNumber,
  InvestigationSummary,
  PastedRows,
  SentDistribution,
  StoredInvestigation,
  System,
  SystemInput,
} from './types';

async function call<T>(command: string, args?: InvokeArgs): Promise<T> {
  try {
    return await invoke<T>(command, args);
  } catch (error) {
    throw new ApiError(toAppError(error));
  }
}

export const api = {
  getSession: () => call<CurrentUser>('get_session'),
  listRecentInvestigations: () => call<InvestigationSummary[]>('list_recent_investigations'),
  findInvestigation: (query: string) => call<StoredInvestigation>('find_investigation', { query }),

  /** Parses text the operator pasted (rows copied from Excel). */
  parsePastedRows: (text: string) => call<PastedRows>('parse_pasted_rows', { text }),

  listActiveSystems: () => call<System[]>('list_active_systems'),
  peekNextInvestigationNumber: () => call<InvestigationNumber>('peek_next_investigation_number'),
  previewInvestigation: (draft: InvestigationDraft) => call<Investigation>('preview_investigation', { draft }),
  createInvestigation: (draft: InvestigationDraft) => call<StoredInvestigation>('create_investigation', { draft }),

  composeDistribution: (number: InvestigationNumber) => call<DistributionMessage>('compose_distribution', { number }),
  distributeInvestigation: (number: InvestigationNumber) =>
    call<SentDistribution>('distribute_investigation', { number }),

  adminListSystems: () => call<System[]>('admin_list_systems'),
  adminSaveSystem: (input: SystemInput) => call<System>('admin_save_system', { input }),
  adminSetSystemActive: (id: string, active: boolean) => call<System>('admin_set_system_active', { id, active }),
};
