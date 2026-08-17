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

function fetchBinary(url, destination) {
  return new Promise((resolve, reject) => {
    const client = url.startsWith('https:') ? require('node:https') : require('node:http');
    const request = client.get(url, (response) => {
      if (response.statusCode !== 200) {
        response.resume();
        reject(new Error(`Download failed: ${response.statusCode} for ${url}`));
        return;
      }
      const file = fs.createWriteStream(destination);
      response.pipe(file);
      file.on('finish', () => file.close(() => resolve()));
      file.on('error', reject);
    });
    // Never hang forever (e.g. the release does not exist yet): destroy the
    // request on inactivity so the error path rejects cleanly.
    request.setTimeout(TIMEOUT_MS, () => {
      request.destroy(new Error(`Download timed out after ${TIMEOUT_MS}ms: ${url}`));
    });
    request.on('error', reject);
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

main();
