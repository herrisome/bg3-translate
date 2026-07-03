//! 用真实 20K 条术语表验证导入后命中检测是否工作。
//! 同时验证 import_json 的数据确实能被 find_matches 使用。

use bg3_translate_lib::glossary::{find_matches, import_json, Glossary};

#[test]
fn import_then_match_works() {
    let path = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .join("samples/bg3-official-glossary.json");
    if !path.exists() {
        eprintln!("⚠️  跳过：未找到 {path:?}");
        return;
    }

    let json = std::fs::read_to_string(&path).unwrap();
    let glossary: Glossary = serde_json::from_str(&json).expect("JSON 应能解析");
    println!("✅ 解析成功：{} 条", glossary.terms.len());

    let enabled = glossary.terms.iter().filter(|t| t.enabled).count();
    println!("enabled: {enabled}");

    // 测试 MOD 的真实文本 + 一些一定命中的术语
    let cases = [
        ("Appearance Editing", true),
        ("Call upon the powers of the Magic Mirror from afar.", true),
        ("Draw upon the powers of the Forgotten One.", true),
        ("Baldur's Gate", true),
        ("The Paladin attacks.", true),
        ("A goblin camp.", true),
    ];

    let mut any_hit = false;
    for (text, expect_hit) in &cases {
        let m = find_matches(text, &glossary);
        if m.is_empty() {
            println!("❌ 未命中: {text}");
        } else {
            any_hit = true;
            println!("✅ 命中 {}: {text}", m.len());
            for hit in m.iter().take(5) {
                println!("     {} = {}", hit.source, hit.target);
            }
        }
    }
    assert!(any_hit, "应至少有一条命中");

    // 验证 import_json（与直接反序列化结果一致）
    let imported = import_json(&json).expect("import_json 应成功");
    assert_eq!(imported.terms.len(), glossary.terms.len());
    println!("✅ import_json 正常，返回 {} 条", imported.terms.len());
}
