import { invoke } from "@tauri-apps/api/core";

export interface AccountDisplay {
  id: string;
  issuer: string;
  label: string;
  algorithm: string;
  digits: number;
  period: number;
  icon: string | null;
}

export interface CodeResponse {
  id: string;
  code: string;
  remaining: number;
}

export async function getAccounts(): Promise<AccountDisplay[]> {
  return invoke<AccountDisplay[]>("get_accounts");
}

export async function addAccount(uri: string): Promise<AccountDisplay> {
  return invoke<AccountDisplay>("add_account", { uri });
}

export async function addAccountManual(
  issuer: string,
  label: string,
  secret: string,
  algorithm: string,
  digits: number,
  period: number,
): Promise<AccountDisplay> {
  return invoke<AccountDisplay>("add_account_manual", {
    issuer,
    label,
    secret,
    algorithm,
    digits,
    period,
  });
}

export async function editAccount(
  id: string,
  issuer: string,
  label: string,
): Promise<AccountDisplay> {
  return invoke<AccountDisplay>("edit_account", { id, issuer, label });
}

export async function deleteAccount(id: string): Promise<void> {
  return invoke<void>("delete_account", { id });
}

export async function reorderAccounts(ids: string[]): Promise<void> {
  return invoke<void>("reorder_accounts", { ids });
}

export async function generateAllCodes(): Promise<CodeResponse[]> {
  return invoke<CodeResponse[]>("generate_all_codes");
}

// --- PIN ---

export async function hasPin(): Promise<boolean> {
  return invoke<boolean>("has_pin");
}

export async function setPin(
  pin: string,
  currentPin: string | null = null,
): Promise<string[]> {
  return invoke<string[]>("set_pin", { pin, current_pin: currentPin });
}

export async function verifyPin(pin: string): Promise<boolean> {
  return invoke<boolean>("verify_pin", { pin });
}

export async function removePin(pin: string): Promise<void> {
  return invoke<void>("remove_pin", { pin });
}

export async function verifyRecoveryCode(code: string): Promise<boolean> {
  return invoke<boolean>("verify_recovery_code", { code });
}

export async function hasRecoveryCodes(): Promise<boolean> {
  return invoke<boolean>("has_recovery_codes");
}

// --- Backup ---

export async function exportBackup(password: string): Promise<number[]> {
  return invoke<number[]>("export_backup", { password });
}

export async function saveBackupFile(data: number[]): Promise<string> {
  return invoke<string>("save_backup_file", { data });
}

// --- Export QR ---

export interface ExportAccountInfo {
  issuer: string;
  label: string;
}

export interface ExportBatch {
  migration_uri: string;
  accounts: ExportAccountInfo[];
  batch_index: number;
  batch_count: number;
}

export async function getExportAccounts(): Promise<ExportBatch[]> {
  return invoke<ExportBatch[]>("get_export_accounts");
}

// --- External import ---

export interface ImportPreview {
  format: string;
  accounts: AccountDisplay[];
  skipped: number;
  duplicates: number;
}

export async function importExternalPreview(
  data: number[],
): Promise<ImportPreview> {
  return invoke<ImportPreview>("import_external_preview", { data });
}

export async function importExternalConfirm(
  data: number[],
): Promise<AccountDisplay[]> {
  return invoke<AccountDisplay[]>("import_external_confirm", { data });
}

export interface BackupPreview {
  accounts: AccountDisplay[];
  duplicates: number;
}

export async function importBackupPreview(
  data: number[],
  password: string,
): Promise<BackupPreview> {
  return invoke<BackupPreview>("import_backup", { data, password });
}

export async function importBackupConfirm(
  data: number[],
  password: string,
): Promise<AccountDisplay[]> {
  return invoke<AccountDisplay[]>("import_backup_confirm", { data, password });
}

// --- Sync ---

export interface SyncSessionInfo {
  session_id: string;
  qr_data: string;
  text_code: string;
  host: string | null;
  all_hosts: string[];
  port: number;
  expires_in: number;
}

export interface SyncPollResult {
  status: string;
  merge_preview: MergePreview | null;
  error: string | null;
}

export interface MergePreview {
  to_add: AccountDisplay[];
  conflicts: ConflictDisplay[];
  to_delete: AccountDisplay[];
  auto_updated: AccountDisplay[];
  unchanged: number;
}

export interface ConflictDisplay {
  account_id: string;
  local: AccountDisplay;
  remote: AccountDisplay;
}

export interface MergeDecision {
  account_id: string;
  action: string;
}

export interface SyncConfirmResult {
  added: number;
  updated: number;
  deleted: number;
}

export interface SyncPeerInfo {
  device_id: string;
  last_synced: number;
}

export async function syncStart(): Promise<SyncSessionInfo> {
  return invoke<SyncSessionInfo>("sync_start");
}

export async function syncPoll(): Promise<SyncPollResult> {
  return invoke<SyncPollResult>("sync_poll");
}

export async function syncJoin(
  code: string,
  host: string,
  port: number,
): Promise<MergePreview> {
  return invoke<MergePreview>("sync_join", { code, host, port });
}

export async function syncConfirm(
  decisions: MergeDecision[],
): Promise<SyncConfirmResult> {
  return invoke<SyncConfirmResult>("sync_confirm", { decisions });
}

export async function syncCancel(): Promise<void> {
  return invoke<void>("sync_cancel");
}

export async function syncHistory(): Promise<SyncPeerInfo[]> {
  return invoke<SyncPeerInfo[]>("sync_history");
}
