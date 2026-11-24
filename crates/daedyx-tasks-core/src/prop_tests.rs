//! Eigenschafts- und Missbrauchstests (SICHERHEIT 2.4) für alles, was Fremddaten liest: Uhr-Stempel, Felder einer
//! Änderung (Farbe, Verweise, ganze Patches) und Wiederholungsregeln. Erwartung: nie Panik, nie Hängen,
//! Ergebnisse erfüllen die zugesagten Eigenschaften. Fuzzing mit cargo-fuzz: siehe docs/FUZZING.md.
use crate::edit_lists::create_list;
use crate::edit_tasks::{create_task, update_task};
use crate::hlc::Hlc;
use crate::repeat::{advance, next_date, valid_rule, MAX_INTERVAL};
use crate::types::{RepeatKind, RepeatRule};
use crate::validate::{check_color, check_links};
use chrono::{Datelike, Duration, NaiveDate};
use chrono_tz::Europe::Berlin;
use proptest::prelude::*;
use serde_json::{json, Map, Value};

fn json_value() -> impl Strategy<Value = Value> {
    let leaf = prop_oneof![
        Just(Value::Null),
        any::<bool>().prop_map(Value::Bool),
        any::<i64>().prop_map(|n| json!(n)),
        any::<f64>().prop_map(|f| json!(f)),
        ".{0,40}".prop_map(Value::String),
        "(19|20|99)[0-9]{2}-[0-9]{2}-[0-9]{2}".prop_map(Value::String),
    ];
    leaf.prop_recursive(3, 32, 6, |inner| {
        prop_oneof![
            prop::collection::vec(inner.clone(), 0..6).prop_map(Value::Array),
            prop::collection::btree_map("[a-zA-Z_]{1,8}", inner, 0..6)
                .prop_map(|m| Value::Object(m.into_iter().collect())),
        ]
    })
}

fn kind() -> impl Strategy<Value = RepeatKind> {
    prop_oneof![Just(RepeatKind::Daily), Just(RepeatKind::Weekly), Just(RepeatKind::Monthly), Just(RepeatKind::Yearly)]
}

fn any_rule() -> impl Strategy<Value = RepeatRule> {
    (kind(), any::<u32>(), prop::collection::vec(any::<u8>(), 0..10), any::<bool>(), any::<Option<u8>>()).prop_map(
        |(kind, interval, weekdays, from_done, month_day)| RepeatRule {
            kind,
            interval,
            weekdays,
            from_done,
            month_day,
        },
    )
}

fn any_date() -> impl Strategy<Value = NaiveDate> {
    (NaiveDate::MIN.num_days_from_ce()..=NaiveDate::MAX.num_days_from_ce())
        .prop_map(|d| NaiveDate::from_num_days_from_ce_opt(d).unwrap_or(NaiveDate::MIN))
}

/// Das frühere Verfahren Tag für Tag, als Vergleich für gültige Wochenregeln mit kleinem Intervall.
fn weekly_reference(weekdays: &[u8], n: i64, base: NaiveDate) -> Option<NaiveDate> {
    let monday = base.checked_sub_signed(Duration::days(i64::from(base.weekday().num_days_from_monday())))?;
    let mut d = base.succ_opt()?;
    loop {
        let week = d.signed_duration_since(monday).num_days().div_euclid(7);
        let listed = weekdays.iter().any(|w| u32::from(*w) == d.weekday().number_from_monday());
        if listed && (week == 0 || week.rem_euclid(n) == 0) {
            return Some(d);
        }
        d = d.succ_opt()?;
    }
}

proptest! {
    #[test]
    fn hlc_parse_never_panics(s in ".{0,60}") {
        let _ = Hlc::parse(&s);
    }

    #[test]
    fn hlc_text_round_trips_and_keeps_order(a in any::<u64>(), c in any::<u16>(), dev in "[a-z0-9]{1,12}") {
        let h = Hlc { ms: a & 0xFFFF_FFFF_FFFF, counter: c, device: dev };
        prop_assert_eq!(Hlc::parse(&h.to_string()), Some(h));
    }

    #[test]
    fn colors_are_either_refused_or_normalised(v in json_value()) {
        if let Ok(out) = check_color(&v) {
            prop_assert!(out.is_null() || out.as_str().is_some_and(|s| s.len() == 7 && s == s.to_ascii_uppercase()));
        }
    }

    #[test]
    fn links_never_panic_and_stay_text(v in json_value()) {
        if let Ok(out) = check_links(&v) {
            prop_assert!(out.is_null() || out.is_string());
        }
    }

    #[test]
    fn next_date_never_panics_and_moves_forward(rule in any_rule(), base in any_date()) {
        if let Some(next) = next_date(&rule, base) {
            prop_assert!(next > base);
        }
    }

    #[test]
    fn weekly_rule_matches_the_day_by_day_reference(
        days in prop::collection::btree_set(1u8..=7, 1..=7), n in 1u32..=8, offset in 0i64..3000,
    ) {
        let start = NaiveDate::from_ymd_opt(2024, 1, 1).unwrap_or(NaiveDate::MIN);
        let base = start.checked_add_signed(Duration::days(offset)).unwrap_or(start);
        let wd: Vec<u8> = days.into_iter().collect();
        let rule = RepeatRule { kind: RepeatKind::Weekly, interval: n, weekdays: wd.clone(), from_done: false, month_day: None };
        prop_assert_eq!(next_date(&rule, base), weekly_reference(&wd, i64::from(n), base));
    }

    #[test]
    fn advance_never_panics(rule in any_rule(), due in ".{0,24}", ts in any::<i64>(), timed in any::<bool>(), done in any::<i64>()) {
        let _ = advance(&rule, &due, timed, done, Berlin);
        let _ = advance(&rule, &ts.to_string(), timed, done, Berlin);
    }
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(48))]

    /// Ganze Patches aus beliebigem JSON: Fehler sind erlaubt, Panik nicht.
    #[test]
    fn hostile_patches_never_panic(patch in prop::collection::btree_map(
        prop_oneof![Just("title"), Just("notes"), Just("due"), Just("due_has_time"), Just("priority"),
            Just("repeat_rule"), Just("remind_minutes"), Just("color"), Just("links"), Just("done_at"), Just("x; DROP")],
        json_value(), 0..6)) {
        let conn = crate::test_db::fresh();
        let list = create_list(&conn, "L", "", 1000).map_err(|e| TestCaseError::fail(e.to_string()))?;
        let mut first = Map::new();
        first.insert("title".into(), json!("T"));
        let task = create_task(&conn, &list.id, None, &first, 2000).map_err(|e| TestCaseError::fail(e.to_string()))?;
        let fields: Map<String, Value> = patch.into_iter().map(|(k, v)| (k.to_string(), v)).collect();
        let _ = update_task(&conn, &task.id, &fields, 3000);
        let _ = create_task(&conn, &list.id, None, &fields, 4000);
    }
}

fn valid_rule_strategy() -> impl Strategy<Value = RepeatRule> {
    let days = prop::collection::btree_set(1u8..=7, 0..=7).prop_map(|s| s.into_iter().collect::<Vec<u8>>());
    (kind(), 1u32..=MAX_INTERVAL, days, any::<bool>(), prop::option::of(1u8..=31)).prop_map(
        |(kind, interval, weekdays, from_done, month_day)| RepeatRule {
            kind,
            interval,
            weekdays,
            from_done,
            month_day,
        },
    )
}

proptest! {
    /// Gültige Regeln finden immer einen nächsten Termin, und zwar in einem begrenzten Abstand.
    #[test]
    fn valid_rules_always_find_a_bounded_next_date(rule in valid_rule_strategy(), offset in 0i64..120_000) {
        let start = NaiveDate::from_ymd_opt(1900, 1, 1).unwrap_or(NaiveDate::MIN);
        let base = start.checked_add_signed(Duration::days(offset)).unwrap_or(start);
        prop_assert!(valid_rule(&rule));
        let next = next_date(&rule, base);
        prop_assert!(next.is_some(), "{rule:?} {base}");
        let gap = next.map_or(0, |n| n.signed_duration_since(base).num_days());
        prop_assert!(gap > 0 && gap <= i64::from(rule.interval).saturating_mul(366).saturating_add(31), "{gap}");
    }

    /// Die Eingabeprüfung nimmt genau die Regeln an, die `valid_rule` gültig nennt (eine Wahrheit, zwei Stellen).
    #[test]
    fn edit_check_accepts_exactly_the_valid_rules(rule in any_rule()) {
        let conn = crate::test_db::fresh();
        let list = create_list(&conn, "L", "", 1000).map_err(|e| TestCaseError::fail(e.to_string()))?;
        let value = serde_json::to_value(&rule).map_err(|e| TestCaseError::fail(e.to_string()))?;
        let mut fields = Map::new();
        fields.insert("title".into(), json!("x"));
        fields.insert("repeat_rule".into(), value);
        prop_assert_eq!(create_task(&conn, &list.id, None, &fields, 1000).is_ok(), valid_rule(&rule), "{:?}", rule);
    }
}
