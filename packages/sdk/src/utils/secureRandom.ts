import { randomBytes } from 'node:crypto';

/**
 * Returns cryptographically secure random bytes in Node and browser runtimes.
 */
export function secureRandomBytes(length: number): Buffer {
  if (typeof globalThis.crypto?.getRandomValues === 'function') {
    const bytes = new Uint8Array(length);
    globalThis.crypto.getRandomValues(bytes);
    return Buffer.from(bytes);
  }

  return randomBytes(length);
}
