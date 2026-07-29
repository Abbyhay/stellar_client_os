const LOOPBACK_HOSTNAMES = new Set(['localhost', '127.0.0.1', '[::1]', '::1']);

function isLoopbackHttpUrl(url: string): boolean {
  try {
    const parsed = new URL(url);
    return parsed.protocol === 'http:' && LOOPBACK_HOSTNAMES.has(parsed.hostname);
  } catch {
    return false;
  }
}

/**
 * Stellar SDK server options for the web app.
 * Plain HTTP is only enabled in non-production builds against loopback hosts.
 */
export function getStellarServerOptions(url: string): { allowHttp: boolean } {
  const isDev = process.env.NODE_ENV !== 'production';
  return { allowHttp: isDev && isLoopbackHttpUrl(url) };
}
