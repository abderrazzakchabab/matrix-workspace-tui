'use strict';

const { test } = require('node:test');
const assert = require('node:assert');
const { execFileSync } = require('node:child_process');
const fs = require('node:fs');
const os = require('node:os');
const path = require('node:path');
const { getPlatform } = require('../scripts/platform');

test('launcher execs the platform binary and forwards args and exit code', () => {
  const { binaryName } = getPlatform();
  const dir = fs.mkdtempSync(path.join(os.tmpdir(), 'mwt-launch-'));
  const binDir = path.join(dir, 'bin');
  fs.mkdirSync(binDir, { recursive: true });

  // Fake "binary": a shell script that echoes its args and exits 7.
  const fake = path.join(binDir, binaryName);
  fs.writeFileSync(fake, '#!/bin/sh\necho "fake-launched $1"\nexit 7\n');
  fs.chmodSync(fake, 0o755);

  fs.copyFileSync(path.join(__dirname, '..', 'index.js'), path.join(dir, 'index.js'));
  fs.cpSync(path.join(__dirname, '..', 'scripts'), path.join(dir, 'scripts'), { recursive: true });

  let output;
  let status = 0;
  try {
    output = execFileSync(process.execPath, ['index.js', 'hello'], { cwd: dir, encoding: 'utf8' });
  } catch (error) {
    output = error.stdout;
    status = error.status;
  }
  assert.match(output, /fake-launched hello/);
  assert.strictEqual(status, 7, 'the binary exit code is forwarded');
});

test('launcher prints a helpful error when the binary is missing', () => {
  const dir = fs.mkdtempSync(path.join(os.tmpdir(), 'mwt-launch-'));
  fs.copyFileSync(path.join(__dirname, '..', 'index.js'), path.join(dir, 'index.js'));
  fs.cpSync(path.join(__dirname, '..', 'scripts'), path.join(dir, 'scripts'), { recursive: true });

  let output = '';
  let status = 0;
  try {
    execFileSync(process.execPath, ['index.js'], { cwd: dir, encoding: 'utf8' });
  } catch (error) {
    output = error.stderr;
    status = error.status;
  }
  assert.match(output, /binary not found/);
  assert.notStrictEqual(status, 0);
});
