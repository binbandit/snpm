#!/usr/bin/env node

const { spawnSync } = require('child_process');
const { join } = require('path');

const binPath = join(__dirname, '..', 'bin', process.platform === 'win32' ? 'snpm.exe' : 'snpm');

const result = spawnSync(binPath, ['--version'], {
  stdio: 'inherit',
});

process.exit(result.status);
