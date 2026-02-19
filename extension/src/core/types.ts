export interface Account {
  id: string;
  issuer: string;
  label: string;
  secret: string;
  algorithm: string;
  digits: number;
  period: number;
  icon: string | null;
  last_modified: number;
}

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

export interface Tombstone {
  id: string;
  deleted_at: number;
}

export interface StoragePayload {
  version: number;
  device_id: string;
  accounts: Account[];
  tombstones: Tombstone[];
}

export interface EncryptedAccount {
  id: string;
  last_modified: number;
  nonce: number[];
  ciphertext: number[];
}

export interface SyncPayload {
  device_id: string;
  timestamp: number;
  accounts: EncryptedAccount[];
  tombstones: Tombstone[];
}

export interface MergeResult {
  to_add: Account[];
  conflicts: MergeConflict[];
  remote_deletions: Account[];
  auto_updated: Account[];
  unchanged: number;
}

export interface MergeConflict {
  local: Account;
  remote: Account;
}

export interface SyncHistory {
  peers: Record<string, number>;
}
