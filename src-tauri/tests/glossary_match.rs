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
    // 注意：用 import_json 清洗后的数据测试（这才是实际翻译时用的）
    let cleaned = import_json(&json).expect("import_json 应成功");
    let cases = [
        "Appearance Editing",
        "Call upon the powers of the Magic Mirror from afar.",
        "Draw upon the powers of the Forgotten One.",
        "Baldur's Gate",
        "The Paladin attacks.",
        "A goblin camp.",
    ];

    let mut any_hit = false;
    for text in &cases {
        let m = find_matches(text, &cleaned);
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

    // 关键验证：清洗后不应再误匹配 from/OF/One 等噪音词
    let noise_check = find_matches("Draw upon the powers of the Forgotten One.", &cleaned);
    let noise_sources: Vec<_> = noise_check.iter().map(|m| m.source.as_str()).collect();
    assert!(
        !noise_sources.contains(&"from"),
        "清洗后不应再匹配 'from'"
    );
    assert!(
        !noise_sources.iter().any(|s| s.eq_ignore_ascii_case("of")),
        "清洗后不应再匹配 'OF'"
    );
    assert!(
        !noise_sources.contains(&"One"),
        "清洗后不应再匹配 'One'"
    );
    println!("✅ 噪音词（from/OF/One）已清除");

    // 验证 import_json 清洗后条目数（应少于原始，因过滤了噪音）
    let imported = import_json(&json).expect("import_json 应成功");
    assert!(
        imported.terms.len() < glossary.terms.len(),
        "清洗后应少于原始条目数"
    );
    println!(
        "✅ import_json 正常：原始 {}，清洗后 {}",
        glossary.terms.len(),
        imported.terms.len()
    );
}
