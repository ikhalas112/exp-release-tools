/// Compare "1.2.3" vs "1.2.0" → -1 | 0 | 1
pub fn cmp_semver(a: &str, b: &str) -> i32 {
    let pa: Vec<i32> = a.split('.').map(|n| n.parse().unwrap_or(0)).collect();
    let pb: Vec<i32> = b.split('.').map(|n| n.parse().unwrap_or(0)).collect();
    for i in 0..3 {
        let x = pa.get(i).copied().unwrap_or(0);
        let y = pb.get(i).copied().unwrap_or(0);
        if x != y {
            return if x > y { 1 } else { -1 };
        }
    }
    0
}

/// Compare (version, build_number) tuples → -1 | 0 | 1
pub fn cmp_release(
    a_ver: &str,
    a_build: Option<&str>,
    b_ver: &str,
    b_build: Option<&str>,
) -> i32 {
    let v = cmp_semver(a_ver, b_ver);
    if v != 0 {
        return v;
    }
    let ab = a_build
        .filter(|s| !s.is_empty())
        .and_then(|s| s.parse::<i32>().ok())
        .unwrap_or(0);
    let bb = b_build
        .filter(|s| !s.is_empty())
        .and_then(|s| s.parse::<i32>().ok())
        .unwrap_or(0);
    if ab == bb {
        0
    } else if ab > bb {
        1
    } else {
        -1
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cmp_semver_orders() {
        assert_eq!(cmp_semver("1.2.0", "1.1.5"), 1);
        assert_eq!(cmp_semver("1.1.0", "1.2.0"), -1);
        assert_eq!(cmp_semver("1.1.0", "1.1.0"), 0);
        assert_eq!(cmp_semver("2.0.0", "1.9.9"), 1);
    }

    #[test]
    fn cmp_release_version_wins() {
        assert_eq!(cmp_release("1.2.0", Some("1"), "1.1.0", Some("9")), 1);
    }

    #[test]
    fn cmp_release_build_number() {
        assert_eq!(cmp_release("1.1.0", Some("2"), "1.1.0", Some("1")), 1);
        assert_eq!(cmp_release("1.1.0", None, "1.1.0", Some("2")), -1);
        assert_eq!(cmp_release("1.1.0", Some("1"), "1.1.0", None), 1);
        assert_eq!(cmp_release("1.1.0", None, "1.1.0", None), 0);
    }

    #[test]
    fn guard_rule() {
        assert!(cmp_release("1.0.5", None, "1.1.0", None) < 0);
        assert!(cmp_release("1.2.0", None, "1.1.0", None) >= 0);
    }
}
