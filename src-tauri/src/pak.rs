//! PAK 文件解包/打包与文件类型识别。

use std::collections::BTreeMap;
use std::path::{Component, Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use bg3rustpaklib::loca::detect_language_from_path;
use bg3rustpaklib::{get_package_priority, Package, PackageBuilder, PackagedFile};

use crate::error::{AppError, Result};
use crate::types::{PakFile, PakFileKind};

/// 由文件路径识别其分类。识别规则按可翻译性优先级排列。
pub fn classify_file(name: &str) -> PakFileKind {
    let lower = name.to_lowercase();
    // 本地化 XML：路径包含 Localization/ 且后缀 .xml
    if lower.ends_with(".xml") && lower.contains("localization") {
        PakFileKind::LocalizationXml
    } else if lower.ends_with(".loca") {
        PakFileKind::LocalizationLoca
    } else if lower.ends_with(".lsx") {
        PakFileKind::MetadataLsx
    } else if lower.ends_with(".lua") {
        PakFileKind::ScriptLua
    } else if lower.ends_with(".txt") {
        PakFileKind::DataTxt
    } else {
        PakFileKind::Other
    }
}

/// 构造传给前端的文件列表，附带语言信息。
pub fn build_pak_files(pkg: &Package) -> Vec<PakFile> {
    let mut out: Vec<PakFile> = pkg.files().iter().map(to_pak_file).collect();
    out.sort_by(|a, b| a.name.cmp(&b.name));
    out
}

/// 打开 MOD 文件（.pak 或 .zip），解包到临时工作目录，返回文件列表。
///
/// - .pak：直接作为 LSPK 归档解包
/// - .zip：Nexus 标准打包，内部含 .pak，先解 zip 再解 pak
pub fn open_and_extract(file_path: &str) -> Result<(String, Vec<PakFile>)> {
    let path = Path::new(file_path);
    if !path.exists() {
        return Err(AppError::Config(format!("文件不存在: {file_path}")));
    }

    let work_dir = create_work_dir()?;

    // 获取实际的 .pak 文件路径（zip 则先解出 .pak）
    let (pak_path, _extracted_zip_dir) = if file_path.to_lowercase().ends_with(".zip") {
        extract_zip_to_find_pak(file_path, &work_dir)?
    } else {
        (PathBuf::from(file_path), None)
    };

    // 解包 PAK
    let pkg = Package::open(&pak_path).map_err(|e| AppError::Pak(format!("打开 PAK 失败: {e}")))?;
    let extract_dir = Path::new(&work_dir).join("unpacked");
    std::fs::create_dir_all(&extract_dir)?;
    let files = extract_package_files(&pkg, &extract_dir)?;

    // 记录 pak 元信息（priority）用于重打包
    let priority: u8 = get_package_priority(&pak_path).unwrap_or(0);
    let meta_path = Path::new(&work_dir).join("pak_meta.json");
    let meta = serde_json::json!({ "priority": priority });
    std::fs::write(&meta_path, meta.to_string())?;

    Ok((work_dir, files))
}

/// 仅解压 MOD 文件到用户指定目录，不创建翻译工作流。
pub fn extract_to_directory(file_path: &str, output_dir: &str) -> Result<Vec<PakFile>> {
    let path = Path::new(file_path);
    if !path.exists() {
        return Err(AppError::Config(format!("文件不存在: {file_path}")));
    }

    let output_dir = Path::new(output_dir);
    std::fs::create_dir_all(output_dir)?;

    let work_dir = create_work_dir()?;
    let (pak_path, _extracted_zip_dir) = if file_path.to_lowercase().ends_with(".zip") {
        extract_zip_to_find_pak(file_path, &work_dir)?
    } else {
        (PathBuf::from(file_path), None)
    };

    let pkg = Package::open(&pak_path).map_err(|e| AppError::Pak(format!("打开 PAK 失败: {e}")))?;
    extract_package_files(&pkg, output_dir)
}

fn extract_package_files(pkg: &Package, extract_dir: &Path) -> Result<Vec<PakFile>> {
    let mut groups: BTreeMap<String, Vec<&PackagedFile>> = BTreeMap::new();
    for file in pkg.files() {
        let normalized = file.name().replace('\\', "/");
        groups.entry(normalized).or_default().push(file);
    }

    let mut files = Vec::with_capacity(groups.len());
    for (name, candidates) in groups {
        let mut selected: Option<(&PackagedFile, Vec<u8>)> = None;
        let mut errors = Vec::new();

        for file in candidates {
            match pkg.read_file(file) {
                Ok(data) => {
                    selected = Some((file, data));
                }
                Err(err) => {
                    errors.push(err.to_string());
                }
            }
        }

        let Some((file, data)) = selected else {
            return Err(AppError::Pak(format!(
                "解包失败: {name}: {}",
                errors.join("; ")
            )));
        };

        let output_path = safe_output_path(extract_dir, &name)?;
        if let Some(parent) = output_path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        std::fs::write(&output_path, &data)?;

        if !errors.is_empty() {
            log::warn!(
                "PAK 条目 {name} 存在不可读的重复版本，已使用可读版本: {}",
                errors.join("; ")
            );
        }

        files.push(to_pak_file(file));
    }

    files.sort_by(|a, b| a.name.cmp(&b.name));
    Ok(files)
}

fn to_pak_file(file: &PackagedFile) -> PakFile {
    let name = file.name().replace('\\', "/");
    let kind = classify_file(&name);
    let language = detect_language_from_path(&name).map(String::from);
    PakFile {
        name,
        size: file.size() as u64,
        kind,
        language,
    }
}

fn safe_output_path(root: &Path, file_name: &str) -> Result<PathBuf> {
    let mut output = root.to_path_buf();
    for component in Path::new(file_name).components() {
        match component {
            Component::Normal(part) => output.push(part),
            Component::CurDir => {}
            Component::Prefix(_) | Component::RootDir | Component::ParentDir => {
                return Err(AppError::Pak(format!(
                    "解包失败: 非法 PAK 路径: {file_name}"
                )));
            }
        }
    }
    Ok(output)
}

/// 用内置 zip 解压 zip，找出其中的 .pak。
/// 返回 (pak路径, 解压临时目录)。
fn extract_zip_to_find_pak(zip_path: &str, work_dir: &str) -> Result<(PathBuf, Option<PathBuf>)> {
    let zip_extract = Path::new(work_dir).join("zip_contents");
    std::fs::create_dir_all(&zip_extract)?;

    let file = std::fs::File::open(zip_path)?;
    let mut archive =
        zip::ZipArchive::new(file).map_err(|e| AppError::Pak(format!("读取 zip 失败: {e}")))?;

    for i in 0..archive.len() {
        let mut entry = archive
            .by_index(i)
            .map_err(|e| AppError::Pak(format!("读取 zip 条目失败: {e}")))?;
        let Some(enclosed_path) = entry.enclosed_name().map(PathBuf::from) else {
            continue;
        };
        let output_path = zip_extract.join(enclosed_path);

        if entry.is_dir() {
            std::fs::create_dir_all(&output_path)?;
            continue;
        }

        if let Some(parent) = output_path.parent() {
            std::fs::create_dir_all(parent)?;
        }

        let mut output = std::fs::File::create(&output_path)?;
        std::io::copy(&mut entry, &mut output)?;
    }

    let mut pak = None;
    walk_dir(&zip_extract, &mut |p| {
        if p.extension()
            .and_then(|e| e.to_str())
            .is_some_and(|e| e.eq_ignore_ascii_case("pak"))
            && pak.is_none()
        {
            pak = Some(p.to_path_buf());
        }
    });
    let pak =
        pak.ok_or_else(|| AppError::Pak("zip 内未找到 .pak 文件，请确认这是 BG3 MOD。".into()))?;

    Ok((pak, Some(zip_extract)))
}

/// 重新打包工作目录为 .pak。
pub fn repack(work_dir: &str, output_path: &str) -> Result<()> {
    let unpacked = Path::new(work_dir).join("unpacked");
    if !unpacked.exists() {
        return Err(AppError::Config(format!(
            "工作目录无效（缺少 unpacked 子目录）: {work_dir}"
        )));
    }

    // 读取 priority（bg3rustpaklib 的 priority 是 u8）
    let meta_path = Path::new(work_dir).join("pak_meta.json");
    let priority: u8 = std::fs::read_to_string(&meta_path)
        .ok()
        .and_then(|s| serde_json::from_str::<serde_json::Value>(&s).ok())
        .and_then(|v| v.get("priority").and_then(|p| p.as_u64()))
        .unwrap_or(0) as u8;

    let builder = PackageBuilder::new()
        .priority(priority)
        .add_directory(&unpacked)
        .map_err(|e| AppError::Pak(format!("add_directory 失败: {e}")))?;
    builder
        .build(output_path)
        .map_err(|e| AppError::Pak(format!("打包失败: {e}")))?;

    Ok(())
}

/// 工作目录根路径，基于原始文件名 + 时间戳。
fn create_work_dir() -> Result<String> {
    let ts = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis())
        .unwrap_or(0);
    let dir = std::env::temp_dir().join(format!("bg3-translate-{ts}"));
    std::fs::create_dir_all(&dir)?;
    Ok(dir.to_string_lossy().into_owned())
}

/// 递归遍历目录，对每个文件调用回调。
pub fn walk_dir(dir: &Path, cb: &mut dyn FnMut(&Path)) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            walk_dir(&path, cb);
        } else {
            cb(&path);
        }
    }
}

/// 解包后的文件根目录（unpacked 子目录）。
pub fn unpacked_dir(work_dir: &str) -> PathBuf {
    Path::new(work_dir).join("unpacked")
}

/// 给定 PAK 内的文件名，返回解包后磁盘上的绝对路径。
pub fn resolve_disk_path(work_dir: &str, file_name: &str) -> PathBuf {
    unpacked_dir(work_dir).join(file_name)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn safe_output_path_rejects_parent_traversal() {
        let root = Path::new("root");
        assert!(safe_output_path(root, "../evil.txt").is_err());
        assert!(safe_output_path(root, "/evil.txt").is_err());
        assert_eq!(
            safe_output_path(root, "Localization/English/test.xml").unwrap(),
            root.join("Localization").join("English").join("test.xml")
        );
    }
}
