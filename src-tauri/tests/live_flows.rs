use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use scoopui_lib::domain::{
    buckets, configfile, index, installed, migrate, profiles, registry, root, scoopcmd, snapshots,
};

fn home() -> Option<PathBuf> {
    std::env::var_os("USERPROFILE").map(PathBuf::from)
}

fn local_root() -> Option<PathBuf> {
    let p = home()?.join("scoop");
    if root::validate(&p).is_ok() {
        Some(p)
    } else {
        None
    }
}

fn global_root() -> Option<PathBuf> {
    let p = PathBuf::from(std::env::var_os("ProgramData")?).join("scoop");
    if p.is_dir() {
        Some(p)
    } else {
        None
    }
}

fn run_scoop(root_path: &Path, args: &[&str]) -> Option<i32> {
    let args: Vec<String> = args.iter().map(|s| s.to_string()).collect();
    let cmd = scoopcmd::build(root_path, None, &args).ok()?;
    println!("$ scoop {}", args.join(" "));
    let child = scoopcmd::spawn(&cmd).ok()?;
    let lines = Arc::new(Mutex::new(Vec::<String>::new()));
    let sink = lines.clone();
    let code = scoopcmd::stream_child(child, move |l| sink.lock().unwrap().push(l.line)).ok()?;
    for line in lines.lock().unwrap().iter().take(20) {
        println!("  | {line}");
    }
    code
}

#[test]
#[ignore = "真实环境：7zip 快照-卸载-恢复流程；cargo test --test live_flows -- --ignored --nocapture"]
fn live_7zip_snapshot_uninstall_restore_flow() {
    let Some(rp) = local_root() else {
        eprintln!("skip: 本机无有效 scoop 根目录");
        return;
    };
    let apps = installed::scan(&rp.join("apps"), false);
    assert!(
        apps.iter().any(|a| a.name == "7zip"),
        "需要用户级 7zip 已安装"
    );
    let tmp = tempfile::tempdir().unwrap();
    let snap_dir = tmp.path().join("snapshots");
    let cfg = configfile::resolve(&rp, &home().unwrap().join(".config"));
    let bucket_list = buckets::scan(&rp.join("buckets"));

    let summary = snapshots::create(
        &snap_dir,
        &rp,
        &cfg.path,
        &bucket_list,
        &apps,
        &tmp.path().join("profiles"),
    )
    .unwrap();
    println!(
        "快照已创建: id={} 应用={} 软件源={}",
        summary.id, summary.app_count, summary.bucket_count
    );

    let code = run_scoop(&rp, &["uninstall", "7zip"]);
    assert_eq!(code, Some(0), "卸载 7zip 应成功");
    let after = installed::scan(&rp.join("apps"), false);
    assert!(!after.iter().any(|a| a.name == "7zip"), "7zip 应已卸载");

    let plan = snapshots::plan(
        &snap_dir,
        &summary.id,
        &rp,
        &cfg.path,
        &buckets::scan(&rp.join("buckets")),
        &after,
    )
    .unwrap();
    println!(
        "恢复计划缺失应用: {:?}",
        plan.missing_apps.iter().map(|a| &a.name).collect::<Vec<_>>()
    );
    assert!(
        plan.missing_apps.iter().any(|a| a.name == "7zip"),
        "计划应包含缺失的 7zip"
    );

    let code = run_scoop(&rp, &["install", "7zip"]);
    assert_eq!(code, Some(0), "重装 7zip 应成功");
    let restored = installed::scan(&rp.join("apps"), false);
    assert!(restored.iter().any(|a| a.name == "7zip"), "7zip 应已恢复");

    let plan2 = snapshots::plan(
        &snap_dir,
        &summary.id,
        &rp,
        &cfg.path,
        &buckets::scan(&rp.join("buckets")),
        &restored,
    )
    .unwrap();
    assert!(
        !plan2.missing_apps.iter().any(|a| a.name == "7zip"),
        "恢复后计划中不应再有缺失的 7zip"
    );
    snapshots::delete(&snap_dir, &summary.id).unwrap();
    println!("✅ 快照-卸载-恢复流程通过");
}

#[test]
#[ignore = "真实环境：导入桌面 scoopfile-test.json；cargo test --test live_flows -- --ignored --nocapture"]
fn live_import_desktop_scoopfile() {
    let path = home().unwrap().join("Desktop").join("scoopfile-test.json");
    let Ok(text) = std::fs::read_to_string(&path) else {
        eprintln!("skip: 文件不存在 {}", path.display());
        return;
    };
    let p = profiles::parse_import(&text, "导入测试").unwrap();
    println!(
        "软件源={:?} 应用={:?} 设置={:?}",
        p.buckets.iter().map(|b| &b.name).collect::<Vec<_>>(),
        p.apps.iter().map(|a| &a.name).collect::<Vec<_>>(),
        p.settings.keys().collect::<Vec<_>>()
    );
    assert_eq!(p.name, "导入测试");
    assert!(p.buckets.iter().any(|b| b.name == "extras"));
    assert!(p.apps.iter().any(|a| a.name == "git"));
    assert_eq!(
        p.settings.get("url_proxy").and_then(|v| v.as_str()),
        Some("https://scoop.201704.xyz")
    );
    println!("✅ 导入解析通过");
}

#[test]
#[ignore = "真实环境：注册表扫描与匹配；cargo test --test live_flows -- --ignored --nocapture"]
fn live_registry_scan_and_match() {
    let Some(rp) = local_root() else {
        eprintln!("skip: 本机无有效 scoop 根目录");
        return;
    };
    let mut roots = vec![rp.clone()];
    if let Some(g) = global_root() {
        roots.push(g);
    }
    let apps = registry::scan_registry(&roots);
    let managed = apps.iter().filter(|a| a.managed).count();
    println!("注册表条目 {} 条，其中已由 scoop 管理 {}", apps.len(), managed);
    for a in apps.iter().take(15) {
        println!("  - {} {:?} managed={}", a.name, a.version, a.managed);
    }
    assert!(!apps.is_empty(), "真实注册表应有可扫描条目");
    let git_entries: Vec<&str> = apps
        .iter()
        .filter(|a| a.name.eq_ignore_ascii_case("git"))
        .map(|a| a.name.as_str())
        .collect();
    assert!(git_entries.is_empty(), "git 已改为 scoop 全局安装，注册表不应再有 Git 条目");

    let entries = index::build(&rp.join("buckets"));
    let matched = migrate::match_apps(&apps, &entries);
    let with_matches = matched.iter().filter(|m| !m.matches.is_empty()).count();
    println!("索引 {} 个包，注册表条目中匹配到包的有 {} 条", entries.len(), with_matches);
    println!("✅ 注册表扫描与匹配通过");
}
