# 변경 기록

이 프로젝트의 주요 변경 사항을 기록합니다. 형식은 [Keep a Changelog](https://keepachangelog.com/ko/1.1.0/)를,
버전 번호는 [유의적 버전(SemVer)](https://semver.org/lang/ko/)을 따릅니다. 영어판: [CHANGELOG.en.md](CHANGELOG.en.md)

## [Unreleased]

## [0.2.3] - 2026-11-10

### 추가

- PdfCraft 다음에 Chojema PDFLock 카드를 추가합니다(PDF 열기 암호·권한 암호 설정)
- Chojema PDFLock은 Windows 버전을 제공하며, macOS 버전은 예정으로 안내합니다

## [0.2.2] - 2026-11-10

### 고침

- macOS 앱 업데이트는 새 앱을 준비한 뒤 교체하며, 교체에 실패하면 기존 앱을 복구합니다. 기존 설치 위치도 유지합니다
- Windows MSI 업데이트에서 기존의 모든 사용자용 설치 범위를 유지합니다
- 설치된 앱과 버전을 확인한 뒤에만 설치·업데이트 완료를 표시합니다

## [0.2.1] - 2026-11-07

### 고침

- GitHub 요청 한도에 걸리면 아무 표시 없이 지난 정보를 보여 주던 문제: 이제 안내와 함께 다시 확인할 수 있는 시각을 보여 줍니다
- 요청 한도에 걸려도 Chojema Office 자체의 새 버전을 알아차리고 업데이트할 수 있습니다(GitHub 릴리스 페이지에서 최신 태그 확인)
- macOS에서 「업데이트하고 다시 시작」이 디스크 이미지만 열던 문제: 이제 앱을 직접 교체하고 다시 실행합니다

### 변경

- 시작할 때의 자동 확인은 최근 10분 안에 확인한 앱을 다시 묻지 않아 요청 한도를 아낍니다(「새로 고침」은 항상 새로 확인)

## [0.2.0] - 2026-11-07

### 추가

- 「로그인할 때 자동 실행」 설정(왼쪽 아래, 기본은 꺼짐; Windows 시작 프로그램·macOS 로그인 항목)
- 「정보와 링크」 창에 GitHub 저장소, Chojema App Park, 채널(블로그·유튜브·X·인스타그램·GitHub·홈페이지) 링크

### 변경

- 「정보와 링크」 창을 새로 디자인(프로젝트·채널·외부 앱과 출처)
- Chojema Editor·DeckCraft 설명 문구 다듬기

### 고침

- 영어 화면에서 업데이트가 1개일 때 "1 updates"로 나오던 문제

## [0.1.0] - 2026-11-07

첫 공개 버전입니다.

### 추가

- Chojema Dooroo, WordCraft, GridCraft, DeckCraft, PdfCraft, Chojema Editor, Chojema Image+, Chojema KeyStamp를 한 창에서 관리
- 각 앱의 최신 GitHub 릴리스 확인(ETag 캐시로 요청 한도 절약, 오프라인일 때는 마지막 정보 표시)
- 설치된 앱과 버전 자동 감지(Windows 레지스트리, macOS 응용 프로그램 폴더)
- 이 컴퓨터의 운영 체제와 CPU에 맞는 설치 파일 선택, SHA-256 체크섬 확인 후 설치·업데이트
- 업데이트가 있는 앱을 한 번에 업데이트하는 「모두 업데이트」
- 서랍(문서 작성·스프레드시트·발표·PDF·이미지 도구)과 살펴보기(설치됨·업데이트·미설치) 분류
- 앱 상세 패널: 버전, 받을 파일, 무결성 검사 여부, 설치 위치, 릴리스 노트
- `Ctrl K` 빠른 실행 팔레트로 앱 찾기·열기·설치
- 한국어(기본)와 영어 화면, 밝은 테마·어두운 테마·시스템 설정 따르기
- Chojema Office 자체 업데이트 알림과 설치

[Unreleased]: https://github.com/chojema/chojema-office/compare/v0.2.3...HEAD
[0.2.3]: https://github.com/chojema/chojema-office/compare/v0.2.2...v0.2.3
[0.2.2]: https://github.com/chojema/chojema-office/compare/v0.2.1...v0.2.2
[0.2.1]: https://github.com/chojema/chojema-office/compare/v0.2.0...v0.2.1
[0.2.0]: https://github.com/chojema/chojema-office/compare/v0.1.0...v0.2.0
[0.1.0]: https://github.com/chojema/chojema-office/releases/tag/v0.1.0
