//! 端到端集成测试：用真实 MOD 验证 解包→解析→写回→重打包 完整链路。

use std::path::PathBuf;

fn project_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .to_path_buf()
}

/// 找到测试 MOD 文件（.pak 或 .zip）。
fn find_test_mod() -> Option<PathBuf> {
    let samples = project_root().join("samples");
    if !samples.exists() {
        return None;
    }
    // 优先找 .pak，其次 .zip
    for entry in std::fs::read_dir(&samples).ok()? {
        let path = entry.ok()?.path();
        if path.extension().and_then(|e| e.to_str()) == Some("pak") {
            return Some(path);
        }
    }
    for entry in std::fs::read_dir(&samples).ok()? {
        let path = entry.ok()?.path();
        if path.extension().and_then(|e| e.to_str()) == Some("zip") {
            return Some(path);
        }
    }
    None
}

#[test]
fn end_to_end_pak_flow() {
    let Some(mod_path) = find_test_mod() else {
        eprintln!("⚠️  未找到测试 MOD，跳过端到端测试");
        return;
    };
    println!("使用测试 MOD: {}", mod_path.display());

    // 1. 解包
    let (work_dir, files) = bg3_translate_lib::pak::open_and_extract(
        mod_path.to_str().unwrap(),
    )
    .expect("解包失败");
    println!("✅ 解包成功，工作目录: {work_dir}，文件数: {}", files.len());
    assert!(!files.is_empty(), "应解出文件");

    // 2. 找一个本地化 XML 文件
    let xml_file = files.iter().find(|f| {
        matches!(
            f.kind,
            bg3_translate_lib::types::PakFileKind::LocalizationXml
        )
    });
    let Some(xml_file) = xml_file else {
        eprintln!("⚠️  MOD 中没有本地化 XML，跳过 XML 翻译验证");
        // 至少验证能重打包
        test_repack(&work_dir, &files);
        return;
    };
    println!("找到本地化 XML: {}", xml_file.name);

    // 3. 读取条目
    let entries = bg3_translate_lib::formats::read_entries(
        &work_dir,
        &xml_file.name,
        &xml_file.kind,
    )
    .expect("读取条目失败");
    println!("✅ 读取 {} 条翻译条目", entries.len());
    assert!(!entries.is_empty(), "XML 应有可翻译条目");

    // 打印前 3 条预览
    for e in entries.iter().take(3) {
        println!("   [{}] {}", e.contentuid, e.source);
    }

    // 4. 模拟翻译（把 target 设为中文）
    let mut translated = entries.clone();
    for (i, e) in translated.iter_mut().enumerate() {
        if !e.source.is_empty() {
            e.target = format!("【译文{}】{}", i + 1, e.source);
            e.status = "translated".into();
        }
    }

    // 5. 写回
    bg3_translate_lib::formats::write_entries(
        &work_dir,
        &xml_file.name,
        &xml_file.kind,
        &translated,
    )
    .expect("写回失败");
    println!("✅ 写回成功");

    // 6. 重新读取验证译文已写入文件（source 字段存的是文件当前内容，即译文）
    let re_read = bg3_translate_lib::formats::read_entries(
        &work_dir,
        &xml_file.name,
        &xml_file.kind,
    )
    .expect("重新读取失败");
    assert_eq!(re_read.len(), translated.len(), "条目数应一致");
    for (written, new) in translated.iter().zip(re_read.iter()) {
        if !written.source.is_empty() {
            assert_eq!(
                new.source, written.target,
                "重新读取的 source 应为写入的译文"
            );
            assert_eq!(
                new.contentuid, written.contentuid,
                "contentuid 必须原样保留"
            );
            assert_eq!(new.version, written.version, "version 必须原样保留");
        }
    }
    println!("✅ 译文已正确写回，contentuid/version 完整保留");

    // 7. 重打包
    test_repack(&work_dir, &files);
}

fn test_repack(work_dir: &str, files: &[bg3_translate_lib::types::PakFile]) {
    let output = format!("{work_dir}/test_output.pak");
    bg3_translate_lib::pak::repack(work_dir, &output).expect("重打包失败");
    println!("✅ 重打包成功: {output}");

    // 验证新 pak 文件存在且非空
    let meta = std::fs::metadata(&output).expect("输出文件不存在");
    assert!(meta.len() > 0, "输出 pak 不应为空");

    // 重新打开验证文件数
    let pkg = bg3rustpaklib::Package::open(&output).expect("重新打开失败");
    let new_count = pkg.files().len();
    println!("✅ 新 pak 文件数: {new_count}（原 {}）", files.len());
    assert_eq!(new_count, files.len(), "重打包后文件数应一致");
}
