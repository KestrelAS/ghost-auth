import { TOTP } from "otpauth";
import type { Account, CodeResponse } from "./types";

export function generateCode(account: Account): CodeResponse {
  const totp = new TOTP({
    issuer: account.issuer,
    label: account.label,
    algorithm: account.algorithm,
    digits: account.digits,
    period: account.period,
    secret: account.secret,
  });

  const now = Math.floor(Date.now() / 1000);
  const code = totp.generate({ timestamp: now * 1000 });
  const remaining = account.period - (now % account.period);

  return { id: account.id, code, remaining };
}

export function generateAllCodes(accounts: Account[]): CodeResponse[] {
  return accounts.map(generateCode);
}
