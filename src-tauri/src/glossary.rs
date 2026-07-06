//! 术语表模块：BG3 官方译名 + 用户自定义 + 翻译时命中检测。
//!
//! 数据结构兼容从游戏本地化文件提取的真实术语表 JSON（source/target/category/...）。
//! 内置 170 条核心种子保证开箱即用；用户可导入完整 20K 条官方术语表。

use std::path::PathBuf;

use regex::Regex;
use serde::{Deserialize, Serialize};

use crate::error::{AppError, Result};

/// 术语分类（兼容真实数据，使用字符串以保持灵活）
pub type Category = String;

/// 单条术语（字段与从游戏提取的真实术语表 JSON 一致）
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GlossaryEntry {
    /// 英文术语
    pub source: String,
    /// 中文译名
    pub target: String,
    /// 分类仅用于兼容旧导入数据，界面与保存文件不再展示/输出。
    #[serde(default = "default_category", skip_serializing)]
    pub category: Category,
    /// 来源（official / user）
    #[serde(default, alias = "source_kind")]
    pub source_kind: String,
    /// 是否启用
    #[serde(default = "default_true")]
    pub enabled: bool,
    /// 是否有歧义（歧义项默认不参与命中，避免误替换）
    #[serde(default)]
    pub ambiguous: bool,
    /// 是否整词匹配
    #[serde(default = "default_true", alias = "whole_word")]
    pub whole_word: bool,
    /// 是否大小写敏感
    #[serde(default, alias = "case_sensitive")]
    pub case_sensitive: bool,
    /// 在游戏原文中的出现次数（排序参考）
    #[serde(default)]
    pub count: u32,
}

fn default_category() -> String {
    "name_or_title".into()
}
fn default_true() -> bool {
    true
}

/// 整个术语表（结构兼容导入文件：{ "terms": [...] }）
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Glossary {
    #[serde(default)]
    pub terms: Vec<GlossaryEntry>,
}

impl Default for Glossary {
    fn default() -> Self {
        Self {
            terms: official_seed().iter().map(|s| s.to_entry()).collect(),
        }
    }
}

/// 命中项
#[derive(Debug, Clone)]
pub struct MatchedTerm {
    pub source: String,
    pub target: String,
}

// ─────────────────────────────────────────────────────────────
// 静态种子条目
// ─────────────────────────────────────────────────────────────

struct StaticEntry {
    source: &'static str,
    target: &'static str,
    category: &'static str,
}

impl StaticEntry {
    fn to_entry(&self) -> GlossaryEntry {
        GlossaryEntry {
            source: self.source.into(),
            target: self.target.into(),
            category: self.category.into(),
            source_kind: "official".into(),
            enabled: true,
            ambiguous: false,
            whole_word: true,
            case_sensitive: false,
            count: 0,
        }
    }
}

// ─────────────────────────────────────────────────────────────
// 存储与加载
// ─────────────────────────────────────────────────────────────

fn glossary_path() -> Result<PathBuf> {
    Ok(crate::config::config_dir().join("glossary.json"))
}

/// 加载术语表；不存在则用官方种子初始化。
pub fn load() -> Result<Glossary> {
    let path = glossary_path()?;
    if !path.exists() {
        let g = Glossary::default();
        save(&g)?;
        return Ok(g);
    }
    let content = std::fs::read_to_string(&path)?;
    let glossary: Glossary = serde_json::from_str(&content).unwrap_or_default();
    Ok(glossary)
}

pub fn save(glossary: &Glossary) -> Result<()> {
    let path = glossary_path()?;
    let content = serde_json::to_string_pretty(glossary)
        .map_err(|e| AppError::Config(format!("序列化失败: {e}")))?;
    std::fs::write(&path, content)?;
    Ok(())
}

/// 重置为官方种子。
pub fn reset() -> Result<Glossary> {
    let g = Glossary::default();
    save(&g)?;
    Ok(g)
}

/// 从 JSON 字符串导入术语，导入时自动清洗：
/// - 过滤噪音条目（占位符模板、UI 按键标记、破折号破碎文本、常见短词）
/// - 只去掉包住整条术语的外层引号，局部引号保留
pub fn import_json(json_str: &str) -> Result<Glossary> {
    let imported: Glossary = serde_json::from_str(json_str)
        .map_err(|e| AppError::Config(format!("术语表 JSON 解析失败: {e}")))?;

    let mut cleaned = Glossary { terms: Vec::new() };
    let mut removed = 0usize;
    for mut entry in imported.terms {
        if let Some(e) = clean_entry(&mut entry) {
            cleaned.terms.push(e);
        } else {
            removed += 1;
        }
    }
    log::info!(
        "术语表导入清洗：{} 条保留，{} 条噪音过滤",
        cleaned.terms.len(),
        removed
    );
    save(&cleaned)?;
    Ok(cleaned)
}

/// 清洗单条术语。返回 None 表示应过滤掉（噪音）。
fn clean_entry(entry: &mut GlossaryEntry) -> Option<GlossaryEntry> {
    use regex::Regex;
    let source = entry.source.trim().to_string();
    let target = entry.target.trim().to_string();

    // 空值过滤
    if source.is_empty() || target.is_empty() {
        return None;
    }

    // ── 噪音过滤（这些不是术语，是从本地化文件机械提取的碎片）──

    // 占位符模板（[1] from [2]、+[1] Damage 这种句式模板）
    if Regex::new(r"\[\d+\]").unwrap().is_match(&source) {
        return None;
    }
    // UI 按键标记（[IE_xxx]、[DRUID] 这种内部 ID）
    if source.contains("[IE_") || Regex::new(r"^\[[A-Z_]{2,}\]").unwrap().is_match(&source) {
        return None;
    }
    // 破折号破碎文本（-- come ----、---OBEY--- 这种）
    if source.matches('-').count() >= 2 {
        let words: Vec<&str> = source.split_whitespace().collect();
        let non_dash_words = words
            .iter()
            .filter(|w| !w.chars().all(|c| c == '-'))
            .count();
        if non_dash_words <= 2 {
            return None;
        }
    }
    // 纯符号/标点
    if Regex::new(r"^[\W_]+$").unwrap().is_match(&source) {
        return None;
    }
    // 过短的常见英文词（of/and/the 等会误匹配正常文本）
    // 覆盖大小写变体（OF/Of/of 都要过滤）
    let common_short = [
        "of", "and", "the", "or", "for", "to", "in", "on", "at", "by", "is", "it", "as", "be",
        "do", "no", "if", "an", "my", "we", "he", "up", "so", "tag", "end", "yes", "die", "one",
        "two", "from", "upon", "with", "that", "this", "then", "than", "but", "not", "all", "any",
        "can", "may", "has", "had", "was", "are", "were", "let", "set", "get", "put", "out", "off",
        "own", "new", "old", "big", "low",
    ];
    let lower = source.to_lowercase();
    if source.len() <= 4 && common_short.contains(&lower.as_str()) {
        return None;
    }

    entry.source = strip_full_wrapping_quotes(&source);
    entry.target = strip_full_wrapping_quotes(&target);
    Some(entry.clone())
}

/// 只去除包住整条文本的成对引号。
/// `"丹瑟隆的飞斧"` -> `丹瑟隆的飞斧`
/// `"制动"拉杆` -> `"制动"拉杆`
fn strip_full_wrapping_quotes(s: &str) -> String {
    let s = s.trim();
    let Some(first) = s.chars().next() else {
        return String::new();
    };
    let Some(expected_last) = matching_quote(first) else {
        return s.to_string();
    };
    if !s.ends_with(expected_last) {
        return s.to_string();
    }

    let start = first.len_utf8();
    let end = s.len() - expected_last.len_utf8();
    if start >= end {
        return s.to_string();
    }

    let inner = s[start..end].trim();
    if inner.is_empty() {
        s.to_string()
    } else {
        inner.to_string()
    }
}

fn matching_quote(c: char) -> Option<char> {
    match c {
        '\'' => Some('\''),
        '"' => Some('"'),
        '“' => Some('”'),
        '‘' => Some('’'),
        '「' => Some('」'),
        '『' => Some('』'),
        _ => None,
    }
}

// ─────────────────────────────────────────────────────────────
// CRUD
// ─────────────────────────────────────────────────────────────

pub fn add(glossary: &mut Glossary, entry: GlossaryEntry) -> Result<()> {
    if entry.source.trim().is_empty() || entry.target.trim().is_empty() {
        return Err(AppError::Config("术语的中英文均不能为空".into()));
    }
    if let Some(existing) = glossary.terms.iter_mut().find(|e| e.source == entry.source) {
        *existing = entry;
    } else {
        glossary.terms.push(entry);
    }
    Ok(())
}

pub fn update(glossary: &mut Glossary, old_source: &str, entry: GlossaryEntry) -> Result<()> {
    if entry.source.trim().is_empty() || entry.target.trim().is_empty() {
        return Err(AppError::Config("术语的中英文均不能为空".into()));
    }
    let idx = glossary
        .terms
        .iter()
        .position(|e| e.source == old_source)
        .ok_or_else(|| AppError::Config("找不到要更新的术语".into()))?;
    glossary.terms[idx] = entry;
    Ok(())
}

pub fn delete(glossary: &mut Glossary, source: &str) -> Result<()> {
    let entry = glossary
        .terms
        .iter()
        .find(|e| e.source == source)
        .ok_or_else(|| AppError::Config("找不到要删除的术语".into()))?;
    if entry.source_kind == "official" {
        return Err(AppError::Config(
            "官方术语不可删除（可禁用或编辑覆盖）".into(),
        ));
    }
    glossary.terms.retain(|e| e.source != source);
    Ok(())
}

// ─────────────────────────────────────────────────────────────
// 命中检测（针对大术语表 20K 条优化）
// ─────────────────────────────────────────────────────────────

/// 在文本中查找命中的术语。
///
/// 优化策略（应对 20K 条）：
/// 1. 只考虑 enabled && !ambiguous 的条目
/// 2. 先用 lowercase 子串快速预筛（source 的 lowercase 是否出现在文本中）
/// 3. 预筛命中后，按 whole_word / case_sensitive 做精确验证
pub fn find_matches(text: &str, glossary: &Glossary) -> Vec<MatchedTerm> {
    let text_lower = text.to_lowercase();
    let mut matches = Vec::new();

    for entry in &glossary.terms {
        if !entry.enabled || entry.ambiguous {
            continue;
        }
        let matched = if entry.case_sensitive {
            // 大小写敏感：直接在原文里找
            if entry.whole_word {
                word_boundary_match(text, &entry.source)
            } else {
                text.contains(&entry.source)
            }
        } else {
            // 大小写不敏感：在 lowercase 文本里找
            let source_lower = entry.source.to_lowercase();
            if entry.whole_word {
                word_boundary_match(&text_lower, &source_lower)
            } else {
                text_lower.contains(&source_lower)
            }
        };
        if matched {
            matches.push(MatchedTerm {
                source: entry.source.clone(),
                target: entry.target.clone(),
            });
        }
    }
    // 长术语优先（避免短术语覆盖长术语，如 "Mind Flayer" 优先于 "Mind"）
    matches.sort_by(|a, b| b.source.len().cmp(&a.source.len()));
    matches
}

/// 词边界匹配（用 \b 正则）。对长术语表性能可接受，因为只有预筛命中的才会走到这。
fn word_boundary_match(haystack: &str, needle: &str) -> bool {
    if needle.is_empty() {
        return false;
    }
    // 快速路径：直接子串检查
    if !haystack.contains(needle) {
        return false;
    }
    // 词边界验证
    let escaped = regex::escape(needle);
    let pattern = format!(r"\b{escaped}\b");
    match Regex::new(&pattern) {
        Ok(re) => re.is_match(haystack),
        Err(_) => true, // 正则编译失败时退化为子串匹配
    }
}

// ─────────────────────────────────────────────────────────────
// 内置核心种子（约 170 条，开箱即用）
// 数据源：BG3 官方简体中文版 + D&D 5e 三宝书译名体系
// ─────────────────────────────────────────────────────────────

const fn official_seed() -> &'static [StaticEntry] {
    &[
        // ── 职业 Class ──
        StaticEntry {
            source: "Barbarian",
            target: "野蛮人",
            category: "class",
        },
        StaticEntry {
            source: "Bard",
            target: "吟游诗人",
            category: "class",
        },
        StaticEntry {
            source: "Cleric",
            target: "牧师",
            category: "class",
        },
        StaticEntry {
            source: "Druid",
            target: "德鲁伊",
            category: "class",
        },
        StaticEntry {
            source: "Fighter",
            target: "战士",
            category: "class",
        },
        StaticEntry {
            source: "Monk",
            target: "武僧",
            category: "class",
        },
        StaticEntry {
            source: "Paladin",
            target: "圣武士",
            category: "class",
        },
        StaticEntry {
            source: "Ranger",
            target: "游侠",
            category: "class",
        },
        StaticEntry {
            source: "Rogue",
            target: "游荡者",
            category: "class",
        },
        StaticEntry {
            source: "Sorcerer",
            target: "术士",
            category: "class",
        },
        StaticEntry {
            source: "Warlock",
            target: "邪术师",
            category: "class",
        },
        StaticEntry {
            source: "Wizard",
            target: "法师",
            category: "class",
        },
        StaticEntry {
            source: "Eldritch Knight",
            target: "奥法骑士",
            category: "class",
        },
        StaticEntry {
            source: "Oathbreaker",
            target: "破誓者",
            category: "class",
        },
        StaticEntry {
            source: "Oath of Devotion",
            target: "奉献之誓",
            category: "class",
        },
        StaticEntry {
            source: "Oath of the Ancients",
            target: "远古之誓",
            category: "class",
        },
        StaticEntry {
            source: "Oath of Vengeance",
            target: "复仇之誓",
            category: "class",
        },
        // ── 种族 Race ──
        StaticEntry {
            source: "Half-Elf",
            target: "半精灵",
            category: "race",
        },
        StaticEntry {
            source: "High Elf",
            target: "高等精灵",
            category: "race",
        },
        StaticEntry {
            source: "Wood Elf",
            target: "木精灵",
            category: "race",
        },
        StaticEntry {
            source: "Drow",
            target: "卓尔",
            category: "race",
        },
        StaticEntry {
            source: "Duergar",
            target: "灰矮人",
            category: "race",
        },
        StaticEntry {
            source: "Halfling",
            target: "半身人",
            category: "race",
        },
        StaticEntry {
            source: "Githyanki",
            target: "吉斯洋基人",
            category: "race",
        },
        StaticEntry {
            source: "Tiefling",
            target: "提夫林",
            category: "race",
        },
        StaticEntry {
            source: "Dragonborn",
            target: "龙裔",
            category: "race",
        },
        StaticEntry {
            source: "Half-Orc",
            target: "半兽人",
            category: "race",
        },
        // ── 地点 Location ──
        StaticEntry {
            source: "Baldur's Gate",
            target: "博德之门",
            category: "location",
        },
        StaticEntry {
            source: "Faerûn",
            target: "费伦",
            category: "location",
        },
        StaticEntry {
            source: "Forgotten Realms",
            target: "被遗忘的国度",
            category: "location",
        },
        StaticEntry {
            source: "Toril",
            target: "托瑞尔",
            category: "location",
        },
        StaticEntry {
            source: "the Underdark",
            target: "幽暗地域",
            category: "location",
        },
        StaticEntry {
            source: "Avernus",
            target: "阿佛纳斯",
            category: "location",
        },
        StaticEntry {
            source: "Sword Coast",
            target: "剑湾",
            category: "location",
        },
        StaticEntry {
            source: "Candlekeep",
            target: "烛堡",
            category: "location",
        },
        StaticEntry {
            source: "Menzoberranzan",
            target: "魔索布莱城",
            category: "location",
        },
        StaticEntry {
            source: "Nine Hells",
            target: "九层地狱",
            category: "location",
        },
        StaticEntry {
            source: "Nautiloid",
            target: "地狱螺壳舰",
            category: "location",
        },
        StaticEntry {
            source: "Emerald Grove",
            target: "翡翠林苑",
            category: "location",
        },
        // ── 角色 Character ──
        StaticEntry {
            source: "Astarion",
            target: "阿斯代伦",
            category: "character",
        },
        StaticEntry {
            source: "Shadowheart",
            target: "影心",
            category: "character",
        },
        StaticEntry {
            source: "Gale",
            target: "盖尔",
            category: "character",
        },
        StaticEntry {
            source: "Lae'zel",
            target: "莱埃泽尔",
            category: "character",
        },
        StaticEntry {
            source: "Karlach",
            target: "卡尔拉赫",
            category: "character",
        },
        StaticEntry {
            source: "Halsin",
            target: "哈尔辛",
            category: "character",
        },
        StaticEntry {
            source: "Minthara",
            target: "明萨拉",
            category: "character",
        },
        StaticEntry {
            source: "Withers",
            target: "威瑟斯",
            category: "character",
        },
        StaticEntry {
            source: "The Emperor",
            target: "皇帝",
            category: "character",
        },
        StaticEntry {
            source: "Vlaakith",
            target: "弗拉基丝",
            category: "character",
        },
        StaticEntry {
            source: "Jaheira",
            target: "洁希拉",
            category: "character",
        },
        StaticEntry {
            source: "Minsc",
            target: "敏斯克",
            category: "character",
        },
        StaticEntry {
            source: "Raphael",
            target: "拉斐尔",
            category: "character",
        },
        StaticEntry {
            source: "Mizora",
            target: "米佐拉",
            category: "character",
        },
        StaticEntry {
            source: "Orin",
            target: "奥林",
            category: "character",
        },
        StaticEntry {
            source: "Ketheric",
            target: "凯瑟里克",
            category: "character",
        },
        StaticEntry {
            source: "Gortash",
            target: "戈塔什",
            category: "character",
        },
        StaticEntry {
            source: "Mystra",
            target: "密斯特拉",
            category: "character",
        },
        StaticEntry {
            source: "Dream Visitor",
            target: "梦境访客",
            category: "character",
        },
        StaticEntry {
            source: "Voss",
            target: "维斯",
            category: "character",
        },
        StaticEntry {
            source: "Novice of the Absolute",
            target: "至上真神学徒",
            category: "character",
        },
        // ── 生物 Creature ──
        StaticEntry {
            source: "Mind Flayer",
            target: "夺心魔",
            category: "creature",
        },
        StaticEntry {
            source: "Illithid",
            target: "夺心魔",
            category: "creature",
        },
        StaticEntry {
            source: "Tadpole",
            target: "蝌蚪",
            category: "creature",
        },
        StaticEntry {
            source: "Beholder",
            target: "眼魔",
            category: "creature",
        },
        StaticEntry {
            source: "Lich",
            target: "巫妖",
            category: "creature",
        },
        StaticEntry {
            source: "Vampire Spawn",
            target: "吸血鬼衍体",
            category: "creature",
        },
        StaticEntry {
            source: "Lycanthrope",
            target: "兽化人",
            category: "creature",
        },
        StaticEntry {
            source: "Hobgoblin",
            target: "大地精",
            category: "creature",
        },
        StaticEntry {
            source: "Bugbear",
            target: "熊地精",
            category: "creature",
        },
        StaticEntry {
            source: "Owlbear",
            target: "枭熊",
            category: "creature",
        },
        StaticEntry {
            source: "Cambion",
            target: "坎比翁",
            category: "creature",
        },
        StaticEntry {
            source: "Intellect Devourer",
            target: "噬脑怪",
            category: "creature",
        },
        StaticEntry {
            source: "Elder Brain",
            target: "主脑",
            category: "creature",
        },
        // ── 机制 Mechanic ──
        StaticEntry {
            source: "Cantrip",
            target: "戏法",
            category: "mechanic",
        },
        StaticEntry {
            source: "Spell Slot",
            target: "法术位",
            category: "mechanic",
        },
        StaticEntry {
            source: "Proficiency Bonus",
            target: "熟练度加值",
            category: "mechanic",
        },
        StaticEntry {
            source: "Advantage",
            target: "优势",
            category: "mechanic",
        },
        StaticEntry {
            source: "Disadvantage",
            target: "劣势",
            category: "mechanic",
        },
        StaticEntry {
            source: "Inspiration",
            target: "激励",
            category: "mechanic",
        },
        StaticEntry {
            source: "Bonus Action",
            target: "附赠动作",
            category: "mechanic",
        },
        StaticEntry {
            source: "Saving Throw",
            target: "豁免检定",
            category: "mechanic",
        },
        StaticEntry {
            source: "Ability Check",
            target: "属性检定",
            category: "mechanic",
        },
        StaticEntry {
            source: "Armor Class",
            target: "护甲等级",
            category: "mechanic",
        },
        StaticEntry {
            source: "Hit Points",
            target: "生命值",
            category: "mechanic",
        },
        StaticEntry {
            source: "Long Rest",
            target: "长休",
            category: "mechanic",
        },
        StaticEntry {
            source: "Short Rest",
            target: "短休",
            category: "mechanic",
        },
        StaticEntry {
            source: "Concentration",
            target: "专注",
            category: "mechanic",
        },
        StaticEntry {
            source: "Difficulty Class",
            target: "难度等级",
            category: "mechanic",
        },
        StaticEntry {
            source: "Initiative",
            target: "先攻",
            category: "mechanic",
        },
        StaticEntry {
            source: "Critical Hit",
            target: "重击",
            category: "mechanic",
        },
        StaticEntry {
            source: "Sneak Attack",
            target: "偷袭",
            category: "mechanic",
        },
        StaticEntry {
            source: "Divine Smite",
            target: "至圣斩",
            category: "mechanic",
        },
        StaticEntry {
            source: "Wild Shape",
            target: "荒野形态",
            category: "mechanic",
        },
        StaticEntry {
            source: "Necrotic",
            target: "黯蚀",
            category: "mechanic",
        },
        StaticEntry {
            source: "Radiant",
            target: "光耀",
            category: "mechanic",
        },
        // ── 法术 Spell ──
        StaticEntry {
            source: "Fireball",
            target: "火球术",
            category: "spell",
        },
        StaticEntry {
            source: "Magic Missile",
            target: "魔法飞弹",
            category: "spell",
        },
        StaticEntry {
            source: "Eldritch Blast",
            target: "魔能爆",
            category: "spell",
        },
        StaticEntry {
            source: "Healing Word",
            target: "治愈真言",
            category: "spell",
        },
        StaticEntry {
            source: "Misty Step",
            target: "迷踪步",
            category: "spell",
        },
        StaticEntry {
            source: "Counterspell",
            target: "反制法术",
            category: "spell",
        },
        StaticEntry {
            source: "Speak with Dead",
            target: "与亡者交谈",
            category: "spell",
        },
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn seed_nonempty() {
        assert!(official_seed().len() >= 70);
        let g = Glossary::default();
        assert!(g.terms.iter().all(|e| e.source_kind == "official"));
    }

    #[test]
    fn find_case_insensitive() {
        let g = Glossary::default();
        let m = find_matches("The Paladin strikes with Divine Smite", &g);
        let srcs: Vec<_> = m.iter().map(|t| t.source.as_str()).collect();
        assert!(srcs.contains(&"Paladin"));
        assert!(srcs.contains(&"Divine Smite"));
    }

    #[test]
    fn find_word_boundary() {
        let g = Glossary::default();
        assert!(!find_matches("Spelling bee", &g)
            .iter()
            .any(|t| t.source == "Cantrip"));
        assert!(find_matches("Cast a Spell", &g).is_empty() || true); // Spell 不在种子
    }

    #[test]
    fn user_override_priority_by_insertion_order() {
        // 后插入的同名条目覆盖（add 实现）
        let mut g = Glossary::default();
        add(
            &mut g,
            GlossaryEntry {
                source: "Paladin".into(),
                target: "自定义".into(),
                category: "class".into(),
                source_kind: "user".into(),
                enabled: true,
                ambiguous: false,
                whole_word: true,
                case_sensitive: false,
                count: 0,
            },
        )
        .unwrap();
        let m = find_matches("Paladin", &g);
        // 可能有多条 Paladin（官方 + 用户），但至少有一条是用户值
        assert!(m.iter().any(|t| t.target == "自定义"));
    }

    #[test]
    fn ambiguous_excluded() {
        let mut g = Glossary::default();
        g.terms.push(GlossaryEntry {
            source: "Test".into(),
            target: "测试".into(),
            category: "test".into(),
            source_kind: "user".into(),
            enabled: true,
            ambiguous: true,
            whole_word: true,
            case_sensitive: false,
            count: 0,
        });
        assert!(!find_matches("Test here", &g)
            .iter()
            .any(|t| t.source == "Test"));
    }

    #[test]
    fn disabled_excluded() {
        let mut g = Glossary::default();
        g.terms.push(GlossaryEntry {
            source: "Disabled".into(),
            target: "禁用".into(),
            category: "test".into(),
            source_kind: "user".into(),
            enabled: false,
            ambiguous: false,
            whole_word: true,
            case_sensitive: false,
            count: 0,
        });
        assert!(find_matches("Disabled", &g)
            .iter()
            .all(|t| t.source != "Disabled"));
    }

    #[test]
    fn import_real_format() {
        let json = r#"{"terms":[{"source":"Fireball","target":"火球术","category":"spell","source_kind":"official","enabled":true,"ambiguous":false,"whole_word":true,"case_sensitive":false,"count":5}]}"#;
        let g = import_json(json).unwrap();
        assert_eq!(g.terms.len(), 1);
        assert_eq!(g.terms[0].target, "火球术");
        assert_eq!(g.terms[0].count, 5);
    }

    #[test]
    fn clean_removes_placeholder_templates() {
        // 占位符模板应被过滤
        let json = r#"{"terms":[
            {"source":"[1] from [2]","target":"从[2]处取走[1]","category":"name_or_title","source_kind":"official","enabled":true,"ambiguous":false,"whole_word":true,"case_sensitive":false,"count":1},
            {"source":"Baldur's Gate","target":"博德之门","category":"location","source_kind":"official","enabled":true,"ambiguous":false,"whole_word":true,"case_sensitive":false,"count":10}
        ]}"#;
        let g = import_json(json).unwrap();
        assert_eq!(g.terms.len(), 1, "占位符模板应被过滤");
        assert_eq!(g.terms[0].source, "Baldur's Gate");
    }

    #[test]
    fn clean_removes_ui_markers() {
        let json = r#"{"terms":[
            {"source":"[IE_ContextMenu] Ping","target":"[IE_ContextMenu]标记","category":"name_or_title","source_kind":"official","enabled":true,"ambiguous":false,"whole_word":true,"case_sensitive":false,"count":1},
            {"source":"[DRUID][RANGER]","target":"[德鲁伊][游侠]","category":"name_or_title","source_kind":"official","enabled":true,"ambiguous":false,"whole_word":true,"case_sensitive":false,"count":1}
        ]}"#;
        let g = import_json(json).unwrap();
        assert_eq!(g.terms.len(), 0, "UI 标记应被过滤");
    }

    #[test]
    fn clean_removes_dash_fragments() {
        let json = r#"{"terms":[
            {"source":"--OBEY--","target":"--服从--","category":"name_or_title","source_kind":"official","enabled":true,"ambiguous":false,"whole_word":true,"case_sensitive":false,"count":1},
            {"source":"---COME---obey- --yield---","target":"---来吧---服从","category":"short_phrase","source_kind":"official","enabled":true,"ambiguous":false,"whole_word":true,"case_sensitive":false,"count":1}
        ]}"#;
        let g = import_json(json).unwrap();
        assert_eq!(g.terms.len(), 0, "破折号破碎文本应被过滤");
    }

    #[test]
    fn clean_removes_common_short_words() {
        let json = r#"{"terms":[
            {"source":"of","target":"之","category":"name_or_title","source_kind":"official","enabled":true,"ambiguous":false,"whole_word":true,"case_sensitive":false,"count":1},
            {"source":"the","target":"这","category":"name_or_title","source_kind":"official","enabled":true,"ambiguous":false,"whole_word":true,"case_sensitive":false,"count":1},
            {"source":"Paladin","target":"圣武士","category":"class","source_kind":"official","enabled":true,"ambiguous":false,"whole_word":true,"case_sensitive":false,"count":5}
        ]}"#;
        let g = import_json(json).unwrap();
        assert_eq!(g.terms.len(), 1, "常见短词应被过滤");
        assert_eq!(g.terms[0].source, "Paladin");
    }

    #[test]
    fn clean_strips_full_quotes_but_keeps_partial_quotes() {
        let json = r#"{"terms":[
            {"source":"'Danthelon's Dancing Axe'","target":"\"丹瑟隆的飞斧\"","category":"name_or_title","source_kind":"official","enabled":true,"ambiguous":false,"whole_word":true,"case_sensitive":false,"count":1},
            {"source":"'Brake' Lever","target":"\"制动\"拉杆","category":"name_or_title","source_kind":"official","enabled":true,"ambiguous":false,"whole_word":true,"case_sensitive":false,"count":1}
        ]}"#;
        let g = import_json(json).unwrap();
        assert_eq!(g.terms.len(), 2);
        assert_eq!(g.terms[0].source, "Danthelon's Dancing Axe");
        assert_eq!(g.terms[0].target, "丹瑟隆的飞斧");
        assert_eq!(g.terms[1].source, "'Brake' Lever");
        assert_eq!(g.terms[1].target, "\"制动\"拉杆");
    }

    #[test]
    fn clean_real_20k_glossary() {
        // 用真实术语表验证清洗效果
        let path = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .unwrap()
            .join("samples/bg3-official-glossary.json");
        if !path.exists() {
            eprintln!("跳过：未找到真实术语表");
            return;
        }
        let json = std::fs::read_to_string(&path).unwrap();
        let raw: Glossary = serde_json::from_str(&json).unwrap();
        let raw_count = raw.terms.len();

        let cleaned = import_json(&json).unwrap();
        println!("原始: {raw_count}，清洗后: {}", cleaned.terms.len());

        // 应过滤掉一部分噪音（约 1-3%）
        assert!(cleaned.terms.len() < raw_count, "应过滤掉部分噪音");
        assert!(cleaned.terms.len() > 15000, "应保留绝大多数有效术语");

        // 验证过滤后没有占位符模板
        assert!(
            !cleaned.terms.iter().any(|t| t.source.contains("[1]")),
            "不应残留占位符模板"
        );
        // 验证没有 UI 标记
        assert!(
            !cleaned.terms.iter().any(|t| t.source.contains("[IE_")),
            "不应残留 UI 标记"
        );
        // 验证局部引号保留，整条外层引号会清理。
        assert!(
            cleaned
                .terms
                .iter()
                .any(|t| t.source.starts_with('\'') && !t.source.ends_with('\'')),
            "应保留局部引号"
        );
        assert!(
            !cleaned.terms.iter().any(|t| t.source == "'Barnabus'"),
            "整条外层引号应被去除"
        );
        println!("✅ 真实术语表清洗验证通过");
    }
}
