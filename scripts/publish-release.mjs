// Publishes a GitHub release from the maintainer's own account, using the installers that the
// Release workflow built for the tag.
//
//   npm run release:publish -- 0.2.0
//
// Needs the GitHub CLI (`gh`), signed in as the repository owner.

import { execFileSync } from 'node:child_process';
import { mkdtempSync, readdirSync, rmSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join } from 'node:path';

const version = (process.argv[2] || '').replace(/^v/, '');
if (!version) {
  console.error('Usage: npm run release:publish -- <version>');
  process.exit(1);
}
const tag = `v${version}`;
const gh = (args, opts = {}) => execFileSync('gh', args, { encoding: 'utf8', ...opts });

const runs = JSON.parse(
  gh(['run', 'list', '--workflow', 'release.yml', '--branch', tag, '--json', 'databaseId,status,conclusion', '--limit', '5']),
);
const run = runs.find((r) => r.conclusion === 'success');
if (!run) {
  console.error(`No successful Release workflow run for ${tag} yet. Runs: ${JSON.stringify(runs)}`);
  process.exit(1);
}

const dir = mkdtempSync(join(tmpdir(), `chojema-office-${version}-`));
try {
  gh(['run', 'download', String(run.databaseId), '--name', `release-${tag}`, '--dir', dir], { stdio: 'inherit' });
  const files = readdirSync(dir).filter((f) => f !== 'RELEASE_NOTES.md').map((f) => join(dir, f));
  const args = ['release', 'create', tag, ...files, '--title', `Chojema Office ${version}`, '--notes-file', join(dir, 'RELEASE_NOTES.md'), '--verify-tag'];
  if (version.includes('-')) args.push('--prerelease');
  gh(args, { stdio: 'inherit' });
} finally {
  rmSync(dir, { recursive: true, force: true });
}
