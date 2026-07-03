#[test]
fn print_all_serialization() {
    use bg3_translate_lib::types::*;
    let e = TranslationEntry {
        id: "f#h1".into(), source_file: "f.xml".into(), source: "Hi".into(),
        target: "你好".into(), contentuid: "h1".into(), version: "1".into(),
        status: "pending".into(), error: None,
    };
    println!("ENTRY_JSON: {}", serde_json::to_string(&e).unwrap());

    // TranslationEvent —— 关键：字段必须是 camelCase（entryId），与前端对齐
    let ev = TranslationEvent::Progress { entry_id: "x1".into(), status: TranslationStatus::Translating };
    let j = serde_json::to_string(&ev).unwrap();
    println!("EVENT_JSON: {j}");
    assert!(j.contains("\"entryId\""), "event 必须用 camelCase entryId, 实际: {j}");
    assert!(j.contains("\"type\":\"progress\""), "type 必须是 progress, 实际: {j}");

    let d = TranslationEvent::Delta { entry_id: "x2".into(), text: "你好".into() };
    let dj = serde_json::to_string(&d).unwrap();
    assert!(dj.contains("\"entryId\""), "delta 必须用 entryId, 实际: {dj}");

    let done = TranslationEvent::Done { entry_id: "x3".into(), text: "完成".into() };
    let dj2 = serde_json::to_string(&done).unwrap();
    assert!(dj2.contains("\"entryId\""), "done 必须用 entryId, 实际: {dj2}");
    println!("ALL_EVENT_CHECKS_PASSED");
}
