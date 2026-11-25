//! Wörter der Schnelleingabe, die Tag, Uhrzeit oder Wiederholung nennen (Deutsch, wie bisher in der
//! Oberfläche). Arbeitet auf Wörtern, nicht auf Text-Schnitten; unmögliche Angaben ergeben `Err`.
use chrono::{Datelike, Days, NaiveDate, NaiveTime};
use serde_json::{json, Value};

pub struct When {
    pub date: Option<NaiveDate>,
    pub time: Option<NaiveTime>,
    pub repeat: Option<Value>,
    /// Wörter, die keine Angabe waren (der Titel).
    pub rest: Vec<String>,
}

const WEEKDAYS: [&str; 7] = ["montag", "dienstag", "mittwoch", "donnerstag", "freitag", "samstag", "sonntag"];
const MONTHS: [&str; 12] = ["jan", "feb", "mär", "apr", "mai", "jun", "jul", "aug", "sep", "okt", "nov", "dez"];
const LEAD: [&str; 10] = ["am", "nächsten", "nächste", "nächster", "um", "den", "ab", "von", "bis", "für"];
/// Füllwörter, die nur **direkt vor** einer erkannten Angabe wegfallen („Kuchen für Oma morgen“ behält „für“).
const FILL: [&str; 7] = ["am", "um", "den", "ab", "von", "bis", "für"];

fn weekday(word: &str) -> Option<u8> {
    WEEKDAYS.iter().position(|w| *w == word).and_then(|i| u8::try_from(i).ok()).map(|i| i.saturating_add(1))
}

fn month(word: &str) -> Option<u32> {
    let word = word.trim_end_matches('.');
    let short: String = word.chars().take(3).collect();
    let known = word.chars().count() >= 3 && (word == short || MONTHS.iter().any(|m| word.starts_with(m)));
    MONTHS
        .iter()
        .position(|m| *m == short)
        .filter(|_| known)
        .and_then(|i| u32::try_from(i).ok())
        .map(|i| i.saturating_add(1))
}

/// Nächster solcher Wochentag (1 = Montag), nie heute.
fn next_weekday(today: NaiveDate, wd: u8) -> Option<NaiveDate> {
    let now = today.weekday().number_from_monday();
    let ahead = (u32::from(wd).saturating_add(7).saturating_sub(now)) % 7;
    today.checked_add_days(Days::new(u64::from(if ahead == 0 { 7 } else { ahead })))
}

/// Datum ohne Jahr: das nächste Mal (heute zählt noch).
fn resolve(today: NaiveDate, year: Option<i32>, month: u32, day: u32) -> Option<NaiveDate> {
    if let Some(y) = year {
        return NaiveDate::from_ymd_opt(y, month, day);
    }
    let this = NaiveDate::from_ymd_opt(today.year(), month, day)?;
    if this >= today {
        Some(this)
    } else {
        NaiveDate::from_ymd_opt(today.year().saturating_add(1), month, day)
    }
}

fn number(s: &str, lens: &[usize]) -> Option<u32> {
    (lens.contains(&s.len()) && s.bytes().all(|b| b.is_ascii_digit())).then(|| s.parse().ok()).flatten()
}

/// `3.12.`, `24.12.2026`, `1.3.27` → (Tag, Monat, Jahr). `None` = kein Datum in dieser Form.
fn dotted(word: &str) -> Option<(u32, u32, Option<i32>)> {
    let parts: Vec<&str> = word.split('.').collect();
    let (d, m, y) = match parts.as_slice() {
        [d, m, ""] => (*d, *m, None),
        [d, m, y] if !y.is_empty() => (*d, *m, Some(*y)),
        [d, m, y, ""] if !y.is_empty() => (*d, *m, Some(*y)),
        _ => return None,
    };
    let year = match y {
        Some(y) => {
            Some(i32::try_from(number(y, &[2, 4])?).ok()?)
                .map(|v| if y.len() == 2 { v.saturating_add(2000) } else { v })
        }
        None => None,
    };
    Some((number(d, &[1, 2])?, number(m, &[1, 2])?, year))
}

fn clock(word: &str) -> Option<(u32, u32)> {
    let (h, m) = word.split_once(':')?;
    Some((number(h, &[1, 2])?, number(m, &[2])?))
}

struct Words {
    lower: Vec<String>,
    used: Vec<bool>,
}

impl Words {
    fn at(&self, i: usize) -> &str {
        self.lower.get(i).filter(|_| !self.used.get(i).copied().unwrap_or(true)).map_or("", String::as_str)
    }
    fn take(&mut self, i: usize) {
        if let Some(u) = self.used.get_mut(i) {
            *u = true;
        }
    }
    fn find(&self, pred: impl Fn(&str, &str) -> bool) -> Option<usize> {
        (0..self.lower.len()).find(|&i| !self.at(i).is_empty() && pred(self.at(i), self.at(i.saturating_add(1))))
    }
    /// Nimmt Füllwörter wie „am“ direkt vor `i` mit.
    fn take_lead(&mut self, mut i: usize, lead: &[&str]) {
        while let Some(prev) = i.checked_sub(1).filter(|&p| lead.contains(&self.at(p))) {
            self.take(prev);
            i = prev;
        }
    }
}

fn repeat(w: &mut Words, today: NaiveDate, date: &mut Option<NaiveDate>) -> Option<Value> {
    if let Some(i) = w.find(|a, b| matches!(a, "jeden" | "jeder") && weekday(b).is_some()) {
        let wd = weekday(w.at(i.saturating_add(1)))?;
        w.take(i);
        w.take(i.saturating_add(1));
        *date = next_weekday(today, wd);
        return Some(json!({ "kind": "weekly", "interval": 1, "weekdays": [wd] }));
    }
    let pairs = [
        ("jeden", "tag", "daily"),
        ("jede", "woche", "weekly"),
        ("jeden", "monat", "monthly"),
        ("jedes", "jahr", "yearly"),
    ];
    for (a, b, kind) in pairs {
        if let Some(i) = w.find(|x, y| x == a && y == b) {
            w.take(i);
            w.take(i.saturating_add(1));
            return Some(json!({ "kind": kind, "interval": 1 }));
        }
    }
    let single = [("täglich", "daily"), ("wöchentlich", "weekly"), ("monatlich", "monthly"), ("jährlich", "yearly")];
    let (i, kind) = single.iter().find_map(|(word, kind)| w.find(|x, _| x == *word).map(|i| (i, *kind)))?;
    w.take(i);
    Some(json!({ "kind": kind, "interval": 1 }))
}

fn day(w: &mut Words, today: NaiveDate) -> Result<Option<NaiveDate>, ()> {
    if let Some(i) = w.find(|a, _| dotted(a).is_some()) {
        let (d, m, y) = dotted(w.at(i)).ok_or(())?;
        w.take(i);
        w.take_lead(i, &FILL);
        return resolve(today, y, m, d).map(Some).ok_or(());
    }
    if let Some(i) =
        w.find(|a, b| a.ends_with('.') && number(a.trim_end_matches('.'), &[1, 2]).is_some() && month(b).is_some())
    {
        let d = number(w.at(i).trim_end_matches('.'), &[1, 2]).ok_or(())?;
        let m = month(w.at(i.saturating_add(1))).ok_or(())?;
        let year = number(w.at(i.saturating_add(2)), &[4]).and_then(|y| i32::try_from(y).ok());
        for k in 0..if year.is_some() { 3 } else { 2 } {
            w.take(i.saturating_add(k));
        }
        w.take_lead(i, &FILL);
        return resolve(today, year, m, d).map(Some).ok_or(());
    }
    let rel = [("heute", 0u64), ("morgen", 1), ("übermorgen", 2)];
    if let Some((i, n)) = rel.iter().find_map(|(word, n)| w.find(|x, _| x == *word).map(|i| (i, *n))) {
        w.take(i);
        w.take_lead(i, &FILL);
        return Ok(today.checked_add_days(Days::new(n)));
    }
    if let Some(i) = w.find(|a, _| weekday(a).is_some()) {
        let wd = weekday(w.at(i)).ok_or(())?;
        w.take(i);
        w.take_lead(i, &LEAD);
        return Ok(next_weekday(today, wd));
    }
    Ok(None)
}

fn time(w: &mut Words) -> Result<Option<NaiveTime>, ()> {
    let (i, (h, m), len) = if let Some(i) = w.find(|a, _| clock(a).is_some()) {
        let len = if w.at(i.saturating_add(1)) == "uhr" { 2 } else { 1 };
        (i, clock(w.at(i)).ok_or(())?, len)
    } else if let Some(i) = w.find(|a, b| number(a, &[1, 2]).is_some() && b == "uhr") {
        (i, (number(w.at(i), &[1, 2]).ok_or(())?, 0), 2)
    } else {
        return Ok(None);
    };
    for k in 0..len {
        w.take(i.saturating_add(k));
    }
    w.take_lead(i, &FILL);
    NaiveTime::from_hms_opt(h, m, 0).map(Some).ok_or(())
}

/// Zerlegt die Wörter. `Err` = eine Angabe ist unmöglich (31.2., 25 Uhr); dann bleibt alles Titel.
pub(crate) fn parse(words: &[String], today: NaiveDate) -> Result<When, ()> {
    let lower: Vec<String> = words.iter().map(|w| w.to_lowercase()).collect();
    let mut w = Words { used: vec![false; lower.len()], lower };
    let mut date = None;
    let repeat = repeat(&mut w, today, &mut date);
    if date.is_none() {
        date = day(&mut w, today)?;
    }
    let time = time(&mut w)?;
    // Füllwörter fallen nur direkt vor einer Angabe weg (`take_lead`); im übrigen Titel bleiben sie (Review M6a N-1).
    let rest = words
        .iter()
        .enumerate()
        .filter(|(i, _)| !w.used.get(*i).copied().unwrap_or(true))
        .map(|(_, s)| s.clone())
        .collect();
    Ok(When { date, time, repeat, rest })
}

/// Wie die Zerlegung der Schnelleingabe, für andere Eingaben (Desk: Termine). `None` = eine Angabe ist unmöglich.
pub fn parse_words(words: &[String], today: NaiveDate) -> Option<When> {
    parse(words, today).ok()
}
