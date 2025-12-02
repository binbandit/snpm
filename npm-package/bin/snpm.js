#!/usr/bin/env node

const { spawnSync } = require('child_process');
const { join } = require('path');
const { existsSync } = require('fs');

const binName = process.platform === 'win32' ? 'snpm.exe' : 'snpm';
const binPath = join(__dirname, binName);

if (!existsSync(binPath)) {
  console.error('snpm binary not found. Please run: npm install');
  console.error('If the problem persists, try reinstalling: npm install -g snpm --force');
  process.exit(1);
}

const result = spawnSync(binPath, process.argv.slice(2), {
  stdio: 'inherit',
  windowsHide: false,
});

process.exit(result.status);
