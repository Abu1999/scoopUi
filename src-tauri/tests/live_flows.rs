use std::path::PathBuf;

use scoopui_lib::domain::{bootstrap, index, migrate, postinstall, profiles, registry, root};

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

#[test]
#[ignore = "真实环境：注册 git 安装钩子（右键菜单/文件关联，幂等）；cargo test --test live_flows -- --ignored --nocapture"]
fn live_register_git_install_hooks() {
    let dir = global_root()
        .map(|g| g.join("apps").join("git").join("current"))
        .filter(|p| p.is_dir())
        .or_else(|| {
            local_root()
                .map(|r| r.join("apps").join("git").join("current"))
                .filter(|p| p.is_dir())
        });
    let Some(dir) = dir else {
        eprintln!("skip: 未找到 git 安装目录");
        return;
    };
    let done = postinstall::register(&dir, "git").unwrap();
    println!("已注册: {done:?}");
    assert!(done.iter().any(|d| d.contains("context")), "应导入 install-context.reg");
    println!("✅ git 安装钩子注册通过");
}

#[test]
#[ignore = "真实环境：内置快照解压与校验；cargo test --test live_flows -- --ignored --nocapture"]
fn live_bootstrap_from_bundle() {
    let dir = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("resources");
    let zip = dir.join("scoop-bundle.zip");
    assert!(zip.is_file(), "缺少内置快照资源");
    let info = bootstrap::read_bundle_info(&dir.join("scoop-bundle.json")).unwrap();
    println!(
        "bundle: built={} scoop={} main={}",
        info.built,
        &info.scoop_commit[..7],
        &info.main_commit[..7]
    );
    let tmp = tempfile::tempdir().unwrap();
    let target = tmp.path().join("scoop");
    let staging = bootstrap::staging_dir(&target);
    let probes = bootstrap::probe();
    let (cmd, _script) =
        bootstrap::extract_command(&zip, &staging, probes.tar_available, tmp.path()).unwrap();
    let status = std::process::Command::new(&cmd.program)
        .args(&cmd.args)
        .status()
        .unwrap();
    assert!(status.success(), "解压失败");
    let count = bootstrap::finalize(&target, &staging).unwrap();
    println!("main manifests = {count}");
    assert!(count > 1000, "main 清单数异常: {count}");
    root::validate(&target).unwrap();
    assert!(target
        .join("apps")
        .join("scoop")
        .join("current")
        .join("bin")
        .join("scoop.ps1")
        .is_file());
    assert!(target.join("apps").join("scoop").join("current").join(".git").is_dir());
    println!("✅ 内置快照初始化通过");
}
