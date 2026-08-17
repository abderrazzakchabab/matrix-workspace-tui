#!/usr/bin/env node
'use strict';

const { createHash } = require('node:crypto');
const fs = require('node:fs');
const path = require('node:path');
const { getPlatform } = require('./platform');

const pkg = require('../package.json');
const VERSION = process.env.MATRIX_WORKSPACE_TUI_VERSION || pkg.version;
// Overridable so tests can point at a local static server.
const BASE_URL =
  process.env.MATRIX_WORKSPACE_TUI_DOWNLOAD_BASE_URL ||
  `https://github.com/abderrazzakchabab/matrix-workspace-tui/releases/download/v${VERSION}`;
// Overridable so the timeout path is testable with a short value.
const TIMEOUT_MS = Number(process.env.MATRIX_WORKSPACE_TUI_DOWNLOAD_TIMEOUT_MS) || 30_000;

// ---------------------------------------------------------------------------
// Proxy support
//
// Node's https.get/http.get do not honor the conventional HTTPS_PROXY /
// HTTP_PROXY / NO_PROXY environment variables, so direct TLS to github.com
// stalls on corporate networks whose firewall only allows proxied egress.
// Route the request through https-proxy-agent / http-proxy-agent when a proxy
// is configured and the target is not excluded via NO_PROXY. (Node 26's
// --use-env-proxy flag is not a substitute: a published package cannot assume
// it for its install-time postinstall.)
// ---------------------------------------------------------------------------

// Normalize one NO_PROXY entry into a comparable host, tolerating optional
// ports and IPv6 brackets. Entry forms: "example.com", "example.com:8080",
// ".example.com", "[::1]", "[::1]:8080", "*".
function noProxyHost(entry) {
  let host = entry.trim().toLowerCase();
  if (host.startsWith('[')) {
    const close = host.indexOf(']');
    return close === -1 ? host : host.slice(1, close);
  }
  return host.split(':')[0];
}

// NO_PROXY semantics follow curl/npm: '*' bypasses everything; an entry
// matches the exact host or any of its subdomains.
function isNoProxyHost(hostname, noProxyList) {
  const host = hostname.toLowerCase();
  return noProxyList
    .split(',')
    .map((entry) => entry.trim())
    .filter(Boolean)
    .some((entry) => {
      if (entry === '*') return true;
      const bare = noProxyHost(entry).replace(/^\./, '');
      return host === bare || host.endsWith(`.${bare}`);
    });
}

// Decide whether `url` should go through a proxy and, if so, build the agent.
// Returns an http-proxy-agent/https-proxy-agent instance, or null for a direct
// connection. Pure function of process.env — unit-testable without a live
// proxy. HTTPS URLs prefer HTTPS_PROXY/https_proxy and fall back to
// HTTP_PROXY/http_proxy (curl behavior); HTTP URLs use HTTP_PROXY/http_proxy.
function resolveProxyFor(url) {
  const env = process.env;
  const isHttps = url.startsWith('https:');
  const proxy =
    (isHttps ? env.HTTPS_PROXY || env.https_proxy : '') ||
    env.HTTP_PROXY ||
    env.http_proxy;
  if (!proxy) return null;
  const noProxy = env.NO_PROXY || env.no_proxy;
  if (noProxy && isNoProxyHost(new URL(url).hostname, noProxy)) return null;
  // Lazy require: the agent packages are only needed when a proxy is actually
  // configured, so the script stays runnable without dependencies otherwise.
  const mod = require(isHttps ? 'https-proxy-agent' : 'http-proxy-agent');
  const Agent = isHttps ? mod.HttpsProxyAgent || mod : mod.HttpProxyAgent || mod;
  return new Agent(proxy);
}

const BIN_DIR = path.join(__dirname, '..', 'bin');
const { name, binaryName } = getPlatform();
const BINARY_PATH = path.join(BIN_DIR, binaryName);

function sha256File(filePath) {
  return new Promise((resolve, reject) => {
    const hash = createHash('sha256');
    const stream = fs.createReadStream(filePath);
    stream.on('error', reject);
    stream.on('data', (chunk) => hash.update(chunk));
    stream.on('end', () => resolve(hash.digest('hex')));
  });
}

// Node's core http client does not follow redirects, and GitHub release
// downloads always answer 302 to the objects.githubusercontent.com CDN, so a
// package that never followed them could not download its binary at all.
// Follow the redirect status codes below, bounded to MAX_REDIRECTS hops.
const REDIRECT_STATUS_CODES = new Set([301, 302, 303, 307, 308]);
const MAX_REDIRECTS = 5;

function fetchBinary(url, destination) {
  return new Promise((resolve, reject) => {
    const attempt = (currentUrl, redirectsLeft) => {
      // Re-decide the proxy on every hop: the CDN the redirect lands on may
      // differ from github.com, so NO_PROXY must be evaluated per host.
      const client = currentUrl.startsWith('https:')
        ? require('node:https')
        : require('node:http');
      const agent = resolveProxyFor(currentUrl);
      const options = agent ? { agent } : undefined;
      const request = client.get(currentUrl, options, (response) => {
        const statusCode = response.statusCode;
        if (REDIRECT_STATUS_CODES.has(statusCode)) {
          response.resume();
          const location = response.headers.location;
          if (!location) {
            reject(new Error(`Download failed: ${statusCode} for ${currentUrl}`));
            return;
          }
          if (redirectsLeft <= 0) {
            reject(new Error(`Download failed: too many redirects (max ${MAX_REDIRECTS}) for ${url}`));
            return;
          }
          let nextUrl;
          try {
            // Resolve the Location header against the current URL (RFC 7231
            // 7.1.2): GitHub sends an absolute CDN URL, but relative and
            // scheme-relative locations must resolve correctly too.
            nextUrl = new URL(location, currentUrl).toString();
          } catch (error) {
            reject(new Error(`Download failed: invalid redirect Location "${location}" from ${currentUrl}`));
            return;
          }
          attempt(nextUrl, redirectsLeft - 1);
          return;
        }
        if (statusCode !== 200) {
          response.resume();
          reject(new Error(`Download failed: ${statusCode} for ${currentUrl}`));
          return;
        }
        const file = fs.createWriteStream(destination);
        response.pipe(file);
        file.on('finish', () => file.close(() => resolve()));
        file.on('error', reject);
      });
      // Never hang forever (e.g. the release does not exist yet): destroy the
      // request on inactivity so the error path rejects cleanly. Applied on
      // every hop, so a stalled redirect target times out like any other.
      request.setTimeout(TIMEOUT_MS, () => {
        request.destroy(new Error(`Download timed out after ${TIMEOUT_MS}ms: ${currentUrl}`));
      });
      request.on('error', reject);
    };
    attempt(url, MAX_REDIRECTS);
  });
}

async function main() {
  try {
    if (fs.existsSync(BINARY_PATH)) {
      console.log(`matrix-workspace-tui: ${binaryName} already present, skipping download`);
      return;
    }
    fs.mkdirSync(BIN_DIR, { recursive: true });
    const binaryUrl = `${BASE_URL}/${binaryName}`;
    const checksumUrl = `${binaryUrl}.sha256`;
    const checksumPath = path.join(BIN_DIR, `${binaryName}.sha256`);
    const tmpPath = `${BINARY_PATH}.tmp`;

    console.log(`matrix-workspace-tui: downloading ${binaryName} v${VERSION}`);
    await fetchBinary(binaryUrl, tmpPath);
    await fetchBinary(checksumUrl, checksumPath);

    const expected = (await fs.promises.readFile(checksumPath, 'utf8'))
      .trim()
      .split(/\s+/)[0]
      .toLowerCase();
    const actual = await sha256File(tmpPath);
    if (expected !== actual) {
      throw new Error(`Checksum mismatch for ${binaryName}: expected ${expected}, got ${actual}`);
    }
    await fs.promises.rename(tmpPath, BINARY_PATH);
    await fs.promises.chmod(BINARY_PATH, 0o755);
    console.log(`matrix-workspace-tui: installed ${binaryName}`);
  } catch (error) {
    // Leave no partial downloads behind (tmp file, checksum file, empty dir).
    fs.rmSync(BIN_DIR, { recursive: true, force: true });
    console.error(`matrix-workspace-tui: ${error.message}`);
    process.exit(1);
  }
}

if (require.main === module) {
  main();
}

module.exports = { isNoProxyHost, resolveProxyFor };
