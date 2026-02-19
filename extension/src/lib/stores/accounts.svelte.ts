import { ExtensionStorage } from "$core/storage";
import { generateAllCodes } from "$core/totp";
import { parseOtpAuthUri } from "$lib/utils/otpauth";
import type { Account, AccountDisplay, CodeResponse } from "$core/types";

export type { Account, AccountDisplay, CodeResponse };

export const storage = new ExtensionStorage();

let accounts: Account[] = $state([]);
let codes: Map<string, CodeResponse> = $state(new Map());

function toDisplay(a: Account): AccountDisplay {
  return {
    id: a.id,
    issuer: a.issuer,
    label: a.label,
    algorithm: a.algorithm,
    digits: a.digits,
    period: a.period,
    icon: a.icon,
  };
}

export function getAccounts(): AccountDisplay[] {
  return accounts.map(toDisplay);
}

export function getCodes(): Map<string, CodeResponse> {
  return codes;
}

export async function loadAccounts(): Promise<void> {
  accounts = await storage.getAccounts();
  refreshCodes();
}

export function refreshCodes(): void {
  if (accounts.length === 0) {
    codes = new Map();
    return;
  }
  const results = generateAllCodes(accounts);
  const map = new Map<string, CodeResponse>();
  for (const c of results) map.set(c.id, c);
  codes = map;
}

export async function addAccountFromUri(uri: string): Promise<AccountDisplay> {
  const params = parseOtpAuthUri(uri);
  const account: Account = {
    id: crypto.randomUUID(),
    issuer: params.issuer,
    label: params.label,
    secret: params.secret,
    algorithm: params.algorithm,
    digits: params.digits,
    period: params.period,
    icon: null,
    last_modified: Math.floor(Date.now() / 1000),
  };
  await storage.addAccount(account);
  accounts = [...accounts, account];
  refreshCodes();
  return toDisplay(account);
}

export async function addAccountManual(
  issuer: string,
  label: string,
  secret: string,
  algorithm: string,
  digits: number,
  period: number,
): Promise<AccountDisplay> {
  // Validate secret: must be valid base32
  const cleaned = secret.replace(/\s+/g, "").toUpperCase();
  if (!/^[A-Z2-7]+=*$/.test(cleaned)) {
    throw new Error("Invalid secret key — must be base32 encoded");
  }

  const account: Account = {
    id: crypto.randomUUID(),
    issuer,
    label,
    secret: cleaned,
    algorithm,
    digits,
    period,
    icon: null,
    last_modified: Math.floor(Date.now() / 1000),
  };
  await storage.addAccount(account);
  accounts = [...accounts, account];
  refreshCodes();
  return toDisplay(account);
}

export async function editAccount(id: string, issuer: string, label: string): Promise<void> {
  await storage.updateAccount(id, { issuer, label });
  const account = accounts.find((a) => a.id === id);
  if (account) {
    account.issuer = issuer;
    account.label = label;
    account.last_modified = Math.floor(Date.now() / 1000);
    accounts = [...accounts];
  }
}

export async function deleteAccount(id: string): Promise<void> {
  await storage.deleteAccount(id);
  accounts = accounts.filter((a) => a.id !== id);
  refreshCodes();
}

export async function reorderAccounts(ids: string[]): Promise<void> {
  await storage.reorderAccounts(ids);
  const accountMap = new Map(accounts.map((a) => [a.id, a]));
  const reordered: Account[] = [];
  for (const id of ids) {
    const account = accountMap.get(id);
    if (account) reordered.push(account);
  }
  accounts = reordered;
}
