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

// spx is shorthand for "snpm dlx"
// So we insert "dlx" into the arguments
const args = ['dlx', ...process.argv.slice(2)];

const result = spawnSync(binPath, args, {
    stdio: 'inherit',
    windowsHide: false,
});

process.exit(result.status);
