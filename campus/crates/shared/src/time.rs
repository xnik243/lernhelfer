//! Kalender ohne Fremdbibliothek: Tage seit 1970, ISO-Woche, "JJJJ-MM-TT".

pub const DAY_MS: i64 = 86_400_000;

pub fn days_from_civil(y: i32, m: u32, d: u32) -> i64 {
    let y = if m <= 2 { y - 1 } else { y } as i64;
    let era = y.div_euclid(400);
    let yoe = y.rem_euclid(400);
    let mp = (m as i64 + 9) % 12;
    let doy = (153 * mp + 2) / 5 + d as i64 - 1;
    era * 146_097 + yoe * 365 + yoe / 4 - yoe / 100 + doy - 719_468
}

pub fn civil_from_days(z: i64) -> (i32, u32, u32) {
    let z = z + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = (doy - (153 * mp + 2) / 5 + 1) as u32;
    let m = if mp < 10 { mp + 3 } else { mp - 9 } as u32;
    ((yoe + era * 400 + if m <= 2 { 1 } else { 0 }) as i32, m, d)
}

/// "2026-11-13" → Tage seit 1970.
pub fn parse_day(s: &str) -> Option<i64> {
    let mut it = s.split('-');
    let (y, m, d) = (it.next()?.parse().ok()?, it.next()?.parse().ok()?, it.next()?.parse().ok()?);
    ((1..=12).contains(&m) && (1..=31).contains(&d) && it.next().is_none()).then(|| days_from_civil(y, m, d))
}

/// ISO-Woche als eine Zahl: Jahr*100 + Woche (z. B. 202646).
pub fn iso_week(days: i64) -> u32 {
    let wd = (days + 3).rem_euclid(7); // 0 = Montag
    let thu = days - wd + 3;
    let (y, _, _) = civil_from_days(thu);
    let week = (thu - days_from_civil(y, 1, 1)) / 7 + 1;
    y as u32 * 100 + week as u32
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn kalender() {
        assert_eq!(days_from_civil(1970, 1, 1), 0);
        assert_eq!(civil_from_days(days_from_civil(2026, 11, 13)), (2026, 11, 13));
        assert_eq!(parse_day("2026-11-13"), Some(days_from_civil(2026, 11, 13)));
        assert_eq!(parse_day("2026-13-01"), None);
        assert_eq!(iso_week(days_from_civil(2026, 11, 13)), 202646);
        assert_eq!(iso_week(days_from_civil(2027, 1, 3)), 202653); // Sonntag gehört noch zur Woche 53 von 2026
        assert_eq!(iso_week(days_from_civil(2027, 1, 4)), 202701);
        assert_eq!(iso_week(days_from_civil(2024, 12, 30)), 202501);
    }
}
