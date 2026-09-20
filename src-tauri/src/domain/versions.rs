use std::cmp::Ordering;

pub fn compare_versions(a: &str, b: &str) -> Ordering {
    compare_with(a, b, '-')
}

fn compare_with(a: &str, b: &str, delimiter: char) -> Ordering {
    let a = a.replace('+', "-");
    let b = b.replace('+', "-");
    if a == b {
        return Ordering::Equal;
    }
    let ap = split_raw(&a, delimiter);
    let bp = split_raw(&b, delimiter);
    if ap.first().map(|s| s.eq_ignore_ascii_case("nightly")).unwrap_or(false)
        && bp.first().map(|s| s.eq_ignore_ascii_case("nightly")).unwrap_or(false)
    {
        return Ordering::Equal;
    }
    for i in 0..ap.len().max(bp.len()) {
        match (ap.get(i), bp.get(i)) {
            (None, Some(v)) => {
                return if is_prerelease(v) { Ordering::Greater } else { Ordering::Less };
            }
            (Some(v), None) => {
                return if is_prerelease(v) { Ordering::Less } else { Ordering::Greater };
            }
            (Some(x), Some(y)) => {
                if x.contains('.') || y.contains('.') {
                    let r = compare_with(x, y, '.');
                    if r != Ordering::Equal {
                        return r;
                    }
                    continue;
                }
                if x.contains('_') || y.contains('_') {
                    let r = compare_with(x, y, '_');
                    if r != Ordering::Equal {
                        return r;
                    }
                    continue;
                }
                let r = compare_part(x, y);
                if r != Ordering::Equal {
                    return r;
                }
            }
            (None, None) => unreachable!(),
        }
    }
    Ordering::Equal
}

fn split_raw(version: &str, delimiter: char) -> Vec<String> {
    let mut wrapped = String::with_capacity(version.len() + 8);
    let mut in_alpha = false;
    for ch in version.chars() {
        if ch.is_ascii_alphabetic() {
            if !in_alpha {
                wrapped.push(delimiter);
                in_alpha = true;
            }
            wrapped.push(ch);
        } else {
            if in_alpha {
                wrapped.push(delimiter);
                in_alpha = false;
            }
            wrapped.push(ch);
        }
    }
    wrapped
        .split(delimiter)
        .filter(|s| !s.is_empty())
        .map(|s| s.to_string())
        .collect()
}

fn parse_num(s: &str) -> Option<u64> {
    if !s.is_empty() && s.chars().all(|c| c.is_ascii_digit()) {
        s.parse().ok()
    } else {
        None
    }
}

fn compare_part(x: &str, y: &str) -> Ordering {
    match (parse_num(x), parse_num(y)) {
        (Some(a), Some(b)) => a.cmp(&b),
        (Some(a), None) => a.to_string().to_ascii_lowercase().cmp(&y.to_ascii_lowercase()),
        (None, Some(b)) => x.to_ascii_lowercase().cmp(&b.to_string().to_ascii_lowercase()),
        (None, None) => x.to_ascii_lowercase().cmp(&y.to_ascii_lowercase()),
    }
}

fn is_prerelease(part: &str) -> bool {
    let lower = part.to_ascii_lowercase();
    ["alpha", "beta", "rc", "pre"].iter().any(|k| lower.contains(k))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cmp::Ordering;

    #[test]
    fn equal_versions() {
        assert_eq!(compare_versions("1.8.2", "1.8.2"), Ordering::Equal);
        assert_eq!(compare_versions("2026-06", "2026-06"), Ordering::Equal);
        assert_eq!(compare_versions("1.0.0+build1", "1.0.0-build1"), Ordering::Equal);
        assert_eq!(compare_versions("nightly-20240101", "nightly-20240102"), Ordering::Equal);
    }

    #[test]
    fn numeric_segments_compare_numerically() {
        assert_eq!(compare_versions("1.8.2", "1.8.10"), Ordering::Less);
        assert_eq!(compare_versions("26.00", "26.03"), Ordering::Less);
        assert_eq!(compare_versions("2.53.0.2", "2.55.0.5"), Ordering::Less);
        assert_eq!(compare_versions("17.0.18-8", "17.0.20-101"), Ordering::Less);
        assert_eq!(compare_versions("1.8.10", "1.8.2"), Ordering::Greater);
    }

    #[test]
    fn prerelease_is_less_than_release() {
        assert_eq!(compare_versions("1.1-alpha", "1.1"), Ordering::Less);
        assert_eq!(compare_versions("1.1", "1.1-alpha"), Ordering::Greater);
        assert_eq!(compare_versions("1.0-beta2", "1.0"), Ordering::Less);
    }

    #[test]
    fn underscore_and_extra_segments() {
        assert_eq!(compare_versions("1.0", "1.0.0"), Ordering::Less);
        assert_eq!(compare_versions("1.0.0", "1.0"), Ordering::Greater);
        assert_eq!(compare_versions("1.0_1", "1.0_2"), Ordering::Less);
    }

    #[test]
    fn split_boundaries_and_case_folding() {
        assert_eq!(compare_versions("1.0", "1.0a"), Ordering::Less);
        assert_eq!(compare_versions("1.0A", "1.0a"), Ordering::Equal);
        assert_eq!(compare_versions("1.0.0-1a", "1.0.0-1"), Ordering::Greater);
        assert_eq!(compare_versions("1.0.0+build2", "1.0.0-build3"), Ordering::Less);
    }
}
