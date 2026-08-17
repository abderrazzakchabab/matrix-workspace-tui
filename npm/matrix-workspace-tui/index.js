#!/usr/bin/env node
'use strict';

const { spawnSync } = require('node:child_process');
const fs = require('node:fs');
const path = require('node:path');
const { getPlatform } = require('./scripts/platform');

const { binaryName } = getPlatform();
const binary = path.join(__dirname, 'bin', binaryName);

if (!fs.existsSync(binary)) {
  console.error(`matrix-workspace-tui: binary not found at ${binary}`);
  console.error('Run `npm install` (or `npm rebuild matrix-workspace-tui`) to download it.');
  process.exit(1);
}

const result = spawnSync(binary, process.argv.slice(2), { stdio: 'inherit' });
if (result.error) {
  console.error(`matrix-workspace-tui: failed to launch: ${result.error.message}`);
  process.exit(1);
}
process.exit(result.status ?? 1);
