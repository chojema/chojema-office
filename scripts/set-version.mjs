// Sets the version everywhere it is written: src-tauri/Cargo.toml (the source of truth that
// Tauri reads), package.json and the lock files.
//
//   npm run version:set -- 0.2.0
//   npm run version:set -- 1.0.0-beta.1

import { readFileSync, writeFileSync } from 'node:fs';
import { execFileSync } from 'node:child_process';

const version = process.argv[2];
const SEMVER = /^(0|[1-9]\d*)\.(0|[1-9]\d*)\.(0|[1-9]\d*)(?:-((?:0|[1-9]\d*|\d*[a-zA-Z-][0-9a-zA-Z-]*)(?:\.(?:0|[1-9]\d*|\d*[a-zA-Z-][0-9a-zA-Z-]*))*))?$/;
if (!version || !SEMVER.test(version)) {
  console.error('Usage: npm run version:set -- <MAJOR.MINOR.PATCH[-pre.N]>');
  process.exit(1);
}

const cargoPath = 'src-tauri/Cargo.toml';
const cargo = readFileSync(cargoPath, 'utf8');
const updated = cargo.replace(/^(\[package\][\s\S]*?^version\s*=\s*)"[^"]*"/m, `$1"${version}"`);
if (updated === cargo && !cargo.includes(`version = "${version}"`)) {
  console.error(`Could not find the package version in ${cargoPath}`);
  process.exit(1);
}
writeFileSync(cargoPath, updated);

for (const file of ['package.json', 'package-lock.json']) {
  try {
    const json = JSON.parse(readFileSync(file, 'utf8'));
    json.version = version;
    if (json.packages && json.packages['']) json.packages[''].version = version;
    writeFileSync(file, `${JSON.stringify(json, null, 2)}\n`);
  } catch {
    /* file not present */
  }
}

try {
  execFileSync('cargo', ['update', '--workspace', '--offline'], { cwd: 'src-tauri', stdio: 'ignore' });
} catch {
  /* Cargo.lock is refreshed on the next build */
}

console.log(`Version set to ${version}. Next: add a "## [${version}]" section to CHANGELOG.md and CHANGELOG.en.md.`);
