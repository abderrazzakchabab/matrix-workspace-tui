'use strict';

const { test } = require('node:test');
const assert = require('node:assert');
const { isNoProxyHost, resolveProxyFor } = require('../scripts/download');
const { HttpsProxyAgent } = require('https-proxy-agent');
const { HttpProxyAgent } = require('http-proxy-agent');

// Proxy decision logic is a pure function of process.env, so the whole suite
// runs without a live proxy: set env vars, assert the returned agent (or
// null), restore. All proxy-related vars are scrubbed before each test and
// restored after, so ambient CI/developer proxy settings cannot leak in.
const PROXY_ENV_KEYS = [
  'HTTPS_PROXY',
  'https_proxy',
  'HTTP_PROXY',
  'http_proxy',
  'NO_PROXY',
  'no_proxy',
];

function withCleanProxyEnv(fn) {
  const saved = {};
  for (const key of PROXY_ENV_KEYS) saved[key] = process.env[key];
  for (const key of PROXY_ENV_KEYS) delete process.env[key];
  try {
    return fn();
  } finally {
    for (const key of PROXY_ENV_KEYS) {
      if (saved[key] === undefined) delete process.env[key];
      else process.env[key] = saved[key];
    }
  }
}

test('no proxy env vars -> direct connection (resolveProxyFor returns null)', () => {
  withCleanProxyEnv(() => {
    assert.strictEqual(resolveProxyFor('https://github.com/x/y'), null);
    assert.strictEqual(resolveProxyFor('http://example.com/x'), null);
  });
});

test('HTTPS_PROXY routes https URLs through an https-proxy-agent', () => {
  withCleanProxyEnv(() => {
    process.env.HTTPS_PROXY = 'http://proxy.example:8080';
    const agent = resolveProxyFor('https://github.com/x/y');
    assert.ok(agent instanceof HttpsProxyAgent, 'must build an HttpsProxyAgent');
  });
});

test('https URLs fall back to HTTP_PROXY when HTTPS_PROXY is unset', () => {
  withCleanProxyEnv(() => {
    process.env.HTTP_PROXY = 'http://proxy.example:8080';
    const agent = resolveProxyFor('https://github.com/x/y');
    assert.ok(agent instanceof HttpsProxyAgent, 'must build an HttpsProxyAgent');
  });
});

test('HTTP_PROXY routes http URLs through an http-proxy-agent', () => {
  withCleanProxyEnv(() => {
    process.env.HTTP_PROXY = 'http://proxy.example:8080';
    const agent = resolveProxyFor('http://example.com/x');
    assert.ok(agent instanceof HttpProxyAgent, 'must build an HttpProxyAgent');
  });
});

test('lowercase https_proxy/http_proxy are honored', () => {
  withCleanProxyEnv(() => {
    process.env.https_proxy = 'http://proxy.example:8080';
    assert.ok(resolveProxyFor('https://github.com/x/y') instanceof HttpsProxyAgent);
    process.env.http_proxy = 'http://proxy.example:8080';
    assert.ok(resolveProxyFor('http://example.com/x') instanceof HttpProxyAgent);
  });
});

test('HTTPS_PROXY wins over HTTP_PROXY for https URLs', () => {
  withCleanProxyEnv(() => {
    process.env.HTTPS_PROXY = 'http://https-proxy.example:8080';
    process.env.HTTP_PROXY = 'http://http-proxy.example:8080';
    const agent = resolveProxyFor('https://github.com/x/y');
    assert.ok(agent instanceof HttpsProxyAgent);
    assert.strictEqual(agent.proxy.hostname, 'https-proxy.example');
  });
});

test('NO_PROXY exact host bypasses the proxy', () => {
  withCleanProxyEnv(() => {
    process.env.HTTPS_PROXY = 'http://proxy.example:8080';
    process.env.NO_PROXY = 'github.com';
    assert.strictEqual(resolveProxyFor('https://github.com/x/y'), null);
  });
});

test('NO_PROXY subdomain entry matches the host and its subdomains', () => {
  withCleanProxyEnv(() => {
    process.env.HTTPS_PROXY = 'http://proxy.example:8080';
    process.env.NO_PROXY = 'example.com';
    assert.strictEqual(resolveProxyFor('https://example.com/x'), null);
    assert.strictEqual(resolveProxyFor('https://api.example.com/x'), null);
    assert.ok(resolveProxyFor('https://other.org/x') instanceof HttpsProxyAgent);
  });
});

test('NO_PROXY leading-dot and port-qualified entries are honored', () => {
  withCleanProxyEnv(() => {
    process.env.HTTPS_PROXY = 'http://proxy.example:8080';
    process.env.NO_PROXY = '.example.com:443';
    assert.strictEqual(resolveProxyFor('https://example.com/x'), null);
    assert.strictEqual(resolveProxyFor('https://foo.example.com/x'), null);
  });
});

test('NO_PROXY wildcard bypasses every host', () => {
  withCleanProxyEnv(() => {
    process.env.HTTPS_PROXY = 'http://proxy.example:8080';
    process.env.NO_PROXY = '*';
    assert.strictEqual(resolveProxyFor('https://github.com/x/y'), null);
  });
});

test('NO_PROXY entries are comma-separated and whitespace tolerant', () => {
  withCleanProxyEnv(() => {
    process.env.HTTPS_PROXY = 'http://proxy.example:8080';
    process.env.NO_PROXY = 'example.com, other.org';
    assert.strictEqual(resolveProxyFor('https://other.org/x'), null);
    assert.ok(resolveProxyFor('https://github.com/x/y') instanceof HttpsProxyAgent);
  });
});

test('no_proxy lowercase is honored', () => {
  withCleanProxyEnv(() => {
    process.env.HTTPS_PROXY = 'http://proxy.example:8080';
    process.env.no_proxy = 'github.com';
    assert.strictEqual(resolveProxyFor('https://github.com/x/y'), null);
  });
});

test('isNoProxyHost matches exact host, subdomains, and wildcards', () => {
  assert.strictEqual(isNoProxyHost('github.com', 'github.com'), true);
  assert.strictEqual(isNoProxyHost('api.github.com', 'github.com'), true);
  assert.strictEqual(isNoProxyHost('github.com', '.github.com'), true);
  assert.strictEqual(isNoProxyHost('other.org', 'github.com'), false);
  assert.strictEqual(isNoProxyHost('anything.io', '*'), true);
  assert.strictEqual(isNoProxyHost('host.example', 'example.com:8080'), false);
  assert.strictEqual(isNoProxyHost('127.0.0.1', '127.0.0.1'), true);
});
