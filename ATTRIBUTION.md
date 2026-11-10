# 출처 표기 · Attribution

Chojema Office의 코드와 고유 자산은 Chulwoo Park이 만들었고 [Apache License 2.0](LICENSE)으로 배포합니다.
다른 사람이 만든 자료는 아래에 모두 밝힙니다. 자산을 추가할 때는 같은 변경에서 이 표에 한 줄을 추가합니다.

The code and original assets of Chojema Office are by Chulwoo Park and licensed under the
[Apache License 2.0](LICENSE). Everything made by someone else is listed below.

## 앱 아이콘 · App icons (`src/app-icons/`)

| 파일 · File | 앱 · App | 만든 이 · Author | 원본 · Source | 라이선스 · License |
|---|---|---|---|---|
| `wordcraft.png` | WordCraft | ArtCraft Team and the WordCraft contributors | [`storytold/wordcraft`](https://github.com/storytold/wordcraft) `assets/app-icon/hicolor/256x256/apps/ai.storyteller.wordcraft.png`, 수정 없이 복사 · copied unmodified | MIT OR Apache-2.0 |
| `gridcraft.png` | GridCraft | ArtCraft Team and the GridCraft contributors | [`storytold/gridcraft`](https://github.com/storytold/gridcraft) `assets/app-icon/hicolor/256x256/apps/ai.storyteller.gridcraft.png`, 수정 없이 복사 · copied unmodified | MIT OR Apache-2.0 |
| `deckcraft.png` | DeckCraft | ArtCraft Team and the DeckCraft contributors | [`storytold/deckcraft`](https://github.com/storytold/deckcraft) `assets/app-icon/hicolor/256x256/apps/ai.storyteller.deckcraft.png`, 수정 없이 복사 · copied unmodified | MIT OR Apache-2.0 |
| `pdfcraft.png` | PdfCraft | ArtCraft Team and the PdfCraft contributors | [`storytold/pdfcraft`](https://github.com/storytold/pdfcraft) `assets/app-icon/hicolor/256x256/apps/ai.storyteller.pdfcraft.png`, 수정 없이 복사 · copied unmodified | MIT OR Apache-2.0 |
| `pdflock.png` | Chojema PDFLock | Chulwoo Park | [`chojema/chojema-pdflock`](https://github.com/chojema/chojema-pdflock) `src-tauri/icons/128x128@2x.png`, 수정 없이 복사 · copied unmodified | MIT |
| `dooroo.png` | Chojema Dooroo | Chulwoo Park | [`chojema/ChojemaDooroo`](https://github.com/chojema/ChojemaDooroo) `apps/desktop/build/icon.png`, 256 px로 축소 · resized | Apache-2.0 |
| `editor.png` | Chojema Editor | Chulwoo Park | [`chojema/chojema-editor`](https://github.com/chojema/chojema-editor) `build/icon.png`, 256 px로 축소 · resized | Apache-2.0 |
| `imageplus.png` | Chojema Image+ | Chulwoo Park | [`chojema/ChojemaImagePlus`](https://github.com/chojema/ChojemaImagePlus) `src-tauri/icons/128x128@2x.png` | MIT |
| `keystamp.png` | Chojema KeyStamp | Chulwoo Park | [`chojema/chojema-keystamp`](https://github.com/chojema/chojema-keystamp) `src-tauri/icons/128x128@2x.png` | Chulwoo Park |

WordCraft, GridCraft, DeckCraft, PdfCraft, ArtCraft라는 이름은 각 소유자의 것이며, 해당 앱을 알아보게 하는 용도로만 씁니다.
The names WordCraft, GridCraft, DeckCraft, PdfCraft and ArtCraft belong to their owners and are used only to identify those apps.

## 외부 앱 · Partner apps

Chojema Office는 외부 앱을 함께 배포하지 않습니다. 사용자가 요청할 때 각 앱의 공식 GitHub 릴리스 페이지에서
설치 파일을 그대로 내려받아 설치합니다. 각 앱의 라이선스와 이용 조건은 해당 저장소를 따릅니다.

Chojema Office does not redistribute partner apps. On request it downloads each app's official installer, unchanged,
from that app's own GitHub release page. Each app's license and terms are those of its repository.

| 앱 · App | 만든 이 · Author | 저장소 · Repository | 라이선스 · License |
|---|---|---|---|
| WordCraft | ArtCraft Team | <https://github.com/storytold/wordcraft> | MIT OR Apache-2.0 |
| GridCraft | ArtCraft Team | <https://github.com/storytold/gridcraft> | MIT OR Apache-2.0 |
| DeckCraft | ArtCraft Team | <https://github.com/storytold/deckcraft> | MIT OR Apache-2.0 |
| PdfCraft | ArtCraft Team | <https://github.com/storytold/pdfcraft> | MIT OR Apache-2.0 |

## 착상 · Inspiration

여러 앱을 한 런처에서 설치·업데이트·실행한다는 착상은 [ArtCraft Launcher](https://github.com/storytold/craft-launcher)
(MIT OR Apache-2.0)에서 얻었습니다. 그 프로젝트의 코드, 디자인, 자산은 포함하지 않았으며 모두 새로 작성했습니다.

The idea of one launcher that installs, updates and opens a suite of apps comes from
[ArtCraft Launcher](https://github.com/storytold/craft-launcher) (MIT OR Apache-2.0). No code, design or assets from
it are included; everything here was written from scratch.

## 의존성 · Dependencies

빌드할 때 [Tauri](https://tauri.app)(MIT OR Apache-2.0)와 Rust 크레이트들을 사용합니다. 각 크레이트의 라이선스는
`src-tauri/Cargo.lock`에 적힌 버전의 crates.io 페이지에서 확인할 수 있습니다.

The app is built with [Tauri](https://tauri.app) (MIT OR Apache-2.0) and Rust crates listed in `src-tauri/Cargo.lock`,
each under its own license as published on crates.io.
