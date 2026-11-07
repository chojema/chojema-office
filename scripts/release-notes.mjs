// Prints the release notes for one version: the Korean section of CHANGELOG.md first, then the
// English section of CHANGELOG.en.md, plus a download guide. Used by the release workflow.
//
//   node scripts/release-notes.mjs 0.1.0 > notes.md

import { readFileSync } from 'node:fs';

const version = (process.argv[2] || '').replace(/^v/, '');
if (!version) {
  console.error('Usage: node scripts/release-notes.mjs <version>');
  process.exit(1);
}

function section(file) {
  const text = readFileSync(file, 'utf8').replace(/\r\n/g, '\n');
  const lines = text.split('\n');
  const start = lines.findIndex((l) => l.startsWith(`## [${version}]`));
  if (start < 0) throw new Error(`${file} has no section for ${version}`);
  let end = lines.findIndex((l, i) => i > start && /^## \[/.test(l));
  if (end < 0) end = lines.length;
  return lines
    .slice(start + 1, end)
    .filter((l) => !/^\[[^\]]+\]:\s/.test(l))
    .join('\n')
    .trim();
}

const ko = section('CHANGELOG.md');
const en = section('CHANGELOG.en.md');

const out = `## 한국어

${ko}

### 내려받기

| 시스템 | 파일 |
|---|---|
| Windows 10/11 (x64) | \`ChojemaOffice-${version}-windows-x64-setup.exe\` |
| macOS 11 이상 (Apple 실리콘·Intel) | \`ChojemaOffice-${version}-macos-universal.dmg\` (별도 빌드, 준비되는 대로 추가) |

\`SHA256SUMS.txt\`로 내려받은 파일을 확인할 수 있습니다. 설치 파일에는 아직 코드 서명이 없어서 처음 실행할 때
Windows SmartScreen이나 macOS Gatekeeper 경고가 나올 수 있습니다. 자세한 안내는 [README](https://github.com/chojema/chojema-office#readme)를 보세요.

---

## English

${en}

### Downloads

| System | File |
|---|---|
| Windows 10/11 (x64) | \`ChojemaOffice-${version}-windows-x64-setup.exe\` |
| macOS 11 or later (Apple silicon and Intel) | \`ChojemaOffice-${version}-macos-universal.dmg\` (built separately, added when ready) |

Check downloads against \`SHA256SUMS.txt\`. The installers are not code-signed yet, so Windows SmartScreen or macOS
Gatekeeper may warn on first launch. See the [English README](https://github.com/chojema/chojema-office/blob/main/README.en.md).
`;

process.stdout.write(out);
