'use strict';

const { test } = require('node:test');
const assert = require('node:assert');
const { spawn } = require('node:child_process');
const crypto = require('node:crypto');
const fs = require('node:fs');
const http = require('node:http');
const os = require('node:os');
const path = require('node:path');
const { getPlatform } = require('../scripts/platform');

function sha256hex(buffer) {
  return crypto.createHash('sha256').update(buffer).digest('hex');
}

/**
 * Static server serving /<binaryName> and /<binaryName>.sha256.
 * With neverRespond, the binary request is accepted but never answered, to
 * exercise the download client's timeout path.
 */
function startServer({ binaryBuffer, checksumLine, neverRespond = false }) {
  const { binaryName } = getPlatform();
  const hits = { binary: 0, checksum: 0 };
  const sockets = new Set();
  const server = http.createServer((request, response) => {
    if (request.url === `/${binaryName}`) {
      hits.binary += 1;
      if (neverRespond) {
        return; // hold the connection open, never answer
      }
      response.writeHead(200, { 'content-type': 'application/octet-stream' });
      response.end(binaryBuffer);
      return;
    }
    if (request.url === `/${binaryName}.sha256`) {
      hits.checksum += 1;
      response.writeHead(200, { 'content-type': 'text/plain' });
      response.end(checksumLine);
      return;
    }
    response.writeHead(404);
    response.end('not found');
  });
  server.on('connection', (socket) => {
    sockets.add(socket);
    socket.on('close', () => sockets.delete(socket));
  });
  return new Promise((resolve) => {
    server.listen(0, '127.0.0.1', () => {
      resolve({
        baseUrl: `http://127.0.0.1:${server.address().port}`,
        close: () =>
          new Promise((done) => {
            for (const socket of sockets) socket.destroy();
            server.close(done);
          }),
        hits,
      });
    });
  });
}

/**
 * Run scripts/download.js as a child. Must be async (spawn, not spawnSync):
 * the static server runs in THIS process, so a synchronous child call would
 * block the event loop and the server could never answer the download.
 */
function runDownload(env) {
  const script = path.join(__dirname, '..', 'scripts', 'download.js');
  return new Promise((resolve) => {
    // Scrub proxy vars so ambient developer/CI proxies cannot route the
    // 127.0.0.1 test server through a real proxy (these tests exercise the
    // direct-connection path; proxy decision logic is tested separately in
    // proxy.test.js).
    const childEnv = { ...process.env, ...env };
    for (const key of [
      'HTTPS_PROXY',
      'https_proxy',
      'HTTP_PROXY',
      'http_proxy',
      'NO_PROXY',
      'no_proxy',
    ]) {
      delete childEnv[key];
    }
    const child = spawn(process.execPath, [script], { env: childEnv });
    let stdout = '';
    let stderr = '';
    child.stdout.setEncoding('utf8');
    child.stderr.setEncoding('utf8');
    child.stdout.on('data', (chunk) => (stdout += chunk));
    child.stderr.on('data', (chunk) => (stderr += chunk));
    child.on('close', (status) => resolve({ status, stdout, stderr }));
  });
}

test('download installs the binary and verifies the sha256 checksum', async () => {
  const binaryBuffer = Buffer.from('#!/bin/sh\necho fake-binary\n');
  const checksumLine = `${sha256hex(binaryBuffer)}  matrix-workspace-tui-${getPlatform().name}`;
  const server = await startServer({ binaryBuffer, checksumLine });
  try {
    const result = await runDownload({
      MATRIX_WORKSPACE_TUI_DOWNLOAD_BASE_URL: server.baseUrl,
      MATRIX_WORKSPACE_TUI_VERSION: '0.1.0',
    });
    assert.strictEqual(result.status, 0, result.stderr);
    const installed = path.join(__dirname, '..', 'bin', getPlatform().binaryName);
    assert.ok(fs.existsSync(installed), 'binary installed');
    assert.deepStrictEqual(fs.readFileSync(installed), binaryBuffer);
    assert.strictEqual(fs.statSync(installed).mode & 0o111, 0o111, 'binary is executable');
    assert.strictEqual(server.hits.binary, 1);
    assert.strictEqual(server.hits.checksum, 1);
    fs.rmSync(path.join(__dirname, '..', 'bin'), { recursive: true, force: true });
  } finally {
    await server.close();
  }
});

test('download fails hard on checksum mismatch and leaves no binary', async () => {
  const binaryBuffer = Buffer.from('#!/bin/sh\necho fake-binary\n');
  const server = await startServer({
    binaryBuffer,
    checksumLine: `${'0'.repeat(64)}  matrix-workspace-tui-${getPlatform().name}`,
  });
  try {
    const result = await runDownload({
      MATRIX_WORKSPACE_TUI_DOWNLOAD_BASE_URL: server.baseUrl,
      MATRIX_WORKSPACE_TUI_VERSION: '0.1.0',
    });
    assert.notStrictEqual(result.status, 0, 'must exit non-zero');
    assert.match(result.stderr, /Checksum mismatch/);
    const binDir = path.join(__dirname, '..', 'bin');
    assert.ok(!fs.existsSync(binDir), 'no partial binary left behind');
  } finally {
    await server.close();
  }
});

test('download fails cleanly when the download times out', async () => {
  const binaryBuffer = Buffer.from('#!/bin/sh\necho fake-binary\n');
  const server = await startServer({
    binaryBuffer,
    checksumLine: `${sha256hex(binaryBuffer)}  matrix-workspace-tui-${getPlatform().name}`,
    neverRespond: true,
  });
  try {
    const result = await runDownload({
      MATRIX_WORKSPACE_TUI_DOWNLOAD_BASE_URL: server.baseUrl,
      MATRIX_WORKSPACE_TUI_VERSION: '0.1.0',
      MATRIX_WORKSPACE_TUI_DOWNLOAD_TIMEOUT_MS: '500',
    });
    assert.notStrictEqual(result.status, 0, 'must exit non-zero');
    assert.match(result.stderr, /timed out/);
    const binDir = path.join(__dirname, '..', 'bin');
    assert.ok(!fs.existsSync(binDir), 'no partial binary left behind');
  } finally {
    await server.close();
  }
});

test('download is idempotent when the binary already exists', async () => {
  const binaryBuffer = Buffer.from('#!/bin/sh\necho fake-binary\n');
  const checksumLine = `${sha256hex(binaryBuffer)}  matrix-workspace-tui-${getPlatform().name}`;
  const server = await startServer({ binaryBuffer, checksumLine });
  try {
    await runDownload({
      MATRIX_WORKSPACE_TUI_DOWNLOAD_BASE_URL: server.baseUrl,
      MATRIX_WORKSPACE_TUI_VERSION: '0.1.0',
    });
    const result = await runDownload({
      MATRIX_WORKSPACE_TUI_DOWNLOAD_BASE_URL: server.baseUrl,
      MATRIX_WORKSPACE_TUI_VERSION: '0.1.0',
    });
    assert.strictEqual(result.status, 0, result.stderr);
    assert.match(result.stdout, /already present/);
    assert.strictEqual(server.hits.binary, 1, 'second run must not re-download');
    fs.rmSync(path.join(__dirname, '..', 'bin'), { recursive: true, force: true });
  } finally {
    await server.close();
  }
});
