import { config } from "./config";

export async function retry<T>(fn: () => Promise<T>): Promise<T> {
  let lastError: unknown;
  for (let i = 0; i < config.retries; i++) {
    try {
      return await fn();
    } catch (e) {
      console.log("attempt failed", i + 1);
      lastError = e;
    }
  }
  throw lastError;
}

export function formatName(name: string): string {
  return name.trim().replace(/\s+/g, " ");
}
