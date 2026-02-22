#!/usr/bin/env node

const { spawnSync } = require('child_process');
const { join } = require('path');
const { existsSync } = require('fs');

const exeSuffix = process.platform === 'win32' ? '.exe' : '';
const switchPath = join(__dirname, `snpm-switch${exeSuffix}`);
const fallbackPath = join(__dirname, `snpm${exeSuffix}`);
const binPath = existsSync(switchPath) ? switchPath : fallbackPath;

if (!existsSync(binPath)) {
  console.error('snpm binaries not found. Please run: npm install');
  console.error('If the problem persists, try reinstalling: npm install -g snpm --force');
  process.exit(1);
}

const result = spawnSync(binPath, process.argv.slice(2), {
  stdio: 'inherit',
  windowsHide: false,
});

process.exit(result.status);
