use scoopui_lib::domain::mirrors;

#[test]
#[ignore = "需要真实网络；用 cargo test --test proxy_probe -- --ignored 运行"]
fn probes_ip_service() {
    let (status, body) = mirrors::http_get_text("https://ip.glimmer.ltd/0").expect("应能访问");
    assert!(status == 200, "状态码 {status}");
    assert!(!body.trim().is_empty());
}
