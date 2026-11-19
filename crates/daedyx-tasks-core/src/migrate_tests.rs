use super::*;
use crate::schema::{STAND_1, STAND_2, STAND_3};

fn conn() -> Connection {
    let c = Connection::open_in_memory().unwrap();
    c.pragma_update(None, "foreign_keys", "ON").unwrap();
    c
}

fn tables(c: &Connection) -> Vec<String> {
    let mut s = c.prepare("SELECT name FROM sqlite_master WHERE type = 'table' ORDER BY name").unwrap();
    s.query_map([], |r| r.get(0)).unwrap().map(|r| r.unwrap()).collect()
}

fn stored_version(c: &Connection) -> Option<String> {
    c.query_row("SELECT value FROM task_meta WHERE key = 'schema_version'", [], |r| r.get(0)).ok()
}

fn insert_list(c: &Connection, id: &str, name: &str) {
    c.execute(
        "INSERT INTO task_lists (id, name, clocks, updated_at, dirty) VALUES (?1, ?2, '{\"name\":\"a-1\"}', 5, 0)",
        [id, name],
    )
    .unwrap();
}

fn list_dump(c: &Connection) -> Vec<(String, String, String, i64, i64)> {
    let mut s = c.prepare("SELECT id, name, clocks, updated_at, dirty FROM task_lists ORDER BY id").unwrap();
    s.query_map([], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?, r.get(4)?))).unwrap().map(|r| r.unwrap()).collect()
}

#[test]
fn a_fresh_database_gets_all_tables_and_the_current_stand() {
    let c = conn();
    migrate(&c).unwrap();
    let t = tables(&c);
    for name in ["task_lists", "tasks", "task_completions", "task_alarms_fired", "task_meta", "task_sections"] {
        assert!(t.contains(&name.to_string()), "{name} fehlt");
    }
    assert_eq!(stored_version(&c), Some(SCHEMA_VERSION.to_string()));
}

#[test]
fn stand_1_without_a_stored_version_is_detected_and_lifted_keeping_rows() {
    let c = conn();
    c.execute_batch(STAND_1).unwrap();
    insert_list(&c, "l1", "Haushalt");
    let before = list_dump(&c);
    migrate(&c).unwrap();
    assert!(tables(&c).contains(&"task_sections".to_string()));
    assert_eq!(list_dump(&c), before);
    assert_eq!(stored_version(&c), Some(SCHEMA_VERSION.to_string()));
}

#[test]
fn stand_2_without_a_stored_version_is_lifted_to_the_current_stand() {
    let c = conn();
    c.execute_batch(STAND_1).unwrap();
    c.execute_batch(STAND_2).unwrap();
    insert_list(&c, "l1", "Haushalt");
    let before = list_dump(&c);
    migrate(&c).unwrap();
    assert_eq!(list_dump(&c), before);
    assert_eq!(stored_version(&c), Some(SCHEMA_VERSION.to_string()));
}

#[test]
fn running_twice_changes_nothing() {
    let c = conn();
    migrate(&c).unwrap();
    insert_list(&c, "l1", "Haushalt");
    let before = list_dump(&c);
    let tables_before = tables(&c);
    migrate(&c).unwrap();
    assert_eq!(list_dump(&c), before);
    assert_eq!(tables(&c), tables_before);
    assert_eq!(stored_version(&c), Some(SCHEMA_VERSION.to_string()));
}

#[test]
fn a_newer_stand_is_refused_and_nothing_is_written() {
    let c = conn();
    migrate(&c).unwrap();
    c.execute("UPDATE task_meta SET value = '4' WHERE key = 'schema_version'", []).unwrap();
    let err = migrate(&c).unwrap_err();
    assert!(matches!(err, MigrateError::TooNew { found: 4, supported: 3 }));
    assert_eq!(stored_version(&c), Some("4".into()));
}

#[test]
fn an_unreadable_stored_version_is_an_error_not_a_guess() {
    let c = conn();
    migrate(&c).unwrap();
    c.execute("UPDATE task_meta SET value = 'x' WHERE key = 'schema_version'", []).unwrap();
    assert!(matches!(migrate(&c).unwrap_err(), MigrateError::BadVersion(_)));
}

#[test]
fn it_runs_inside_a_foreign_transaction_and_follows_its_fate() {
    let c = conn();
    c.execute_batch("BEGIN").unwrap();
    migrate(&c).unwrap();
    assert_eq!(stored_version(&c), Some(SCHEMA_VERSION.to_string()));
    c.execute_batch("ROLLBACK").unwrap();
    assert!(tables(&c).is_empty());

    c.execute_batch("BEGIN").unwrap();
    migrate(&c).unwrap();
    c.execute_batch("COMMIT").unwrap();
    assert_eq!(stored_version(&c), Some(SCHEMA_VERSION.to_string()));
}

#[test]
fn a_failing_step_leaves_the_database_as_it_was() {
    let c = conn();
    // Fremde Tabelle mit dem Namen einer Kern-Tabelle: der Schritt scheitert nach `task_lists`.
    c.execute_batch("CREATE TABLE tasks (x INTEGER)").unwrap();
    assert!(matches!(migrate(&c).unwrap_err(), MigrateError::Sqlite(_)));
    assert_eq!(tables(&c), vec!["tasks".to_string()]);
    // Die Verbindung ist danach nicht in einer offenen Transaktion hängen geblieben.
    assert!(c.is_autocommit());
}

#[test]
fn a_stored_stand_without_its_tables_is_not_accepted_silently() {
    let c = conn();
    c.execute_batch(STAND_1).unwrap();
    c.execute("INSERT INTO task_meta (key, value) VALUES ('schema_version', '2')", []).unwrap();
    assert!(matches!(migrate(&c).unwrap_err(), MigrateError::Inconsistent { stored: 2 }));
    assert!(!tables(&c).contains(&"task_sections".to_string()), "nichts wird nachgebaut oder geändert");
    assert_eq!(stored_version(&c), Some("2".into()));
}

#[test]
fn a_stored_stand_3_without_its_columns_is_not_accepted_silently() {
    let c = conn();
    c.execute_batch(STAND_1).unwrap();
    c.execute_batch(STAND_2).unwrap();
    c.execute("INSERT INTO task_meta (key, value) VALUES ('schema_version', '3')", []).unwrap();
    assert!(matches!(migrate(&c).unwrap_err(), MigrateError::Inconsistent { stored: 3 }));
    assert!(!has_column(&c, "tasks", "color"), "nichts wird nachgebaut");
}

fn has_column(c: &Connection, table: &str, col: &str) -> bool {
    let mut s = c.prepare(&format!("SELECT name FROM pragma_table_info('{table}')")).unwrap();
    let names: Vec<String> = s.query_map([], |r| r.get(0)).unwrap().map(|r| r.unwrap()).collect();
    names.iter().any(|n| n == col)
}

#[test]
fn stand_2_is_lifted_to_3_keeping_rows_and_adding_empty_columns() {
    let c = conn();
    c.execute_batch(STAND_1).unwrap();
    c.execute_batch(STAND_2).unwrap();
    insert_list(&c, "l1", "Haushalt");
    c.execute("INSERT INTO task_meta (key, value) VALUES ('schema_version', '2')", []).unwrap();
    let before = list_dump(&c);
    migrate(&c).unwrap();
    assert_eq!(list_dump(&c), before);
    for (t, col) in [("task_sections", "color"), ("tasks", "color"), ("tasks", "links"), ("task_lists", "home"), ("task_lists", "moved_at")] {
        assert!(has_column(&c, t, col), "{t}.{col} fehlt");
    }
    let (home, moved): (Option<String>, Option<i64>) =
        c.query_row("SELECT home, moved_at FROM task_lists", [], |r| Ok((r.get(0)?, r.get(1)?))).unwrap();
    assert_eq!((home, moved), (None, None));
    assert_eq!(stored_version(&c), Some("3".into()));
}

#[test]
fn stand_3_applied_by_hand_is_the_same_as_the_migration() {
    let cols = |c: &Connection| -> Vec<String> {
        let mut out = Vec::new();
        for t in ["task_lists", "task_sections", "tasks"] {
            let mut s = c.prepare(&format!("SELECT name, type FROM pragma_table_info('{t}') ORDER BY name")).unwrap();
            out.extend(s.query_map([], |r| Ok(format!("{t}.{}:{}", r.get::<_, String>(0)?, r.get::<_, String>(1)?))).unwrap().map(|r| r.unwrap()));
        }
        out
    };
    let a = conn();
    a.execute_batch(STAND_1).unwrap();
    a.execute_batch(STAND_2).unwrap();
    a.execute_batch(STAND_3).unwrap();
    let b = conn();
    migrate(&b).unwrap();
    assert_eq!(cols(&a), cols(&b));
}

#[test]
fn stored_stands_below_one_are_bad_versions() {
    for bad in ["0", "-5"] {
        let c = conn();
        migrate(&c).unwrap();
        c.execute("UPDATE task_meta SET value = ?1 WHERE key = 'schema_version'", [bad]).unwrap();
        assert!(matches!(migrate(&c).unwrap_err(), MigrateError::BadVersion(_)), "{bad}");
    }
}
