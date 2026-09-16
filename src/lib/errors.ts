export type ErrorCode =
  | "validation"
  | "not_found"
  | "conflict"
  | "forbidden"
  | "session_expired"
  | "rate_limited"
  | "invalid_credentials"
  | "storage"
  | "printer";

export class ApiError extends Error {
  code: ErrorCode | string;

  constructor(code: ErrorCode | string, message: string) {
    super(message);
    this.name = "ApiError";
    this.code = code;
  }

  override toString() {
    return this.message;
  }
}

function asRecord(value: unknown): Record<string, unknown> | null {
  return value !== null && typeof value === "object" ? (value as Record<string, unknown>) : null;
}

function parseLegacyCode(text: string): string {
  const match = text.match(
    /^(SESSION_EXPIRED|FORBIDDEN|RATE_LIMITED|INVALID_CREDENTIALS):/,
  );
  return match ? match[1].toLowerCase() : "storage";
}

function stripLegacyPrefix(text: string): string {
  return text.replace(
    /^(SESSION_EXPIRED|FORBIDDEN|RATE_LIMITED|INVALID_CREDENTIALS):\s*/,
    "",
  );
}

export function toApiError(error: unknown): ApiError {
  if (error instanceof ApiError) return error;
  if (typeof error === "string") {
    const trimmed = error.trim();
    if (trimmed.startsWith("{")) {
      try {
        return toApiError(JSON.parse(trimmed));
      } catch {
        /* fall through */
      }
    }
    return new ApiError(parseLegacyCode(trimmed), stripLegacyPrefix(trimmed));
  }
  const record = asRecord(error);
  if (record) {
    const nested = record.error ?? record.payload;
    if (nested && typeof nested === "object") {
      const inner = toApiError(nested);
      if (inner.message) return inner;
    }
    const message =
      typeof record.message === "string"
        ? record.message
        : typeof record.msg === "string"
          ? record.msg
          : "";
    if (message.startsWith("{")) {
      try {
        return toApiError(JSON.parse(message));
      } catch {
        /* fall through */
      }
    }
    const code =
      typeof record.code === "string"
        ? record.code
        : parseLegacyCode(message || String(error));
    if (message) return new ApiError(code, stripLegacyPrefix(message));
  }
  const text = String(error);
  return new ApiError(parseLegacyCode(text), stripLegacyPrefix(text) || text);
}

export function fail(code: ErrorCode | string, message: string): never {
  throw new ApiError(code, message);
}
