//! Hybride logische Uhr (HLC) für den feldweisen Abgleich. Die Textform ist so gebaut, dass der
//! Zeichenkettenvergleich der Zeitordnung entspricht: `<ms hex 12>-<zähler hex 4>-<gerät>`.
use std::cmp::Ordering;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Hlc {
    pub ms: u64,
    pub counter: u16,
    pub device: String,
}

impl Hlc {
    pub fn parse(s: &str) -> Option<Hlc> {
        let mut parts = s.splitn(3, '-');
        let ms = u64::from_str_radix(parts.next()?, 16).ok()?;
        let counter = u16::from_str_radix(parts.next()?, 16).ok()?;
        let device = parts.next()?.to_string();
        if device.is_empty() {
            return None;
        }
        Some(Hlc { ms, counter, device })
    }
}

impl std::fmt::Display for Hlc {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{:012x}-{:04x}-{}", self.ms, self.counter, self.device)
    }
}

impl Ord for Hlc {
    fn cmp(&self, o: &Self) -> Ordering {
        (self.ms, self.counter, &self.device).cmp(&(o.ms, o.counter, &o.device))
    }
}
impl PartialOrd for Hlc {
    fn partial_cmp(&self, o: &Self) -> Option<Ordering> {
        Some(self.cmp(o))
    }
}

/// Uhr eines Geräts. Liefert streng aufsteigende Stempel, auch wenn die Systemzeit stehen bleibt oder zurückspringt.
pub struct Clock {
    device: String,
    last: Hlc,
}

impl Clock {
    pub fn new(device: &str) -> Clock {
        Clock { device: device.to_string(), last: Hlc { ms: 0, counter: 0, device: device.to_string() } }
    }

    /// Stempel für eine lokale Änderung. `wall_ms` = Systemzeit in Millisekunden.
    pub fn tick(&mut self, wall_ms: u64) -> Hlc {
        if wall_ms > self.last.ms {
            self.last = Hlc { ms: wall_ms, counter: 0, device: self.device.clone() };
        } else {
            self.last.counter = self.last.counter.saturating_add(1);
        }
        self.last.clone()
    }

    /// Letzter vergebener oder beobachteter Stand.
    pub fn last(&self) -> &Hlc {
        &self.last
    }

    /// Fremden Stempel einrechnen, damit spätere eigene Stempel größer sind.
    pub fn observe(&mut self, remote: &Hlc) {
        if remote.ms > self.last.ms || (remote.ms == self.last.ms && remote.counter > self.last.counter) {
            self.last = Hlc { ms: remote.ms, counter: remote.counter, device: self.device.clone() };
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn text_form_round_trips_and_sorts_like_time() {
        let a = Hlc { ms: 255, counter: 1, device: "a".into() };
        assert_eq!(a.to_string(), "0000000000ff-0001-a");
        assert_eq!(Hlc::parse(&a.to_string()), Some(a.clone()));
        let b = Hlc { ms: 4096, counter: 0, device: "a".into() };
        assert!(a < b && a.to_string() < b.to_string());
        let c = Hlc { ms: 255, counter: 2, device: "a".into() };
        assert!(a.to_string() < c.to_string());
    }

    #[test]
    fn device_name_with_dashes_survives() {
        let h = Hlc::parse("000000000001-0000-pc-von-anna").unwrap();
        assert_eq!(h.device, "pc-von-anna");
    }

    #[test]
    fn garbage_is_rejected() {
        assert!(Hlc::parse("").is_none());
        assert!(Hlc::parse("zz-0000-a").is_none());
        assert!(Hlc::parse("0001-0000-").is_none());
    }

    #[test]
    fn tick_is_strictly_increasing_even_if_the_wall_clock_stands_or_goes_back() {
        let mut c = Clock::new("a");
        let t1 = c.tick(1000);
        let t2 = c.tick(1000);
        let t3 = c.tick(500);
        assert!(t1 < t2 && t2 < t3);
        assert!(c.tick(2000) > t3);
    }

    #[test]
    fn observing_a_remote_stamp_moves_the_clock_ahead() {
        let mut c = Clock::new("a");
        let remote = Hlc { ms: 9000, counter: 3, device: "b".into() };
        c.observe(&remote);
        assert!(c.tick(1000) > remote);
    }
}
