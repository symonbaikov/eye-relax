# LookAway for Linux — Архитектура и инженерные решения

> Desktop-приложение для мягких напоминаний о перерывах от экрана.
> Tauri 2 · Rust · React · TypeScript · SQLite

---

## Содержание

1. [Обзор системы](#1-обзор-системы)
2. [Файловая структура](#2-файловая-структура)
3. [Архитектурные слои](#3-архитектурные-слои)
4. [Scheduler — конечный автомат](#4-scheduler--конечный-автомат)
5. [Activity Tracker](#5-activity-tracker)
6. [EventBus — шина событий](#6-eventbus--шина-событий)
7. [Storage — персистентный слой](#7-storage--персистентный-слой)
8. [ConfigManager](#8-configmanager)
9. [Tauri IPC — контракт frontend/backend](#9-tauri-ipc--контракт-frontendbackend)
10. [UI-компоненты](#10-ui-компоненты)
11. [NotificationManager](#11-notificationmanager)
12. [System Tray](#12-system-tray)
13. [Платформенные адаптеры](#13-платформенные-адаптеры)
14. [Инициализация и DI](#14-инициализация-и-di)
15. [Идемпотентность](#15-идемпотентность)
16. [Принципы SOLID](#16-принципы-solid)
17. [Тестирование](#17-тестирование)
18. [Зависимости](#18-зависимости)

---

## 1. Обзор системы

LookAway — desktop-приложение для Linux, которое мягко и ненавязчиво напоминает пользователю делать перерывы от экрана. Вдохновлено Look Away на macOS, адаптировано под Linux с улучшенным UX.

Ключевые функции:

- Напоминания по правилу 20-20-20 (20 мин работы → 20 сек смотреть вдаль)
- Микро-паузы и длинные перерывы
- Полупрозрачный overlay с анимацией
- Idle detection (автосброс при отсутствии пользователя)
- Tray-приложение с быстрым доступом
- Статистика активности

---

## 2. Файловая структура

```
lookaway/
├── src-tauri/                        # Rust backend
│   ├── src/
│   │   ├── main.rs                   # Точка входа, DI-контейнер
│   │   ├── scheduler.rs              # FSM таймеров
│   │   ├── activity.rs               # Idle detection
│   │   ├── notifications.rs          # libnotify / D-Bus
│   │   ├── config.rs                 # Настройки + валидация
│   │   ├── stats.rs                  # Статистика сессий
│   │   ├── storage.rs                # SQLite адаптер
│   │   ├── commands.rs               # Tauri IPC commands
│   │   ├── events.rs                 # EventBus + типы событий
│   │   └── platform/
│   │       ├── mod.rs                # Трейт PlatformAdapter
│   │       ├── x11.rs                # X11 реализация
│   │       └── wayland.rs            # Wayland реализация
│   ├── Cargo.toml
│   └── tauri.conf.json
├── src/                              # React frontend
│   ├── App.tsx
│   ├── stores/
│   │   ├── useSchedulerStore.ts      # Zustand: состояние таймера
│   │   └── useConfigStore.ts         # Zustand: настройки
│   ├── components/
│   │   ├── OverlayWindow.tsx         # Окно перерыва
│   │   ├── SettingsPanel.tsx         # Панель настроек
│   │   └── StatsDashboard.tsx        # Дашборд
│   ├── hooks/
│   │   └── useTauriEvents.ts         # Подписка на backend-события
│   └── lib/
│       └── ipc.ts                    # Типизированные invoke-вызовы
├── package.json
├── vite.config.ts
└── tailwind.config.ts
```

---

## 3. Архитектурные слои

Система построена на трёхслойной архитектуре с чётким разделением ответственности:

```
┌─────────────────────────────────────────────┐
│              System Tray                     │
└──────────────────┬──────────────────────────┘
                   │
┌──────────────────▼──────────────────────────┐
│         Rust Backend (Tauri 2)               │
│                                              │
│  ┌───────────┐ ┌──────────────┐ ┌─────────┐ │
│  │ Scheduler │→│ActivityTracker│→│ Notif.  │ │
│  └───────────┘ └──────────────┘ └─────────┘ │
│  ┌─────────────┐  ┌───────────────────────┐ │
│  │ConfigManager│  │   StatsAggregator     │ │
│  └─────────────┘  └───────────────────────┘ │
│                                              │
│           EventBus (broadcast)               │
└──────────────────┬──────────────────────────┘
                   │ Tauri IPC
┌──────────────────▼──────────────────────────┐
│         React + TypeScript UI                │
│                                              │
│  ┌──────────────┐ ┌────────┐ ┌───────────┐  │
│  │OverlayWindow │ │Settings│ │ Stats     │  │
│  └──────────────┘ └────────┘ └───────────┘  │
│                                              │
│        Zustand Store + Framer Motion         │
└──────────────────┬──────────────────────────┘
                   │ Read / Write
┌──────────────────▼──────────────────────────┐
│            SQLite (Storage)                  │
│    настройки · статистика · сессии           │
└─────────────────────────────────────────────┘
```

**Слой 1 — Rust Backend.** Таймеры, отслеживание активности через X11/Wayland API, нотификации, агрегация статистики. Модули общаются через `EventBus`, с фронтом — через Tauri IPC commands.

**Слой 2 — React UI.** Три ключевых компонента: overlay-окно, панель настроек, дашборд. Zustand держит состояние, Framer Motion анимирует переходы. UI не содержит бизнес-логики.

**Слой 3 — Storage.** SQLite через rusqlite с WAL-режимом. Путь: `$XDG_DATA_HOME/lookaway/data.db`.

---

## 4. Scheduler — конечный автомат

Scheduler — ядро приложения. Реализован как конечный автомат (FSM) с четырьмя состояниями.

### 4.1 Состояния

| Состояние | Описание |
|-----------|----------|
| `Idle` | Приложение запущено, таймер не активен. Начальное состояние |
| `Working` | Обратный отсчёт до перерыва (20 мин по умолчанию). `tokio::time::interval` |
| `OnBreak` | Перерыв активен: overlay показан, таймер отсчитывает 20 секунд |
| `Paused` | Пользователь приостановил напоминания через tray-меню |

### 4.2 Таблица переходов

```
Idle ──start_timer──▶ Working
Working ──timer_elapsed──▶ OnBreak
Working ──user_idle(5min)──▶ Idle (авто-сброс)
Working ──pause──▶ Paused
OnBreak ──break_complete──▶ Working
OnBreak ──skip/snooze──▶ Working
Paused ──resume──▶ Working
```

| Из состояния | Событие | В состояние |
|-------------|---------|-------------|
| Idle | `start_timer` | Working |
| Working | `timer_elapsed` | OnBreak |
| Working | `user_idle(5min)` | Idle (auto-reset) |
| Working | `pause` | Paused |
| OnBreak | `break_complete` | Working |
| OnBreak | `skip` / `snooze` | Working |
| Paused | `resume` | Working |

### 4.3 Трейт

```rust
pub trait SchedulerPort: Send + Sync {
    fn start(&self);
    fn skip(&self);
    fn snooze(&self, duration: Duration);
    fn pause(&self);
    fn resume(&self);
    fn state(&self) -> SchedulerState;
    fn remaining(&self) -> Duration;
}

pub enum SchedulerState { Idle, Working, OnBreak, Paused }
```

Реализация `TimerScheduler` использует `tokio::spawn` для асинхронного таймера и `tokio::sync::watch` для броадкаста состояния на UI.

### 4.4 Конфигурация интервалов

| Параметр | Дефолт | Диапазон |
|----------|--------|----------|
| `work_interval` | 20 мин | 5–60 мин |
| `break_duration` | 20 сек | 10–60 сек |
| `long_break_interval` | 60 мин | 30–120 мин |
| `long_break_duration` | 5 мин | 2–15 мин |
| `snooze_duration` | 5 мин | 1–10 мин |
| `idle_threshold` | 5 мин | 2–15 мин |

---

## 5. Activity Tracker

Модуль определяет, находится ли пользователь за компьютером. Если пользователь ушёл — таймер сбрасывается автоматически.

### 5.1 Трейт

```rust
pub trait ActivitySource: Send + Sync {
    fn idle_seconds(&self) -> Result<u64>;
    fn is_screen_locked(&self) -> Result<bool>;
}
```

### 5.2 Адаптеры

| Адаптер | Механизм |
|---------|----------|
| `X11IdleSource` | `XScreenSaverQueryInfo` через `x11rb` crate. Поллинг каждые 30 секунд |
| `WaylandIdleSource` | `ext_idle_notify_v1` протокол через `wayland-client` crate. Event-driven: протокол присылает `idled` / `resumed` |

### 5.3 Логика

1. `ActivityTracker` запускает фоновую задачу (`tokio::spawn`)
2. Poll `idle_seconds()` каждые 30 сек (X11) или слушает события (Wayland)
3. Если `idle > idle_threshold` → эмитит `Event::UserIdle`
4. Scheduler подписан на `Event::UserIdle` → переходит `Working → Idle`
5. При возвращении пользователя → `Event::UserReturned` → `Idle → Working`

> **Выбор адаптера:** определяется в `main.rs` через проверку `$XDG_SESSION_TYPE` (`wayland` / `x11`). Если переменная отсутствует — fallback на X11.

---

## 6. EventBus — шина событий

Все модули общаются через центральную шину событий. Реализация — `tokio::sync::broadcast` с буфером 64 события.

### 6.1 Типы событий

```rust
#[derive(Clone, Debug)]
pub enum AppEvent {
    // Scheduler
    BreakDue { break_type: BreakType },
    BreakCompleted,
    BreakSkipped,
    BreakSnoozed { until: Instant },
    StateChanged(SchedulerState),

    // Activity
    UserIdle { idle_secs: u64 },
    UserReturned,

    // Config
    ConfigUpdated(AppConfig),

    // Stats
    SessionTick { work_secs: u64 },
}
```

### 6.2 API

```rust
pub struct EventBus {
    tx: broadcast::Sender<AppEvent>,
}

impl EventBus {
    pub fn new() -> Self { /* buffer = 64 */ }
    pub fn emit(&self, event: AppEvent);     // неблокирующий
    pub fn subscribe(&self) -> Receiver;     // клон receiver
}
```

Каждый модуль получает `Arc<EventBus>` при инициализации. Подписка через `subscribe()` в своём `tokio::spawn` лупе.

---

## 7. Storage — персистентный слой

Все данные хранятся в одном SQLite-файле. Адаптер использует `rusqlite` с WAL-режимом.

### 7.1 Трейт

```rust
pub trait StoragePort: Send + Sync {
    fn load_config(&self) -> Result<AppConfig>;
    fn save_config(&self, config: &AppConfig) -> Result<()>;
    fn record_break(&self, record: &BreakRecord) -> Result<()>;
    fn get_stats(&self, range: DateRange) -> Result<Vec<DayStat>>;
    fn upsert_session(&self, session: &Session) -> Result<()>;
}
```

### 7.2 Схема БД

```sql
CREATE TABLE IF NOT EXISTS config (
    key   TEXT PRIMARY KEY,
    value TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS breaks (
    id         TEXT PRIMARY KEY,  -- UUID
    type       TEXT NOT NULL,      -- short | long
    status     TEXT NOT NULL,      -- completed | skipped | snoozed
    started_at TEXT NOT NULL,      -- ISO 8601
    ended_at   TEXT
);

CREATE TABLE IF NOT EXISTS sessions (
    id            TEXT PRIMARY KEY,  -- UUID
    date          TEXT NOT NULL,      -- YYYY-MM-DD
    work_seconds  INTEGER DEFAULT 0,
    break_count   INTEGER DEFAULT 0,
    skip_count    INTEGER DEFAULT 0
);
```

### 7.3 Миграции

Схема версионируется через `PRAGMA user_version`. При запуске `SqliteStorage` проверяет текущую версию и применяет миграции последовательно. Все миграции обёрнуты в `IF NOT EXISTS`.

---

## 8. ConfigManager

Настройки загружаются при старте из SQLite, кешируются в памяти и синхронизируются с диском при каждом изменении.

### 8.1 Структура

```rust
#[derive(Clone, Serialize, Deserialize)]
pub struct AppConfig {
    pub work_interval_secs: u64,       // дефолт: 1200
    pub break_duration_secs: u64,       // дефолт: 20
    pub long_break_interval_secs: u64,  // дефолт: 3600
    pub long_break_duration_secs: u64,  // дефолт: 300
    pub snooze_duration_secs: u64,      // дефолт: 300
    pub idle_threshold_secs: u64,       // дефолт: 300
    pub sound_enabled: bool,            // дефолт: true
    pub autostart: bool,                // дефолт: true
    pub theme: Theme,                   // дефолт: System
}
```

### 8.2 Валидация

`ConfigManager` валидирует значения перед сохранением. Невалидное значение возвращает `Result::Err` с описанием ограничения. При успешном сохранении эмитится `Event::ConfigUpdated` — Scheduler перезапускает свой таймер с новыми интервалами без перезапуска приложения.

---

## 9. Tauri IPC — контракт frontend/backend

Frontend общается с backend исключительно через Tauri IPC commands (фронт → бэк) и Tauri events (бэк → фронт).

### 9.1 Commands (frontend → backend)

| Command | Параметры | Возврат |
|---------|-----------|---------|
| `get_state` | — | `SchedulerState` |
| `get_remaining` | — | `u64` (seconds) |
| `skip_break` | — | `()` |
| `snooze_break` | `duration_secs: u64` | `()` |
| `pause_timer` | — | `()` |
| `resume_timer` | — | `()` |
| `get_config` | — | `AppConfig` |
| `set_config` | `AppConfig` | `Result<()>` |
| `get_stats` | `DateRange` | `Vec<DayStat>` |

### 9.2 Events (backend → frontend)

| Событие | Payload |
|---------|---------|
| `state-changed` | `{ state: string, remaining: number }` |
| `break-due` | `{ type: "short" \| "long" }` |
| `break-tick` | `{ remaining: number }` |
| `break-completed` | `{}` |
| `config-updated` | `AppConfig` |

### 9.3 Типизированные вызовы

```typescript
// src/lib/ipc.ts
import { invoke } from '@tauri-apps/api/core';

export const getState = () =>
  invoke<SchedulerState>('get_state');

export const skipBreak = () =>
  invoke<void>('skip_break');

export const setConfig = (config: AppConfig) =>
  invoke<void>('set_config', { config });
```

---

## 10. UI-компоненты

### 10.1 OverlayWindow

Полупрозрачное окно поверх всех окон. Реализовано как отдельное Tauri-окно (`WebviewWindow`):

- `transparent: true` — прозрачный фон
- `always_on_top: true` — поверх всех окон
- `decorations: false` — без рамки окна
- Размер: 400×300px, центрировано

Компонент подписан на `break-due` и `break-tick`. При `break-due` окно появляется с Framer Motion анимацией (`opacity 0→1, scale 0.95→1, 300ms`). При `break-completed` — обратная анимация и скрытие.

Элементы окна: круговой прогресс-бар (SVG arc), текст оставшегося времени, кнопка Skip, кнопка Snooze, мотивационное сообщение (ротация из массива строк).

### 10.2 SettingsPanel

Панель настроек открывается из tray-меню как отдельное Tauri-окно. При открытии запрашивает `get_config`, рендерит форму. При сохранении вызывает `set_config`.

Поля формы: слайдер интервала работы, слайдер длительности перерыва, слайдер snooze, тоггл звука, тоггл автозапуска, выбор темы (Light / Dark / System).

### 10.3 StatsDashboard

Дашборд отображает статистику за период. При открытии вызывает `get_stats` с диапазоном 7 дней. Отображает: общее время работы, количество перерывов, процент пропущенных, график по дням (bar chart на CSS/SVG).

### 10.4 Zustand Store

```typescript
interface SchedulerStore {
  state: SchedulerState;
  remaining: number;
  breakType: 'short' | 'long' | null;

  skip: () => Promise<void>;
  snooze: () => Promise<void>;
  pause: () => Promise<void>;
  resume: () => Promise<void>;
}
```

Стор инициализируется в `App.tsx`. Хук `useTauriEvents` подписывается на `state-changed` и `break-tick` и обновляет стор автоматически. Компоненты подписаны через селекторы — перерендер только при изменении нужного поля.

---

## 11. NotificationManager

Уведомления используются как мягкий fallback, когда overlay не может быть показан (полноэкранный режим), и как дополнительный сигнал.

### 11.1 Трейт

```rust
pub trait NotificationPort: Send + Sync {
    fn send(&self, title: &str, body: &str) -> Result<()>;
    fn supports_actions(&self) -> bool;
}
```

### 11.2 Реализации

| Адаптер | Механизм |
|---------|----------|
| `DbusNotifier` | Прямой вызов `org.freedesktop.Notifications` через `zbus` crate. Поддерживает кнопки действий (Skip, Snooze) |
| `LibnotifyNotifier` | Fallback через `notify-rust` crate. Проще, но без кнопок действий |

> **Дедупликация:** `NotificationManager` хранит `last_notification_id`. Повторный вызов `send()` с тем же событием обновляет существующее уведомление вместо создания нового.

---

## 12. System Tray

Tray-иконка — основной интерфейс взаимодействия. Приложение не имеет главного окна — только tray.

### 12.1 Меню

| Пункт | Действие |
|-------|----------|
| Статус: Working (14:32) | Информационный, некликабельный |
| Пауза / Продолжить | Тоггл `pause` / `resume` |
| Перерыв сейчас | Принудительный `Working → OnBreak` |
| Настройки | Открывает `SettingsPanel` |
| Статистика | Открывает `StatsDashboard` |
| Выход | `process::exit(0)` |

### 12.2 Иконки состояния

| Состояние | Иконка |
|-----------|--------|
| Working | Глаз открыт (зелёный) |
| OnBreak | Глаз закрыт (жёлтый) |
| Paused | Глаз с паузой (серый) |

Иконки — SVG, конвертируются в PNG 22×22px при сборке.

---

## 13. Платформенные адаптеры

Платформенно-зависимый код изолирован в каталоге `platform/`. Все адаптеры реализуют общие трейты. Выбор происходит один раз при старте.

### 13.1 Сравнение платформ

| Возможность | X11 | Wayland |
|-------------|-----|---------|
| Idle detection | `XScreenSaverQueryInfo` | `ext_idle_notify_v1` |
| Поллинг/события | Поллинг 30с | Event-driven |
| Window overlay | `XSetAttributes` | `layer-shell` протокол |
| Global hotkeys | Полная поддержка | Ограниченная |
| Screen lock detect | XScreenSaver events | D-Bus (logind) |

### 13.2 Стратегия раскатки

1. **GNOME + Wayland** — основной таргет
2. **KDE Plasma** — второй приоритет
3. **X11 fallback** — обратная совместимость
4. **Flatpak sandbox** — дополнительный формат

Распространение: AppImage (основной, без зависимостей) → `.deb` → Flatpak.

---

## 14. Инициализация и DI

Все зависимости собираются в `main.rs`. Никаких DI-фреймворков — ручная инъекция через конструкторы.

```rust
fn main() {
    // 1. Storage
    let storage = Arc::new(SqliteStorage::new(db_path)?);

    // 2. EventBus
    let bus = Arc::new(EventBus::new());

    // 3. Config
    let config = ConfigManager::new(storage.clone(), bus.clone());

    // 4. Platform adapter (выбор X11/Wayland)
    let activity: Arc<dyn ActivitySource> =
        match detect_session_type() {
            Session::Wayland => Arc::new(WaylandIdleSource),
            Session::X11     => Arc::new(X11IdleSource),
        };

    // 5. Scheduler
    let scheduler = TimerScheduler::new(bus.clone(), config.current());

    // 6. Activity Tracker
    let tracker = ActivityTracker::new(
        activity, bus.clone(), config.current()
    );

    // 7. Notifications
    let notifier: Arc<dyn NotificationPort> =
        Arc::new(DbusNotifier::new()?);

    // 8. Tauri app
    tauri::Builder::default()
        .manage(scheduler)
        .manage(config)
        .manage(notifier)
        .manage(storage)
        .invoke_handler(commands::handler())
        .system_tray(build_tray())
        .run()?;
}
```

---

## 15. Идемпотентность

Повторный вызов любой операции даёт тот же результат, что и первый. Критично для утилиты, работающей часами без перезапуска.

### Scheduler

Вызов `start()` в состоянии `Working` — no-op. Вызов `skip()` в состоянии `Working` — no-op. Проверка состояния через `match` перед каждым переходом. FSM гарантирует: переход возможен только из определённого состояния.

### Storage

`record_break()` использует `INSERT OR REPLACE` по UUID. Повторный вызов с тем же `id` перезаписывает запись, не создавая дубликат. `upsert_session()` аналогично.

### ConfigManager

`save_config()` идемпотентна по определению — `set_interval(20)` всегда записывает 20 вне зависимости от количества вызовов.

### Overlay

Двойной вызов `show_overlay` не создаёт два окна. Модуль хранит ссылку на текущее окно — если оно уже существует, повторный вызов выводит его на передний план.

### NotificationManager

Повторный `send()` обновляет существующее уведомление через `last_notification_id`, а не создаёт новое.

---

## 16. Принципы SOLID

### S — Single Responsibility

Каждый модуль отвечает за одну вещь. `Scheduler` не знает, как показать overlay. `ActivityTracker` не знает, как сохранить статистику. Шесть модулей — шесть зон ответственности.

### O — Open/Closed

Система открыта для расширения через Rust traits. `ActivityTracker` реализует трейт `ActivitySource`. Можно добавить `WaylandActivitySource` рядом с `X11ActivitySource` без изменения кода `Scheduler`. Добавление нового типа перерыва — новая реализация `BreakStrategy`, а не правка существующего кода.

### L — Liskov Substitution

Любая реализация `ActivitySource` (X11, Wayland, тестовый мок) подставляется без изменения поведения системы. `BreakStrategy` работает одинаково — `ShortBreak`, `LongBreak` или `ExerciseBreak`. Потребитель не знает конкретную реализацию.

### I — Interface Segregation

UI-слой видит только `SchedulerCommands` (`start`, `skip`, `snooze`) и `SchedulerState` (статус, оставшееся время). Он не видит внутренние трейты `ActivitySource` или `StorageBackend`. Тонкие, специализированные интерфейсы вместо одного монолитного.

### D — Dependency Inversion

Высокоуровневые модули (`Scheduler`) зависят от абстракций (`trait StoragePort`, `trait ActivitySource`), а не от конкретных реализаций (`SqliteStorage`, `X11Tracker`). Конкретные реализации инжектируются при инициализации. В тестах — `MockStorage`, в проде — `SqliteStorage`.

---

## 17. Тестирование

### 17.1 Unit-тесты (Rust)

Каждый модуль тестируется изолированно. Зависимости подменяются моками:

- `MockStorage` реализует `StoragePort` с `HashMap` в памяти
- `MockActivity` реализует `ActivitySource` с настраиваемым `idle_seconds`
- `MockNotifier` реализует `NotificationPort` с записью вызовов

### 17.2 Ключевые тест-кейсы

| Тест | Проверяет |
|------|-----------|
| `scheduler_idle_to_working` | Переход `start()` из Idle в Working |
| `scheduler_double_start_noop` | Повторный `start()` в Working = no-op |
| `scheduler_skip_resets_timer` | Skip во время OnBreak → Working |
| `tracker_idle_emits_event` | `idle > threshold` → `Event::UserIdle` |
| `config_invalid_rejected` | Значение вне диапазона → `Err` |
| `storage_upsert_idempotent` | Двойной `record_break` с тем же UUID = 1 запись |
| `overlay_single_instance` | Двойной `show_overlay` = 1 окно |

### 17.3 Интеграционные тесты

Полный цикл: `start → Working → timer_elapsed → OnBreak → break_complete → Working`. Проверяется что события `BreakDue` и `BreakCompleted` эмитятся в правильном порядке, статистика записывается в storage. Использует `tokio::time::pause()` для детерминистичного управления временем.

---

## 18. Зависимости

### Rust (Cargo)

| Crate | Версия | Назначение |
|-------|--------|-----------|
| `tauri` | 2.x | Desktop framework |
| `tokio` | 1.x | Async runtime |
| `rusqlite` | 0.31 | SQLite адаптер |
| `serde` / `serde_json` | 1.x | Сериализация |
| `uuid` | 1.x | Генерация UUID v4 |
| `x11rb` | 0.13 | X11 протокол (idle) |
| `wayland-client` | 0.31 | Wayland протокол |
| `zbus` | 4.x | D-Bus для нотификаций |
| `notify-rust` | 4.x | Fallback нотификации |
| `chrono` | 0.4 | Работа с датами |
| `tracing` | 0.1 | Логирование |

### Frontend (npm)

| Пакет | Назначение |
|-------|-----------|
| `react` + `react-dom` | UI framework |
| `typescript` | Типизация |
| `@tauri-apps/api` | Tauri IPC |
| `zustand` | State management |
| `framer-motion` | Анимации |
| `tailwindcss` | Стилизация |
| `vite` | Сборка |