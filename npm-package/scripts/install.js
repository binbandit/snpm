#!/usr/bin/env node

const { existsSync, mkdirSync, chmodSync } = require('fs');
const { join } = require('path');
const { get } = require('https');
const { createWriteStream } = require('fs');
const { pipeline } = require('stream');
const { promisify } = require('util');
const { createGunzip } = require('zlib');
const tar = require('tar');

const streamPipeline = promisify(pipeline);

const BINARY_NAME = 'snpm';
const REPO_OWNER = 'binbandit';
const REPO_NAME = 'snpm';

function getPlatform() {
  const platform = process.platform;
  const arch = process.arch;

  const platformMap = {
    darwin: {
      x64: 'snpm-macos-amd64',
      arm64: 'snpm-macos-arm64',
    },
    linux: {
      x64: 'snpm-linux-amd64',
      arm64: 'snpm-linux-arm64',
    },
    win32: {
      x64: 'snpm-windows-amd64',
      arm64: 'snpm-windows-arm64',
    },
  };

  if (!platformMap[platform]) {
    throw new Error(`Unsupported platform: ${platform}`);
  }

  if (!platformMap[platform][arch]) {
    throw new Error(`Unsupported architecture: ${arch} on ${platform}`);
  }

  return platformMap[platform][arch];
}

function getDownloadUrl(version) {
  const platform = getPlatform();
  const isWindows = process.platform === 'win32';
  const ext = isWindows ? 'zip' : 'tar.gz';
  
  // Use the version from package.json or fetch latest
  const tag = version || 'latest';
  
  if (tag === 'latest') {
    return `https://github.com/${REPO_OWNER}/${REPO_NAME}/releases/latest/download/${platform}.${ext}`;
  }
  
  return `https://github.com/${REPO_OWNER}/${REPO_NAME}/releases/download/${tag}/${platform}.${ext}`;
}

async function download(url, dest) {
  return new Promise((resolve, reject) => {
    get(url, (response) => {
      if (response.statusCode === 302 || response.statusCode === 301) {
        // Follow redirect
        download(response.headers.location, dest).then(resolve).catch(reject);
        return;
      }

      if (response.statusCode !== 200) {
        reject(new Error(`Failed to download: ${response.statusCode} ${response.statusMessage}`));
        return;
      }

      const file = createWriteStream(dest);
      response.pipe(file);
      file.on('finish', () => {
        file.close(resolve);
      });
      file.on('error', (err) => {
        reject(err);
      });
    }).on('error', reject);
  });
}

async function extractTarGz(archivePath, destDir) {
  await tar.extract({
    file: archivePath,
    cwd: destDir,
  });
}

async function extractZip(archivePath, destDir) {
  const AdmZip = require('adm-zip');
  const zip = new AdmZip(archivePath);
  zip.extractAllTo(destDir, true);
}

async function install() {
  try {
    const binDir = join(__dirname, '..', 'bin');
    const version = process.env.npm_package_version;
    
    console.log(`Installing snpm ${version || 'latest'}...`);
    
    // Create bin directory if it doesn't exist
    if (!existsSync(binDir)) {
      mkdirSync(binDir, { recursive: true });
    }

    const platform = getPlatform();
    const isWindows = process.platform === 'win32';
    const ext = isWindows ? 'zip' : 'tar.gz';
    const archivePath = join(binDir, `${platform}.${ext}`);
    const url = getDownloadUrl(version);

    console.log(`Downloading from: ${url}`);
    await download(url, archivePath);
    console.log('Download complete, extracting...');

    if (isWindows) {
      await extractZip(archivePath, binDir);
    } else {
      await extractTarGz(archivePath, binDir);
    }

    // Make binary executable on Unix-like systems
    if (!isWindows) {
      const binaryPath = join(binDir, BINARY_NAME);
      if (existsSync(binaryPath)) {
        chmodSync(binaryPath, 0o755);
      }
    }

    // Clean up archive
    const fs = require('fs');
    fs.unlinkSync(archivePath);

    console.log('✓ snpm installed successfully!');
  } catch (error) {
    console.error('Failed to install snpm:', error.message);
    console.error('You can manually download snpm from:');
    console.error(`https://github.com/${REPO_OWNER}/${REPO_NAME}/releases`);
    process.exit(1);
  }
}

install();
