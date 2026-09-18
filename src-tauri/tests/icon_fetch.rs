use scoopui_lib::domain::icons;

#[test]
#[ignore = "需要真实网络；用 cargo test --test icon_fetch -- --ignored 运行"]
fn fetches_real_favicon() {
    let tmp = tempfile::tempdir().unwrap();
    let out = icons::resolve_icon("https://www.python.org", tmp.path(), None);
    assert!(out.as_deref().unwrap_or("").starts_with("data:image/"));
    let cached = icons::load_icon(tmp.path(), "https://www.python.org");
    assert!(cached.is_some());
}
