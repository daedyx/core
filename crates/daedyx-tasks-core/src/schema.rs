//! SQL der Kern-Stände. Stand 1 = Desk-Migration 7 (Listen, Aufgaben, Verlauf), Stand 2 = Desk-Migration 8 (Abschnitte), Stand 3 = Farben, Verweise, Heimat, mehrere Erinnerungen (K2).
//! **Wortgleiche Kopie** der Desk-Texte; ein Test in Desk (`schema_tasks_core_tests`) vergleicht beide Wege.

/// Stand 1: `task_lists`, `tasks`, `task_completions`, `task_alarms_fired`, `task_meta`.
pub const STAND_1: &str = "
CREATE TABLE task_lists (
    id         TEXT PRIMARY KEY,
    name       TEXT NOT NULL,
    color      TEXT NOT NULL DEFAULT '',
    position   REAL NOT NULL DEFAULT 0,
    owner_id   TEXT NULL,
    clocks     TEXT NOT NULL DEFAULT '{}',
    updated_at INTEGER NOT NULL,
    deleted_at INTEGER NULL,
    dirty      INTEGER NOT NULL DEFAULT 1
);

CREATE TABLE tasks (
    id             TEXT PRIMARY KEY,
    list_id        TEXT NOT NULL REFERENCES task_lists(id) ON DELETE CASCADE,
    parent_id      TEXT NULL REFERENCES tasks(id) ON DELETE CASCADE,
    title          TEXT NOT NULL,
    notes          TEXT NOT NULL DEFAULT '',
    due            TEXT NULL,
    due_has_time   INTEGER NOT NULL DEFAULT 0,
    priority       INTEGER NOT NULL DEFAULT 0 CHECK (priority BETWEEN 0 AND 3),
    done_at        INTEGER NULL,
    position       REAL NOT NULL DEFAULT 0,
    repeat_rule    TEXT NULL,
    remind_minutes INTEGER NULL,
    assignee_id    TEXT NULL,
    created_by     TEXT NULL,
    clocks         TEXT NOT NULL DEFAULT '{}',
    updated_at     INTEGER NOT NULL,
    deleted_at     INTEGER NULL,
    dirty          INTEGER NOT NULL DEFAULT 1
);
CREATE INDEX idx_tasks_list ON tasks (list_id, parent_id, position);
CREATE INDEX idx_tasks_due ON tasks (due);
CREATE INDEX idx_tasks_title ON tasks (title COLLATE NOCASE);

CREATE TABLE task_completions (
    id         TEXT PRIMARY KEY,
    task_id    TEXT NOT NULL REFERENCES tasks(id) ON DELETE CASCADE,
    done_at    INTEGER NOT NULL,
    due_was    TEXT NULL,
    clocks     TEXT NOT NULL DEFAULT '{}',
    updated_at INTEGER NOT NULL,
    deleted_at INTEGER NULL,
    dirty      INTEGER NOT NULL DEFAULT 1
);
CREATE INDEX idx_task_completions_task ON task_completions (task_id);

-- Schon gemeldete Erinnerungen (`tasks/reminders.rs`), nur lokal, wird nicht abgeglichen.
CREATE TABLE task_alarms_fired (
    task_id TEXT NOT NULL,
    fire_at INTEGER NOT NULL,
    PRIMARY KEY (task_id, fire_at)
);

-- Gerätekennung und letzter Stempel der Uhr (`tasks/meta.rs`).
CREATE TABLE task_meta (
    key   TEXT PRIMARY KEY,
    value TEXT NOT NULL
);
";

/// Stand 2: `task_sections`, `tasks.section_id`.
pub const STAND_2: &str = "
CREATE TABLE task_sections (
    id         TEXT PRIMARY KEY,
    list_id    TEXT NOT NULL REFERENCES task_lists(id) ON DELETE CASCADE,
    name       TEXT NOT NULL,
    position   REAL NOT NULL DEFAULT 0,
    clocks     TEXT NOT NULL DEFAULT '{}',
    updated_at INTEGER NOT NULL,
    deleted_at INTEGER NULL,
    dirty      INTEGER NOT NULL DEFAULT 1
);
CREATE INDEX idx_task_sections_list ON task_sections (list_id, position);

ALTER TABLE tasks ADD COLUMN section_id TEXT NULL REFERENCES task_sections(id) ON DELETE SET NULL;
CREATE INDEX idx_tasks_section ON tasks (section_id);
";

/// Stand 3: Farben (`#RRGGBB` oder NULL = Standard), Verweise (JSON-Liste), Heimat-App und Umzugszeitpunkt der Liste,
/// dazu `task_alarms` (mehrere Erinnerungen je Aufgabe).
pub const STAND_3: &str = "
ALTER TABLE task_sections ADD COLUMN color TEXT NULL;
ALTER TABLE tasks ADD COLUMN color TEXT NULL;
ALTER TABLE tasks ADD COLUMN links TEXT NULL;
ALTER TABLE task_lists ADD COLUMN home TEXT NULL;
ALTER TABLE task_lists ADD COLUMN moved_at INTEGER NULL;

-- Mehrere Erinnerungen je Aufgabe; `tasks.remind_minutes` bleibt als erste Erinnerung für ältere Apps.
CREATE TABLE task_alarms (
    id             TEXT PRIMARY KEY,
    task_id        TEXT NOT NULL REFERENCES tasks(id) ON DELETE CASCADE,
    minutes_before INTEGER NOT NULL DEFAULT 0,
    clocks         TEXT NOT NULL DEFAULT '{}',
    updated_at     INTEGER NOT NULL,
    deleted_at     INTEGER NULL,
    dirty          INTEGER NOT NULL DEFAULT 1
);
CREATE INDEX idx_task_alarms_task ON task_alarms (task_id);
";
