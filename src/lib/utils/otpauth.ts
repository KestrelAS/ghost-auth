export interface OtpAuthParams {
  issuer: string;
  label: string;
  secret: string;
  algorithm: string;
  digits: number;
  period: number;
}

export function parseOtpAuthUri(uri: string): OtpAuthParams {
  const url = new URL(uri);

  if (url.protocol !== "otpauth:") {
    throw new Error("Invalid protocol: expected otpauth://");
  }

  if (url.host !== "totp") {
    throw new Error("Only TOTP is supported");
  }

  const path = decodeURIComponent(url.pathname.slice(1)); // remove leading /
  const params = url.searchParams;

  let issuer = params.get("issuer") || "";
  let label = path;

  // Parse "Issuer:label" format
  if (path.includes(":")) {
    const [pathIssuer, ...rest] = path.split(":");
    if (!issuer) issuer = pathIssuer;
    label = rest.join(":");
  }

  const secret = params.get("secret");
  if (!secret) {
    throw new Error("Missing secret parameter");
  }

  return {
    issuer,
    label,
    secret: secret.toUpperCase(),
    algorithm: (params.get("algorithm") || "SHA1").toUpperCase(),
    digits: parseInt(params.get("digits") || "6", 10),
    period: parseInt(params.get("period") || "30", 10),
  };
}

export function buildOtpAuthUri(params: OtpAuthParams): string {
  const label = params.issuer
    ? `${encodeURIComponent(params.issuer)}:${encodeURIComponent(params.label)}`
    : encodeURIComponent(params.label);

  const qs = new URLSearchParams({
    secret: params.secret,
    issuer: params.issuer,
    algorithm: params.algorithm,
    digits: params.digits.toString(),
    period: params.period.toString(),
  });

  return `otpauth://totp/${label}?${qs.toString()}`;
}
