# План релизной упаковки для Linux

Ниже - подробный план реализации под `Ubuntu + Fedora` с приоритетом на `.deb + .rpm + AppImage`, чтобы проект стабильно собирал установочные артефакты и удобно раздавался через GitHub Releases. Включает полноценный CI/CD pipeline с тестами перед релизом, механизм автообновлений через `tauri-plugin-updater`, систему уведомлений пользователей о новых версиях и UI-компонент обновления в окне настроек.

База уже есть:

- `src-tauri/tauri.conf.json` уже включает bundling.
- `src-tauri/tauri.conf.json` уже использует `targets: "all"`.
- `npm run tauri build` уже умеет собирать Linux bundles.
- `README.md` уже содержит раздел установки, но пока он больше как заготовка.
- В Rust-коде уже есть 39 unit/integration тестов (`config`, `events`, `notifications`, `scheduler`, `storage`, `activity`).
- Система уведомлений через `notify-rust` / D-Bus уже реализована.
- Окно настроек (`settings`) с многостраничной структурой уже работает.

## Цель

- собирать `AppImage`, `.deb`, `.rpm` на CI;
- публиковать их в GitHub Releases по тегу;
- иметь воспроизводимую локальную сборку;
- проверить, что пакет реально устанавливается на Ubuntu и Fedora;
- CI/CD pipeline должен прогонять тесты перед каждым релизом;
- пользователи приложения получают уведомление о доступности новой версии;
- в настройках приложения появляется плашка с предложением обновиться;
- при нажатии на "Обновить" приложение обновляется автоматически (для AppImage) или открывает страницу загрузки (для `.deb`/`.rpm`).

## Что должно получиться в итоге

- локально можно запустить `npm run tauri build -- --bundles appimage,deb,rpm`;
- CI по тегу `v0.1.0` собирает все 3 артефакта;
- CI на каждый push и PR прогоняет тесты (`cargo test`, `npm run lint`, `npm run typecheck`);
- release workflow не создает артефакты, если тесты не прошли;
- GitHub Release получает файлы:
  - `*.AppImage`
  - `*.AppImage.sig`
  - `*.deb`
  - `*.rpm`
  - `latest.json` (манифест для updater)
- пользователь открывает Releases и скачивает нужный формат:
  - Ubuntu/Debian -> `.deb`
  - Fedora -> `.rpm`
  - универсальный fallback -> `AppImage`
- пользователи AppImage получают автоматическое обновление in-place;
- пользователи `.deb`/`.rpm` получают системное уведомление + плашку в настройках с ссылкой на скачивание.

---

## Фаза 1. Довести packaging metadata до релизного уровня

Нужно дополнить конфиг так, чтобы пакеты выглядели как нормальный продукт, а не просто бинарник.

Что добавить или проверить в `src-tauri/tauri.conf.json`:

- `bundle.category` - например `Utility`;
- `bundle.shortDescription` - короткое описание;
- `bundle.longDescription` - расширенное описание для package metadata;
- `bundle.license` или `bundle.licenseFile`;
- `bundle.publisher`;
- `bundle.homepage` и `bundle.repository`, если поддерживается схемой вашей версии Tauri;
- Linux-специфичные секции для `deb` и `rpm`, если нужны кастомные зависимости или maintainer metadata.

Что проверить по артефактам:

- иконка уже есть и выглядит достаточной;
- имя пакета должно быть консистентным: `blinkly`;
- description должен совпадать с позиционированием из `README.md`.

Результат фазы:

- `.deb` и `.rpm` содержат нормальное имя, описание, лицензию, category, desktop entry и иконки.

## Фаза 2. Зафиксировать локальную release-сборку

Нужно сделать локальную сборку предсказуемой до CI.

Целевой сценарий:

- `npm ci`
- `npm run tauri build -- --bundles appimage,deb,rpm`

Ожидаемые выходы:

- `src-tauri/target/release/bundle/appimage/`
- `src-tauri/target/release/bundle/deb/`
- `src-tauri/target/release/bundle/rpm/`

Что проверить локально:

- что все 3 формата действительно появляются;
- что имя файлов содержит версию `0.1.0`;
- что AppImage запускается;
- что `.deb` устанавливается на Ubuntu;
- что `.rpm` устанавливается на Fedora.

Почему это важно:

- CI не должен быть местом, где впервые выясняется, что packaging сломан.

## Фаза 3. Определить build environment для CI

Для лучшей совместимости Linux-пакеты лучше собирать на `ubuntu-22.04`.

Рекомендуемый runner:

- `ubuntu-22.04`

Почему:

- AppImage и Linux bundles обычно более переносимы, если собраны на чуть более старом baseline;
- Fedora-пользователи все равно будут ставить `.rpm`, но сам билд удобно делать на Ubuntu runner.

Системные зависимости CI:

- `libgtk-3-dev`
- `libwebkit2gtk-4.1-dev` или для `22.04` возможно `libwebkit2gtk-4.0-dev` - это надо проверить по runner;
- `libayatana-appindicator3-dev` или `libappindicator3-dev`
- `librsvg2-dev`
- `patchelf`
- `rpm`

Отдельно стоит проверить, какая именно WebKitGTK dev-библиотека нужна на выбранном runner, потому что это самый частый источник падений CI.

## Фаза 4. Настроить CI pipeline с тестами (ci.yml)

Перед тем как настраивать release workflow, нужно поднять CI pipeline, который будет гарантировать качество на каждый push и PR.

### Зачем отдельный CI workflow

Release workflow (по тегу) должен заниматься сборкой и публикацией. Тесты должны проходить **до** создания тега, чтобы:

- не тратить время CI на сборку артефактов из сломанного кода;
- выявлять регрессии на этапе PR, а не при попытке выпустить релиз;
- разработчик знал о проблеме ещё до мержа в main.

### Триггеры

```yaml
on:
  push:
    branches: [main]
  pull_request:
    branches: [main]
```

### Структура workflow `.github/workflows/ci.yml`

```yaml
name: CI

on:
  push:
    branches: [main]
  pull_request:
    branches: [main]

env:
  CARGO_TERM_COLOR: always

jobs:
  test:
    name: Test & Lint
    runs-on: ubuntu-22.04
    steps:
      - uses: actions/checkout@v4

      - name: Install system dependencies
        run: |
          sudo apt-get update
          sudo apt-get install -y \
            libgtk-3-dev \
            libwebkit2gtk-4.1-dev \
            libayatana-appindicator3-dev \
            librsvg2-dev \
            patchelf

      - name: Setup Node.js
        uses: actions/setup-node@v4
        with:
          node-version: 22
          cache: npm

      - name: Setup Rust
        uses: dtolnay/rust-toolchain@stable

      - name: Rust cache
        uses: Swatinem/rust-cache@v2
        with:
          workspaces: src-tauri

      - name: Install frontend dependencies
        run: npm ci

      - name: Frontend typecheck
        run: npm run typecheck

      - name: Frontend lint
        run: npm run lint

      - name: Frontend build
        run: npm run build

      - name: Rust tests
        working-directory: src-tauri
        run: cargo test --all-targets

      - name: Rust clippy
        working-directory: src-tauri
        run: cargo clippy --all-targets -- -D warnings
```

### Что проверяется в CI

| Проверка                    | Команда                               | Что ловит                                                               |
| --------------------------- | ------------------------------------- | ----------------------------------------------------------------------- |
| TypeScript типы             | `npm run typecheck`                   | Ошибки типизации в React-коде                                           |
| ESLint                      | `npm run lint`                        | Code style, неиспользуемые переменные, ошибки хуков                     |
| Frontend сборка             | `npm run build` (`tsc && vite build`) | Битый импорт, ошибки сборки                                             |
| Rust unit/integration тесты | `cargo test --all-targets`            | Регрессии в config, scheduler, storage, events, notifications, activity |
| Rust clippy                 | `cargo clippy -- -D warnings`         | Потенциальные баги, идиоматичность кода                                 |

### Текущее покрытие Rust тестами

Уже существуют 39 тестов в 6 модулях:

- `config.rs` — 12 тестов (валидация конфига, ConfigManager save/reject);
- `events.rs` — 4 теста (EventBus emit/subscribe, overflow);
- `notifications.rs` — 3 теста (MockNotifier dedup, close, create-after-close);
- `scheduler.rs` — 11 тестов (FSM transitions, full cycle, skip/snooze/defer);
- `storage.rs` — 5 тестов (SQLite round-trip, idempotent save, defaults);
- `activity.rs` — 4 теста (idle detection, user returned).

### Важно

- CI workflow **не** собирает Linux bundles (`.deb`, `.rpm`, `AppImage`) — это задача release workflow. CI только проверяет, что код компилируется и тесты проходят.
- `cargo test` запускает тесты без GUI, используя моки (`MockStorage`, `MockNotifier`, `MockActivity`).
- Если CI красный — тег не создается, релиз не происходит.

## Фаза 5. Настроить GitHub Actions для релизов (release.yml)

Нужен workflow по тегу, а не по обычному push. Release workflow **обязательно прогоняет тесты повторно** перед сборкой, чтобы гарантировать, что артефакты собраны из проверенного кода.

Рекомендуемый trigger:

- push tag `v*`

Пример жизненного цикла:

- CI на main зелёный;
- разработчик обновляет версию в `tauri.conf.json` и `package.json`;
- создает commit и тег `v0.1.0`;
- тег пушится в репозиторий;
- GitHub Actions запускает release workflow;
- **сначала проходят тесты** (`cargo test`, `typecheck`, `lint`);
- если тесты зелёные — собирается release;
- workflow создает или обновляет GitHub Release;
- артефакты прикрепляются к Release.

Структура workflow `.github/workflows/release.yml`:

```yaml
name: Release

on:
  push:
    tags:
      - "v*"

permissions:
  contents: write

env:
  CARGO_TERM_COLOR: always

jobs:
  # --- Шаг 1: тесты ---
  test:
    name: Pre-release tests
    runs-on: ubuntu-22.04
    steps:
      - uses: actions/checkout@v4

      - name: Install system dependencies
        run: |
          sudo apt-get update
          sudo apt-get install -y \
            libgtk-3-dev \
            libwebkit2gtk-4.1-dev \
            libayatana-appindicator3-dev \
            librsvg2-dev \
            patchelf

      - name: Setup Node.js
        uses: actions/setup-node@v4
        with:
          node-version: 22
          cache: npm

      - name: Setup Rust
        uses: dtolnay/rust-toolchain@stable

      - name: Rust cache
        uses: Swatinem/rust-cache@v2
        with:
          workspaces: src-tauri

      - name: Install frontend dependencies
        run: npm ci

      - name: Frontend typecheck
        run: npm run typecheck

      - name: Frontend lint
        run: npm run lint

      - name: Rust tests
        working-directory: src-tauri
        run: cargo test --all-targets

  # --- Шаг 2: сборка (только после тестов) ---
  build:
    name: Build & Release
    needs: test
    runs-on: ubuntu-22.04
    steps:
      - uses: actions/checkout@v4

      - name: Install system dependencies
        run: |
          sudo apt-get update
          sudo apt-get install -y \
            libgtk-3-dev \
            libwebkit2gtk-4.1-dev \
            libayatana-appindicator3-dev \
            librsvg2-dev \
            patchelf \
            rpm

      - name: Setup Node.js
        uses: actions/setup-node@v4
        with:
          node-version: 22
          cache: npm

      - name: Setup Rust
        uses: dtolnay/rust-toolchain@stable

      - name: Rust cache
        uses: Swatinem/rust-cache@v2
        with:
          workspaces: src-tauri

      - name: Install frontend dependencies
        run: npm ci

      - name: Build Tauri app
        run: npm run tauri build -- --bundles appimage,deb,rpm
        env:
          TAURI_SIGNING_PRIVATE_KEY: ${{ secrets.TAURI_SIGNING_PRIVATE_KEY }}
          TAURI_SIGNING_PRIVATE_KEY_PASSWORD: ${{ secrets.TAURI_SIGNING_PRIVATE_KEY_PASSWORD }}

      - name: Generate latest.json for updater
        run: |
          # ... см. Фазу 11 для деталей генерации latest.json

      - name: Create GitHub Release
        env:
          GH_TOKEN: ${{ github.token }}
        run: |
          gh release create "${{ github.ref_name }}" \
            --title "${{ github.ref_name }}" \
            --generate-notes \
            src-tauri/target/release/bundle/appimage/*.AppImage \
            src-tauri/target/release/bundle/appimage/*.AppImage.sig \
            src-tauri/target/release/bundle/deb/*.deb \
            src-tauri/target/release/bundle/rpm/*.rpm \
            latest.json
```

### Ключевой момент: `needs: test`

Job `build` зависит от `test`. Если тесты упали — сборка не начнётся. Это гарантирует, что в релиз не попадёт сломанный код.

Права workflow:

- `permissions: contents: write`

Это нужно, чтобы workflow мог создать release и загрузить файлы.

## Фаза 6. Выбрать способ публикации артефактов

Есть два нормальных пути.

Вариант A - рекомендуется:

- использовать `gh release create` + `gh release upload`

Плюсы:

- прозрачно;
- легко контролировать названия и повторные загрузки;
- меньше "магии".

Вариант B:

- `tauri-apps/tauri-action`

Плюсы:

- быстрее старт;
- удобно для типового Tauri pipeline;
- автоматически генерирует `latest.json` для updater.

Для этого проекта рекомендуется:

- сначала обычный явный workflow с `gh release upload`;
- если понадобится, позже перейти на `tauri-action`.

## Фаза 7. Добавить smoke tests пакетов

Сборка артефакта не гарантирует, что он реально устанавливается.

Минимальные smoke checks:

- Ubuntu:
  - скачать `.deb`;
  - `sudo dpkg -i ...`;
  - проверить, что пакет установился.
- Fedora:
  - скачать `.rpm`;
  - `sudo dnf install -y ./...rpm`;
  - проверить, что пакет установился.
- AppImage:
  - `chmod +x`;
  - запустить хотя бы с базовой sanity-проверкой.

Практично это делать отдельными jobs после build:

- `test-deb-ubuntu`
- `test-rpm-fedora`
- `test-appimage-linux`

Важно:

- для AppImage бывают проблемы с FUSE;
- если FUSE нет, надо предусмотреть fallback-проверку или хотя бы документировать `--appimage-extract-and-run`.

## Фаза 8. Привести README и выдачу пользователю к реальному релизному сценарию

Сейчас раздел установки выглядит как заготовка. После внедрения релизов нужно сделать его реальным install guide.

Что обновить:

- прямые инструкции для Ubuntu:
  - скачать `.deb`;
  - `sudo dpkg -i blinkly_...deb`;
  - при необходимости `sudo apt -f install`.
- прямые инструкции для Fedora:
  - `sudo dnf install ./blinkly-....rpm`
- инструкции для AppImage:
  - `chmod +x`
  - запуск
  - если проблемы с FUSE - запасной способ
- ссылка на страницу Releases
- таблица "что скачать на какой системе"
- **информация об автообновлениях**: пояснить, что AppImage обновляется автоматически, а для `.deb`/`.rpm` приложение покажет уведомление.

Пример логики:

- Ubuntu/Debian -> `.deb`
- Fedora -> `.rpm`
- если не уверены -> `AppImage` (рекомендуется для автообновлений)

## Фаза 9. Нормализовать versioning

Сейчас версия есть в:

- `package.json`
- `src-tauri/tauri.conf.json`

Нужно выбрать один источник правды и держать его синхронным.

Рекомендуется:

- использовать `src-tauri/tauri.conf.json` как релизную версию;
- при релизе тег должен совпадать: `v0.1.0`.

Что зафиксировать процессно:

- обновили версию в `tauri.conf.json` и `package.json`;
- прогнали тесты локально (`cargo test`, `npm run typecheck`, `npm run lint`);
- создали commit;
- создали tag `vX.Y.Z`;
- push tag;
- CI прогоняет тесты;
- если тесты зелёные — CI выпускает release.

### Валидация версии в CI

В release workflow стоит добавить шаг, который проверяет, что:

- тег `v0.1.0` совпадает с версией из `tauri.conf.json`;
- версия из `package.json` совпадает с `tauri.conf.json`.

Пример:

```yaml
- name: Validate version consistency
  run: |
    TAG_VERSION="${{ github.ref_name }}"
    TAG_VERSION="${TAG_VERSION#v}"  # убрать префикс v
    TAURI_VERSION=$(jq -r '.version' src-tauri/tauri.conf.json)
    NPM_VERSION=$(jq -r '.version' package.json)
    if [ "$TAG_VERSION" != "$TAURI_VERSION" ]; then
      echo "ERROR: Tag ($TAG_VERSION) != tauri.conf.json ($TAURI_VERSION)"
      exit 1
    fi
    if [ "$TAG_VERSION" != "$NPM_VERSION" ]; then
      echo "ERROR: Tag ($TAG_VERSION) != package.json ($NPM_VERSION)"
      exit 1
    fi
    echo "Version $TAG_VERSION is consistent across all sources"
```

## Фаза 10. Проверить Linux packaging-специфику

Есть несколько типичных подводных камней.

Для `.deb`:

- могут подтянуться слишком новые зависимости, если собирать на слишком свежем runner;
- иногда нужно вручную уточнять runtime deps;
- надо проверить установку на Ubuntu 22.04 и 24.04.

Для `.rpm`:

- у Fedora строже отношение к metadata;
- license лучше указывать в нормальном SPDX-виде;
- иногда dependency naming отличается.

Для `AppImage`:

- у части пользователей нет FUSE;
- desktop integration может вести себя по-разному;
- tray, Wayland и X11 стоит проверить руками;
- **AppImage — единственный формат, для которого работает `tauri-plugin-updater` на Linux**. Это важно учитывать при рекомендации формата пользователям.

---

## Фаза 11. Подписание обновлений (signing keys)

Tauri Updater требует криптографической подписи каждого обновления. Без подписи updater не установит артефакт.

### Генерация ключевой пары

Выполнить один раз:

```bash
npm run tauri signer generate -- -w ~/.tauri/blinkly.key
```

Это создаст:

- `~/.tauri/blinkly.key` — **приватный ключ** (НИКОГДА не коммитить, не делиться);
- `~/.tauri/blinkly.key.pub` — **публичный ключ** (безопасно хранить в конфиге).

### Хранение ключей

| Ключ                          | Где хранить                                          | Доступ    |
| ----------------------------- | ---------------------------------------------------- | --------- |
| Приватный (`blinkly.key`)     | GitHub Secrets: `TAURI_SIGNING_PRIVATE_KEY`          | Только CI |
| Пароль ключа                  | GitHub Secrets: `TAURI_SIGNING_PRIVATE_KEY_PASSWORD` | Только CI |
| Публичный (`blinkly.key.pub`) | `tauri.conf.json` → `plugins.updater.pubkey`         | Публичный |

### Настройка GitHub Secrets

В настройках репозитория: **Settings → Secrets and variables → Actions**:

- `TAURI_SIGNING_PRIVATE_KEY` — содержимое файла `blinkly.key` (не путь, а сам текст ключа);
- `TAURI_SIGNING_PRIVATE_KEY_PASSWORD` — пароль, если был задан при генерации (может быть пустым).

### Сборка с подписью

При наличии переменных окружения `TAURI_SIGNING_PRIVATE_KEY` и `TAURI_SIGNING_PRIVATE_KEY_PASSWORD` команда `npm run tauri build` автоматически генерирует `.sig` файлы рядом с артефактами:

- `blinkly_0.1.0_amd64.AppImage` → `blinkly_0.1.0_amd64.AppImage.sig`

Эти `.sig` файлы нужны для `latest.json`.

### Важно

- Если потерять приватный ключ — все существующие пользователи **не смогут обновиться** через updater. Придется просить их переустановить приложение вручную.
- Ключ нужно хранить в безопасном резервном месте помимо GitHub Secrets.
- Локальная сборка с подписью: `export TAURI_SIGNING_PRIVATE_KEY="$(cat ~/.tauri/blinkly.key)"`.

## Фаза 12. Настроить Tauri Updater (автообновление для AppImage)

Tauri v2 поддерживает автообновление на Linux **только для AppImage**. Для `.deb` и `.rpm` используется альтернативный механизм уведомлений (см. Фазу 13).

### Зависимости для подключения

**Cargo.toml** (`src-tauri/Cargo.toml`):

```toml
[dependencies]
tauri-plugin-updater = "2"
tauri-plugin-process = "2"  # для relaunch после обновления
```

**package.json**:

```json
{
  "dependencies": {
    "@tauri-apps/plugin-updater": "^2",
    "@tauri-apps/plugin-process": "^2"
  }
}
```

### Конфигурация `tauri.conf.json`

Добавить в корень конфига:

```json
{
  "bundle": {
    "createUpdaterArtifacts": true
  },
  "plugins": {
    "updater": {
      "pubkey": "<СОДЕРЖИМОЕ blinkly.key.pub>",
      "endpoints": ["https://github.com/<owner>/<repo>/releases/latest/download/latest.json"]
    }
  }
}
```

`createUpdaterArtifacts: true` говорит сборщику генерировать `.sig` файлы рядом с AppImage.

### Capabilities

Обновить `src-tauri/capabilities/default.json`:

```json
{
  "$schema": "../gen/schemas/desktop-schema.json",
  "identifier": "default",
  "description": "Capability for all app windows",
  "windows": ["overlay", "settings"],
  "permissions": ["core:default", "opener:default", "updater:default", "process:default"]
}
```

Добавлены:

- `updater:default` — разрешает `check`, `download`, `install`, `download-and-install`;
- `process:default` — разрешает `relaunch` для перезапуска после обновления.

### Инициализация плагинов в `lib.rs`

В функции `run()` при создании Tauri Builder добавить:

```rust
use tauri_plugin_updater;
use tauri_plugin_process;

tauri::Builder::default()
    .plugin(tauri_plugin_updater::Builder::new().build())
    .plugin(tauri_plugin_process::init())
    // ... остальная настройка
```

### Как работает updater на Linux

1. Приложение (из frontend или backend) вызывает `check()`;
2. Updater загружает `latest.json` из endpoint;
3. Сравнивает `latest.json.version` с текущей версией приложения;
4. Если есть новая версия — возвращает объект `Update` с метаданными;
5. `downloadAndInstall()` скачивает новый AppImage, проверяет подпись, заменяет текущий файл;
6. `relaunch()` перезапускает приложение.

### Ограничение: только AppImage

На Linux Tauri Updater заменяет текущий AppImage-файл на новый. Для `.deb` и `.rpm` этот механизм **не работает**, потому что:

- `.deb`/`.rpm` устанавливаются через системный пакетный менеджер с правами root;
- Tauri не может вызвать `dpkg` или `dnf` из sandboxed-приложения;
- нет стандартного способа бесшовного обновления системного пакета из userspace.

Для пользователей `.deb`/`.rpm` используется Фаза 13.

## Фаза 13. Уведомление пользователей об обновлении

Стратегия разная в зависимости от формата установки.

### Определение формата установки

Приложение при старте определяет, как оно было установлено:

```rust
/// Определяет, запущено ли приложение из AppImage.
fn is_appimage() -> bool {
    std::env::var("APPIMAGE").is_ok()
}

/// Определяет тип установки для выбора стратегии обновления.
#[derive(Debug, Clone, Copy, PartialEq)]
enum InstallType {
    AppImage,   // автообновление через tauri-plugin-updater
    SystemPkg,  // .deb или .rpm — уведомление + ссылка
}

fn detect_install_type() -> InstallType {
    if is_appimage() {
        InstallType::AppImage
    } else {
        InstallType::SystemPkg
    }
}
```

Переменная окружения `APPIMAGE` автоматически устанавливается AppImage runtime и содержит путь к файлу `.AppImage`. Если переменная есть — приложение запущено из AppImage. Если нет — скорее всего из `.deb`/`.rpm` (бинарник лежит в `/usr/bin/`).

### Сценарий A: AppImage (полное автообновление)

1. Приложение периодически (раз в 12 часов) или при открытии Settings проверяет наличие обновления через `tauri-plugin-updater`;
2. Если обновление найдено:
   - показывает системное уведомление через `notify-rust`: "Blinkly vX.Y.Z доступна";
   - в окне Settings появляется плашка с кнопкой "Обновить";
3. При нажатии "Обновить":
   - `downloadAndInstall()` скачивает новый AppImage;
   - показывает прогресс-бар скачивания;
   - после завершения — `relaunch()`.

### Сценарий B: .deb/.rpm (уведомление + ссылка)

1. Приложение проверяет наличие новой версии через запрос к `latest.json` (тот же endpoint, что и для updater);
2. Парсит JSON и сравнивает `version` с текущей версией;
3. Если обновление найдено:
   - показывает системное уведомление через `notify-rust`: "Blinkly vX.Y.Z доступна. Скачайте обновление.";
   - в окне Settings появляется плашка с кнопкой "Скачать";
4. При нажатии "Скачать":
   - открывает страницу GitHub Releases в браузере через `tauri-plugin-opener`.

### Периодическая проверка обновлений (backend)

В `lib.rs` при старте приложения запускается фоновая задача:

```rust
fn spawn_update_checker(app_handle: tauri::AppHandle) {
    let install_type = detect_install_type();
    tauri::async_runtime::spawn(async move {
        loop {
            match install_type {
                InstallType::AppImage => {
                    check_update_appimage(&app_handle).await;
                }
                InstallType::SystemPkg => {
                    check_update_system_pkg(&app_handle).await;
                }
            }
            // проверять раз в 12 часов
            tokio::time::sleep(std::time::Duration::from_secs(12 * 60 * 60)).await;
        }
    });
}
```

Для `SystemPkg` проверка — это обычный HTTP-запрос к `latest.json` и сравнение версий:

```rust
async fn check_update_system_pkg(app_handle: &tauri::AppHandle) {
    let url = "https://github.com/<owner>/<repo>/releases/latest/download/latest.json";
    // HTTP GET запрос (через reqwest или tauri-plugin-http)
    // Парсим JSON, сравниваем version с текущей
    // Если есть обновление -> emit event на frontend
    // -> показываем системное уведомление через notify-rust
}
```

### IPC события для frontend

Backend отправляет событие на frontend при обнаружении обновления:

```rust
#[derive(Clone, serde::Serialize)]
struct UpdateAvailable {
    version: String,
    notes: String,
    pub_date: String,
    install_type: String,  // "appimage" или "system_pkg"
}

// Emit из backend:
app_handle.emit("update-available", UpdateAvailable { ... });
```

### Новые IPC команды

Добавить в `commands.rs`:

```rust
#[tauri::command]
async fn check_for_update(app_handle: tauri::AppHandle) -> Result<Option<UpdateInfo>, String> {
    // Проверяет наличие обновления вручную (по запросу из UI)
}

#[tauri::command]
async fn install_update(app_handle: tauri::AppHandle) -> Result<(), String> {
    // Для AppImage: скачивает и устанавливает через tauri-plugin-updater
    // Для system_pkg: открывает GitHub Releases в браузере
}
```

Зарегистрировать в `lib.rs`:

```rust
.invoke_handler(tauri::generate_handler![
    // ... существующие команды ...
    commands::check_for_update,
    commands::install_update,
])
```

## Фаза 14. UI плашка обновления в Settings

В окне настроек при наличии обновления показывается информационная плашка.

### Zustand-стор для состояния обновления

Создать `src/stores/useUpdateStore.ts`:

```typescript
import { create } from "zustand";

interface UpdateInfo {
  version: string;
  notes: string;
  pubDate: string;
  installType: "appimage" | "system_pkg";
}

type UpdateStatus = "idle" | "checking" | "available" | "downloading" | "error";

interface UpdateState {
  status: UpdateStatus;
  update: UpdateInfo | null;
  downloadProgress: number; // 0-100
  error: string | null;

  checkForUpdate: () => Promise<void>;
  installUpdate: () => Promise<void>;
  dismiss: () => void;
}

export const useUpdateStore = create<UpdateState>((set, get) => ({
  status: "idle",
  update: null,
  downloadProgress: 0,
  error: null,

  checkForUpdate: async () => {
    set({ status: "checking", error: null });
    try {
      const result = await invoke<UpdateInfo | null>("check_for_update");
      if (result) {
        set({ status: "available", update: result });
      } else {
        set({ status: "idle" });
      }
    } catch (e) {
      set({ status: "error", error: String(e) });
    }
  },

  installUpdate: async () => {
    const { update } = get();
    if (!update) return;

    if (update.installType === "appimage") {
      set({ status: "downloading" });
      try {
        // Используем tauri-plugin-updater напрямую из JS
        const { check } = await import("@tauri-apps/plugin-updater");
        const { relaunch } = await import("@tauri-apps/plugin-process");
        const upd = await check();
        if (upd) {
          let total = 0;
          await upd.downloadAndInstall((event) => {
            if (event.event === "Started") {
              total = event.data.contentLength ?? 0;
            } else if (event.event === "Progress") {
              const downloaded = (get().downloadProgress / 100) * total + event.data.chunkLength;
              set({ downloadProgress: total > 0 ? Math.round((downloaded / total) * 100) : 0 });
            }
          });
          await relaunch();
        }
      } catch (e) {
        set({ status: "error", error: String(e) });
      }
    } else {
      // Для system_pkg — открыть Releases в браузере
      await invoke("install_update");
      set({ status: "idle" });
    }
  },

  dismiss: () => set({ status: "idle", update: null }),
}));
```

### Компонент `UpdateBanner`

Создать `src/components/settings/UpdateBanner.tsx`:

```tsx
import { useUpdateStore } from "../../stores/useUpdateStore";
import { useEffect } from "react";

export default function UpdateBanner() {
  const { status, update, downloadProgress, error, checkForUpdate, installUpdate, dismiss } =
    useUpdateStore();

  useEffect(() => {
    checkForUpdate();
  }, [checkForUpdate]);

  if (status === "idle" || status === "checking") return null;

  if (status === "error") {
    return (
      <div className="mx-8 mb-4 px-4 py-3 rounded-2xl bg-red-50/80 border border-red-200 text-sm text-red-600">
        Failed to check for updates: {error}
        <button onClick={dismiss} className="ml-2 underline">
          Dismiss
        </button>
      </div>
    );
  }

  if (status === "available" && update) {
    return (
      <div className="mx-8 mb-4 px-4 py-3 rounded-2xl bg-gradient-to-r from-blue-50 to-purple-50 border border-blue-200/60 shadow-sm">
        <div className="flex items-center justify-between">
          <div>
            <p className="text-sm font-semibold text-gray-800">
              Blinkly {update.version} is available
            </p>
            {update.notes && (
              <p className="text-xs text-gray-500 mt-0.5 line-clamp-1">{update.notes}</p>
            )}
          </div>
          <div className="flex items-center gap-2 ml-4 shrink-0">
            <button
              onClick={dismiss}
              className="text-xs text-gray-400 hover:text-gray-600 transition-colors"
            >
              Later
            </button>
            <button
              onClick={installUpdate}
              className="px-4 py-1.5 rounded-xl text-xs font-bold text-white bg-gradient-to-r from-pink-400 to-blue-400 hover:from-pink-500 hover:to-blue-500 shadow-md shadow-pink-500/20 transition-all active:scale-95"
            >
              {update.installType === "appimage" ? "Update" : "Download"}
            </button>
          </div>
        </div>
      </div>
    );
  }

  if (status === "downloading") {
    return (
      <div className="mx-8 mb-4 px-4 py-3 rounded-2xl bg-blue-50/80 border border-blue-200/60">
        <div className="flex items-center gap-3">
          <div className="flex-1">
            <p className="text-sm font-medium text-gray-700">Downloading update...</p>
            <div className="mt-2 h-1.5 rounded-full bg-blue-100 overflow-hidden">
              <div
                className="h-full rounded-full bg-gradient-to-r from-pink-400 to-blue-400 transition-all duration-300"
                style={{ width: `${downloadProgress}%` }}
              />
            </div>
          </div>
          <span className="text-xs text-gray-400 tabular-nums">{downloadProgress}%</span>
        </div>
      </div>
    );
  }

  return null;
}
```

### Интеграция в SettingsLayout

В `src/components/settings/SettingsLayout.tsx` добавить `UpdateBanner` над контентом:

```tsx
import UpdateBanner from "./UpdateBanner";

// ... внутри JSX, перед {renderPage()}:
<div className="flex-1 overflow-y-auto px-8 py-8">
  <UpdateBanner />
  {renderPage()}
  {/* ... */}
</div>;
```

### Подписка на backend-события

При инициализации приложения (в `App.tsx` или `main.tsx`) подписаться на событие `update-available`:

```typescript
import { listen } from "@tauri-apps/api/event";
import { useUpdateStore } from "./stores/useUpdateStore";

// В useEffect при монтировании:
listen<UpdateInfo>("update-available", (event) => {
  useUpdateStore.setState({
    status: "available",
    update: event.payload,
  });
});
```

### Визуальное поведение

- Плашка появляется **в верхней части** окна настроек, над контентом страницы;
- Стилизована в духе существующего дизайна (градиенты `pink → blue`, скруглённые углы `rounded-2xl`, backdrop-blur);
- Кнопка "Update" для AppImage: начинает скачивание с прогресс-баром, затем перезапуск;
- Кнопка "Download" для `.deb`/`.rpm`: открывает страницу GitHub Releases в браузере;
- Кнопка "Later": скрывает плашку до следующей проверки;
- При скачивании показывается анимированный прогресс-бар;
- Ошибки показываются в красной плашке с кнопкой "Dismiss".

### Поток взаимодействия пользователя

```
Запуск приложения
  ↓
Backend: spawn_update_checker() → проверяет latest.json
  ↓ (обновление найдено)
Backend: emit("update-available", { version, notes, installType })
  +
Backend: notify-rust → системное уведомление "Blinkly vX.Y.Z доступна"
  ↓
Пользователь видит системное уведомление
  ↓
Пользователь открывает Settings
  ↓
Frontend: UpdateBanner отображает плашку "Blinkly vX.Y.Z is available"
  ↓
[AppImage] Нажимает "Update"
  → downloadAndInstall() → прогресс-бар → relaunch()
  ↓
[.deb/.rpm] Нажимает "Download"
  → открывается GitHub Releases в браузере
  → пользователь скачивает и устанавливает вручную
```

## Фаза 15. Генерация `latest.json` в CI

Updater требует JSON-манифест определённого формата на endpoint. Этот файл создаётся при сборке release workflow и прикрепляется к GitHub Release.

### Формат `latest.json`

```json
{
  "version": "0.2.0",
  "notes": "Bug fixes and performance improvements",
  "pub_date": "2025-01-15T12:00:00Z",
  "platforms": {
    "linux-x86_64": {
      "signature": "<содержимое .AppImage.sig файла>",
      "url": "https://github.com/<owner>/<repo>/releases/download/v0.2.0/blinkly_0.2.0_amd64.AppImage"
    }
  }
}
```

Ключевые поля:

- `version` — новая версия (должна совпадать с тегом без `v`);
- `platforms.linux-x86_64.signature` — **содержимое** файла `.sig` (не путь к файлу);
- `platforms.linux-x86_64.url` — прямая ссылка на AppImage в GitHub Release.

### Генерация в CI

Добавить step в release workflow **после** сборки, **перед** `gh release create`:

```yaml
- name: Generate latest.json
  run: |
    VERSION="${{ github.ref_name }}"
    VERSION="${VERSION#v}"
    APPIMAGE_FILE=$(ls src-tauri/target/release/bundle/appimage/*.AppImage)
    APPIMAGE_NAME=$(basename "$APPIMAGE_FILE")
    SIG_CONTENT=$(cat "${APPIMAGE_FILE}.sig")
    REPO="${{ github.repository }}"
    TAG="${{ github.ref_name }}"
    NOTES=$(gh api "repos/${REPO}/releases/generate-notes" \
      -f tag_name="${TAG}" \
      --jq '.body' 2>/dev/null || echo "Release ${TAG}")
    PUB_DATE=$(date -u +"%Y-%m-%dT%H:%M:%SZ")

    cat > latest.json << ENDJSON
    {
      "version": "${VERSION}",
      "notes": $(echo "$NOTES" | jq -Rs .),
      "pub_date": "${PUB_DATE}",
      "platforms": {
        "linux-x86_64": {
          "signature": "${SIG_CONTENT}",
          "url": "https://github.com/${REPO}/releases/download/${TAG}/${APPIMAGE_NAME}"
        }
      }
    }
    ENDJSON
  env:
    GH_TOKEN: ${{ github.token }}
```

### Endpoint в `tauri.conf.json`

```json
{
  "plugins": {
    "updater": {
      "endpoints": ["https://github.com/<owner>/<repo>/releases/latest/download/latest.json"]
    }
  }
}
```

GitHub Releases автоматически обеспечивает, что URL `.../releases/latest/download/latest.json` всегда указывает на последний релиз.

### Альтернатива: `tauri-action`

Если перейти на `tauri-apps/tauri-action` (Фаза 6, Вариант B), то `latest.json` генерируется автоматически. Но при ручном workflow его нужно создавать самостоятельно.

### Проверка `latest.json`

После первого релиза проверить:

```bash
curl -sL https://github.com/<owner>/<repo>/releases/latest/download/latest.json | jq .
```

Должен вернуться валидный JSON с полями `version`, `platforms.linux-x86_64.signature`, `platforms.linux-x86_64.url`.

---

## Фаза 16. Определить MVP и расширение после него

Рекомендованный MVP релиза:

- ручная локальная сборка подтверждена;
- CI workflow (`ci.yml`) на каждый push/PR: `cargo test` + `typecheck` + `lint`;
- release workflow (`release.yml`) по тегу с тестами перед сборкой;
- GitHub Release с `.deb`, `.rpm`, `.AppImage`, `.AppImage.sig`, `latest.json`;
- README с инструкциями;
- smoke test установки на Ubuntu и Fedora;
- `tauri-plugin-updater` подключен и настроен;
- автообновление работает для AppImage;
- для `.deb`/`.rpm` — системное уведомление + плашка в Settings с ссылкой на скачивание;
- плашка обновления в Settings с прогресс-баром для AppImage.

Что потом, но не в первый заход:

- Linux package signing (GPG подпись `.deb`/`.rpm`);
- отдельные репозитории пакетов (APT/DNF repo);
- Copr, AUR, Snap или Flatpak;
- delta-обновления (для уменьшения размера скачивания);
- release channels (stable / beta);
- rollback механизм;
- Vitest для frontend unit-тестов;
- e2e-тесты через WebDriver.

## Рекомендуемый порядок внедрения

1. packaging metadata в `tauri.conf.json`
2. локальная сборка `appimage,deb,rpm`
3. ручная проверка `.deb` на Ubuntu и `.rpm` на Fedora
4. **CI workflow (`ci.yml`)** — тесты на каждый push/PR
5. **генерация ключей подписи** (`tauri signer generate`), настройка GitHub Secrets
6. **настройка `tauri-plugin-updater`** в `tauri.conf.json`, `Cargo.toml`, `lib.rs`, `capabilities`
7. GitHub Actions release workflow по тегу с тестами
8. автоматическая загрузка артефактов + `latest.json` в Release
9. **механизм определения типа установки** (`AppImage` vs `SystemPkg`)
10. **фоновая проверка обновлений** в backend + системные уведомления
11. **UI плашка обновления** в Settings (`UpdateBanner`, `useUpdateStore`)
12. **IPC команды** `check_for_update`, `install_update`
13. smoke-test jobs
14. обновление `README.md` (с информацией об автообновлениях)
15. сквозное тестирование: тег → CI → release → updater → обновление

## Критерии готовности

Считать задачу завершенной, когда:

### Базовое

- `npm run tauri build -- --bundles appimage,deb,rpm` стабильно работает;
- в CI по тегу создается GitHub Release;
- к релизу прикреплены все артефакты: `.AppImage`, `.AppImage.sig`, `.deb`, `.rpm`, `latest.json`;
- `.deb` ставится на Ubuntu;
- `.rpm` ставится на Fedora;
- `AppImage` запускается;
- `README.md` объясняет, что качать и как ставить.

### CI/CD

- CI workflow (`ci.yml`) запускается на каждый push в main и каждый PR;
- CI прогоняет: `cargo test`, `npm run typecheck`, `npm run lint`, `cargo clippy`;
- release workflow прогоняет тесты перед сборкой (`needs: test`);
- если тесты красные — артефакты не собираются, релиз не создаётся;
- версия в теге, `tauri.conf.json` и `package.json` совпадает (проверяется в CI).

### Автообновление

- `tauri-plugin-updater` подключен и инициализирован;
- `latest.json` генерируется в CI и прикрепляется к Release;
- ключи подписи сгенерированы, публичный ключ в `tauri.conf.json`, приватный в GitHub Secrets;
- AppImage: при наличии обновления — скачивание + установка + перезапуск работают;
- `.deb`/`.rpm`: при наличии обновления — показывается системное уведомление.

### Уведомления и UI

- при запуске приложение проверяет наличие обновлений;
- если обновление есть — показывается системное уведомление через `notify-rust`;
- в окне Settings отображается плашка `UpdateBanner` с версией и release notes;
- для AppImage: кнопка "Update" запускает скачивание с прогресс-баром и перезапуск;
- для `.deb`/`.rpm`: кнопка "Download" открывает GitHub Releases в браузере;
- кнопка "Later" скрывает плашку до следующей проверки;
- ошибки при проверке/скачивании корректно отображаются.

## Что делать следующим шагом

1. сначала настроить CI workflow (`ci.yml`) — это основа для всего остального;
2. сгенерировать ключи подписи и настроить GitHub Secrets;
3. оформить packaging metadata в `tauri.conf.json`;
4. подключить `tauri-plugin-updater` и `tauri-plugin-process`;
5. настроить release workflow с тестами и генерацией `latest.json`;
6. реализовать `detect_install_type()` и `spawn_update_checker()` в backend;
7. реализовать `UpdateBanner` и `useUpdateStore` на frontend;
8. добавить IPC команды `check_for_update` и `install_update`;
9. отдельно добавить smoke tests;
10. дополировать `README.md` и release notes.
