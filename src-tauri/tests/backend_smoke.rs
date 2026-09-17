use std::path::PathBuf;

use scoopui_lib::domain::{buckets, configfile, installed, root, scoopcmd};

fn local_root() -> Option<PathBuf> {
    let home = std::env::var_os("USERPROFILE")?;
    let root = PathBuf::from(home).join("scoop");
    if root::validate(&root).is_ok() {
        Some(root)
    } else {
        None
    }
}

#[test]
fn scans_real_environment() {
    let Some(root_path) = local_root() else {
        eprintln!("skip: 本机无有效 scoop 根目录");
        return;
    };
    let apps = installed::scan(&root_path.join("apps"));
    println!("app_count={}", apps.len());
    assert!(!apps.is_empty(), "应至少扫出一个已装应用");
    assert!(apps.iter().all(|a| !a.version.is_empty()));
    assert!(apps.iter().all(|a| a.name != "scoop"), "scoop 自身不应计入");

    let buckets = buckets::scan(&root_path.join("buckets"));
    assert!(!buckets.is_empty());
    assert!(buckets.iter().any(|b| b.manifest_count > 0));

    let cfg = configfile::resolve(&root_path, &PathBuf::from(std::env::var_os("USERPROFILE").unwrap()).join(".config"));
    let values = configfile::load(&cfg.path);
    println!("config={} portable={}", cfg.path.display(), cfg.portable);
    assert!(values.is_object());
}

#[test]
fn runs_real_scoop_version() {
    let Some(root_path) = local_root() else {
        eprintln!("skip: 本机无有效 scoop 根目录");
        return;
    };
    let cmd = scoopcmd::build(&root_path, None, &["--version".to_string()]).unwrap();
    let child = scoopcmd::spawn(&cmd).unwrap();
    let mut lines: Vec<String> = Vec::new();
    let code = scoopcmd::stream_child(child, |l| lines.push(l.line)).unwrap();
    println!("exit={:?} lines={}", code, lines.len());
    assert_eq!(code, Some(0));
    assert!(lines.iter().any(|l| l.to_lowercase().contains("scoop")));
}
