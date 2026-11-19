//! 0.2 Electron 的导航, 标题, 分区和快捷键.
//! 乐章合并和多碟导入不在侧栏里, 仍从分轨页和 CD 抓轨整理进入.

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Place {
    Projects,
    Split,
    Scripts,
    History,
    Templates,
}

pub struct Item {
    pub id: &'static str,
    pub mark: &'static str,
    pub label: &'static str,
    pub place: Place,
}

pub fn navigation() -> [Item; 5] {
    [
        Item { id: "nav-projects", mark: "▤", label: "录音项目", place: Place::Projects },
        Item { id: "nav-split", mark: "✂", label: "整场录音分轨", place: Place::Split },
        Item { id: "nav-scripts", mark: "›_", label: "脚本生成", place: Place::Scripts },
        Item { id: "nav-history", mark: "◷", label: "处理记录", place: Place::History },
        Item { id: "nav-templates", mark: "▧", label: "模板与命名", place: Place::Templates },
    ]
}

pub fn heading(place: Place) -> (&'static str, &'static str) {
    match place {
        Place::Projects => ("录音项目", "归档演出录音，管理分轨、专辑标签、封面和处理进度。"),
        Place::Split => ("整场录音分轨", "试听原始录音，标记起止点，使用 FFmpeg 分轨与处理。"),
        Place::Scripts => ("脚本生成", "将所选音频和封面放在同一个文件夹中，在该文件夹打开 Windows PowerShell，粘贴脚本运行。"),
        Place::History => ("处理记录", "按操作与结果查看项目历史、错误和修订。"),
        Place::Templates => ("模板与命名", "保存常用处理参数和英文文件命名规则。"),
    }
}

pub mod section {
    pub const IMPORT: &str = "1. 导入录音";
    pub const SESSION: &str = "2. 场次信息";
    pub const COVER: &str = "专辑封面";
    pub const TRACKS: &str = "3. 曲目与时间点";
    pub const OUTPUT: &str = "4. 输出设置";
    pub const RESULT: &str = "分轨结果";
}

pub fn split_sections() -> [&'static str; 6] {
    [section::IMPORT, section::SESSION, section::COVER, section::TRACKS, section::OUTPUT, section::RESULT]
}

pub mod label {
    pub const CD: &str = "CD 抓轨整理";
    pub const NEW_SESSION: &str = "新建分轨场次";
    pub const IMPORT_AUDIO: &str = "导入录音文件";
    pub const RECOGNIZE_AUDIO: &str = "识别录音标签";
    pub const CONCAT: &str = "按此顺序拼接";
    pub const BACK_FIVE: &str = "↶ 后退 5 秒";
    pub const FORWARD_FIVE: &str = "↷ 前进 5 秒";
    pub const PREVIEW_FROM: &str = "从指定时间试听 20 秒";
    pub const RETURN_PREVIEW: &str = "返回整场试听";
    pub const ENGLISH: &str = "一键转英文并核对";
    pub const UPLOAD_COVER: &str = "上传场照制作封面";
    pub const PICK_BACK: &str = "选择封底";
    pub const RUN: &str = "运行 FFmpeg";
    pub const LOCAL_SCRIPT: &str = "生成本机 PowerShell 脚本";
    pub const STOP: &str = "停止处理";
    pub const SAVE_PROJECT: &str = "保存到录音项目";
    pub const OPEN_SAVED: &str = "打开已保存项目";
    pub const PREVIEW_FX: &str = "试听处理效果";
    pub const COVER_VIDEO: &str = "制作封面视频";
    pub const OPEN_PROJECT: &str = "打开项目文件";
    pub const SAVE_LOCAL: &str = "保存为本地文件";
    pub const NEW_PROJECT: &str = "＋ 新建项目";
    pub const ADD_TRACK: &str = "＋ 添加曲目";
    pub const EXPORT_BACKUP: &str = "↓ 导出备份";
    pub const IMPORT_BACKUP: &str = "↑ 导入备份";
    pub const LLM: &str = "LLM 配置";
    pub const LLM_FILES: &str = "模型识别文件";
    pub const CLOSE: &str = "关闭";
    pub const JOIN: &str = "终点留空时接到同一文件的下一轨起点，末轨到该文件结束";
    pub const MERGE: &str = "合并所选为一轨";
    pub const UNMERGE: &str = "拆回乐章";
    pub const MERGE_GROUPS: &str = "合并建议的乐章";
    pub const PREFLIGHT: &str = "导出前检查";
    pub const EMBED_BACK: &str = "嵌入封底";
    pub const DISC_FOLDERS: &str = "按碟分子文件夹";
    pub const APPEND_DISC: &str = "追加为新碟";
    pub const SYNC: &str = "本机工作区 · 已连接";
    pub const SCRIPT_CARD: &str = "PowerShell 预览";
    pub const SCRIPT_EMPTY: &str = "请先创建录音项目。";
    pub const BACK_LIST: &str = "← 返回项目列表";
    pub const EDIT_PROJECT: &str = "编辑项目";
    pub const OPEN_SPLIT: &str = "在软件中分轨 ↗";
    pub const OPEN_SCRIPT: &str = "生成脚本 ↗";
    pub const OPEN_ROW: &str = "打开 ↗";
    pub const CLEAR_FILTER: &str = "清除筛选";
    pub const MORE_FILTER: &str = "更多筛选 ▾";
    pub const EMPTY_PROJECTS: &str = "没有符合条件的录音项目。";
    pub const NEW_TEMPLATE: &str = "＋ 新建模板";
    pub const NAMING_CARD: &str = "默认曲目命名规则";
    pub const SESSION_CARD: &str = "演出与录音";
    pub const FILES_CARD: &str = "原始文件";
    pub const AUDIO_CARD: &str = "封面与音频设置";
}

pub fn project_track_columns() -> [&'static str; 5] {
    ["顺序", "Title / Composer", "原始文件", "分轨时间", "英文文件名（不含扩展名）"]
}

pub fn header_tools() -> [&'static str; 4] {
    [label::LLM, label::LLM_FILES, label::EXPORT_BACKUP, label::IMPORT_BACKUP]
}

pub fn kept_features() -> [&'static str; 6] {
    [label::MERGE, label::UNMERGE, label::MERGE_GROUPS, label::APPEND_DISC, label::EMBED_BACK, label::PREFLIGHT]
}

pub fn removed_from_nav() -> [&'static str; 3] {
    [label::CD, "封面视频", "模型配置"]
}

pub fn groups() -> [(&'static str, &'static str, &'static str); 4] {
    [("all", "▤", "全部项目"), ("open", "◷", "整理中"), ("complete", "✓", "已完成"), ("repair", "◇", "待修复")]
}

pub fn project_columns() -> [&'static str; 7] {
    ["录音项目", "演出日期", "城市 / 音乐厅", "录音来源", "曲目", "状态", "操作"]
}

pub fn detail_tabs() -> [&'static str; 3] {
    ["项目概览", "分轨与标签", "项目历史"]
}

pub fn sorts() -> [(&'static str, &'static str); 4] {
    [("recent", "日期从近到远"), ("oldest", "日期从远到近"), ("name", "项目名称"), ("manual", "手动顺序")]
}

pub fn history_columns() -> [&'static str; 5] {
    ["时间", "项目", "操作", "结果", "详情"]
}

pub fn split_track_columns() -> [&'static str; 6] {
    ["顺序", "曲目名称", "音频来源", "起点", "终点", "试听与标签"]
}

pub fn detail_fields() -> [(&'static str, &'static str); 9] {
    [
        ("performer", "演奏家"),
        ("conductor", "指挥家"),
        ("composer", "作曲家"),
        ("date", "演出日期"),
        ("city", "城市"),
        ("venue", "音乐厅"),
        ("origin", "录音来源"),
        ("status", "项目状态"),
        ("orchestra", "乐团"),
    ]
}

pub struct Listing<'a> {
    pub name: &'a str,
    pub performer: &'a str,
    pub conductor: &'a str,
    pub composer: &'a str,
    pub date: &'a str,
    pub city: &'a str,
    pub venue: &'a str,
    pub origin: &'a str,
    pub status: &'a str,
    pub extra: &'a str,
}

pub struct ListingFilter<'a> {
    pub group: &'a str,
    pub query: &'a str,
    pub status: &'a str,
    pub origin: &'a str,
    pub date: &'a str,
    pub performer: &'a str,
    pub conductor: &'a str,
    pub composer: &'a str,
    pub city: &'a str,
    pub venue: &'a str,
}

pub fn listing_visible(row: &Listing<'_>, filter: &ListingFilter<'_>) -> bool {
    if !in_group(row.status, filter.group) {
        return false;
    }
    let exact = [
        (filter.status, row.status),
        (filter.origin, row.origin),
        (filter.date, row.date),
        (filter.performer, row.performer),
        (filter.conductor, row.conductor),
        (filter.city, row.city),
        (filter.venue, row.venue),
    ];
    if exact.iter().any(|(want, have)| !want.is_empty() && *want != *have) {
        return false;
    }
    if !filter.composer.is_empty() {
        let hit = row.composer.split(" / ").any(|part| part == filter.composer) || row.extra.lines().any(|part| part == filter.composer);
        if !hit {
            return false;
        }
    }
    let query = filter.query.trim().to_lowercase();
    if query.is_empty() {
        return true;
    }
    let hay = format!(
        "{} {} {} {} {} {} {} {} {} {}",
        row.name, row.performer, row.conductor, row.composer, row.date, row.city, row.venue, row.origin, row.status, row.extra
    )
    .to_lowercase();
    hay.contains(&query)
}

pub fn in_group(status: &str, group: &str) -> bool {
    match group {
        "all" | "" => true,
        "complete" => status == "已完成",
        "repair" => status == "处理失败" || status == "需修复",
        "open" => status != "已完成" && status != "处理失败" && status != "需修复",
        _ => false,
    }
}

pub fn script_preview(plan: &str) -> String {
    format!("# 古典录音工作台\n# {}\n{plan}", heading(Place::Scripts).1)
}

pub const SEEK_SECONDS: f64 = 5.0;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Mods {
    pub primary: bool,
    pub shift: bool,
    pub alt: bool,
    pub function: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Focus {
    pub typing: bool,
    pub dialog: bool,
    pub lookup: bool,
    pub preview: bool,
    pub composing: bool,
}

impl Focus {
    pub fn page() -> Self {
        Self { typing: false, dialog: false, lookup: false, preview: false, composing: false }
    }

    pub fn typing() -> Self {
        Self { typing: true, ..Self::page() }
    }

    pub fn dialog() -> Self {
        Self { dialog: true, ..Self::page() }
    }

    pub fn lookup() -> Self {
        Self { typing: true, dialog: true, lookup: true, ..Self::page() }
    }

    pub fn preview() -> Self {
        Self { preview: true, ..Self::page() }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Command {
    Confirm,
    Dismiss,
    PlayPause,
    SeekBack,
    SeekForward,
    Copy,
    Cut,
    Paste,
    SelectAll,
    Undo,
    Redo,
    Save,
}

pub fn resolve(key: &str, mods: Mods, focus: Focus) -> Option<Command> {
    if focus.composing || key.is_empty() {
        return None;
    }
    let key = key.to_ascii_lowercase();
    if focus.typing && mods.primary && !mods.alt && !mods.function {
        return match (key.as_str(), mods.shift) {
            ("c", false) => Some(Command::Copy),
            ("x", false) => Some(Command::Cut),
            ("v", false) => Some(Command::Paste),
            ("a", false) => Some(Command::SelectAll),
            ("z", false) => Some(Command::Undo),
            ("z", true) => Some(Command::Redo),
            ("y", false) => Some(Command::Redo),
            ("s", false) => Some(Command::Save),
            _ => None,
        };
    }
    if mods.primary && !mods.alt && !mods.shift && !mods.function && key == "s" {
        return Some(Command::Save);
    }
    if mods.primary || mods.alt || mods.shift || mods.function {
        return None;
    }
    match key.as_str() {
        "escape" | "esc" if focus.dialog => Some(Command::Dismiss),
        "enter" | "return" if focus.lookup => Some(Command::Confirm),
        "space" if focus.preview && !focus.typing && !focus.dialog => Some(Command::PlayPause),
        "left" | "arrowleft" if focus.preview && !focus.typing && !focus.dialog => Some(Command::SeekBack),
        "right" | "arrowright" if focus.preview && !focus.typing && !focus.dialog => Some(Command::SeekForward),
        _ => None,
    }
}

pub fn keeps_text_editing(command: Command) -> bool {
    matches!(command, Command::Copy | Command::Cut | Command::Paste | Command::SelectAll | Command::Undo | Command::Redo)
}

pub fn veto_close(dirty: bool) -> bool {
    dirty
}

pub fn nudge_playhead(current: f64, delta: f64) -> f64 {
    let next = current + delta;
    if next.is_finite() && next > 0.0 { next } else { 0.0 }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::Project;
    use crate::names::DEFAULT_NAMING;
    use crate::store;

    fn bare() -> Mods {
        Mods { primary: false, shift: false, alt: false, function: false }
    }

    fn press(key: &str, focus: Focus) -> Option<Command> {
        resolve(key, bare(), focus)
    }

    fn chord(key: &str, primary: bool, shift: bool, alt: bool, function: bool, focus: Focus) -> Option<Command> {
        resolve(key, Mods { primary, shift, alt, function }, focus)
    }

    #[test]
    fn navigation_matches_electron_0_2() {
        let labels = navigation().map(|item| item.label);
        assert_eq!(labels, ["录音项目", "整场录音分轨", "脚本生成", "处理记录", "模板与命名"]);
        let places = navigation().map(|item| item.place);
        assert_eq!(places, [Place::Projects, Place::Split, Place::Scripts, Place::History, Place::Templates]);
        let ids: Vec<_> = navigation().map(|item| item.id).to_vec();
        let mut unique = ids.clone();
        unique.sort_unstable();
        unique.dedup();
        assert_eq!(ids.len(), unique.len());
        for item in navigation() {
            assert_eq!(heading(item.place).0, item.label);
            assert!(!item.label.is_empty());
            assert!(!heading(item.place).1.is_empty());
        }
    }

    #[test]
    fn headings_are_the_electron_sentences() {
        assert_eq!(heading(Place::Projects).1, "归档演出录音，管理分轨、专辑标签、封面和处理进度。");
        assert_eq!(heading(Place::Split).1, "试听原始录音，标记起止点，使用 FFmpeg 分轨与处理。");
        assert_eq!(heading(Place::Scripts).1, "将所选音频和封面放在同一个文件夹中，在该文件夹打开 Windows PowerShell，粘贴脚本运行。");
        assert_eq!(heading(Place::History).1, "按操作与结果查看项目历史、错误和修订。");
        assert_eq!(heading(Place::Templates).1, "保存常用处理参数和英文文件命名规则。");
    }

    #[test]
    fn split_page_uses_the_numbered_sections() {
        assert_eq!(
            split_sections(),
            ["1. 导入录音", "2. 场次信息", "专辑封面", "3. 曲目与时间点", "4. 输出设置", "分轨结果"]
        );
    }

    #[test]
    fn header_tools_and_sidebar_match() {
        assert_eq!(header_tools(), ["LLM 配置", "模型识别文件", "↓ 导出备份", "↑ 导入备份"]);
        assert_eq!(label::SYNC, "本机工作区 · 已连接");
        assert_eq!(label::OPEN_PROJECT, "打开项目文件");
        assert_eq!(label::SAVE_LOCAL, "保存为本地文件");
        assert_eq!(label::NEW_PROJECT, "＋ 新建项目");
    }

    #[test]
    fn cd_video_and_model_are_not_sidebar_pages() {
        let labels: Vec<_> = navigation().map(|item| item.label).to_vec();
        for removed in removed_from_nav() {
            assert!(!labels.contains(&removed), "{removed}");
        }
        assert_eq!(label::CD, "CD 抓轨整理");
        assert_eq!(label::COVER_VIDEO, "制作封面视频");
        assert_eq!(label::LLM, "LLM 配置");
    }

    #[test]
    fn new_features_stay_available_and_out_of_the_nav() {
        let labels: Vec<_> = navigation().map(|item| item.label).to_vec();
        for feature in kept_features() {
            assert!(!labels.contains(&feature), "{feature}");
            assert!(!feature.is_empty());
        }
        assert_eq!(label::MERGE, "合并所选为一轨");
        assert_eq!(label::UNMERGE, "拆回乐章");
        assert_eq!(label::MERGE_GROUPS, "合并建议的乐章");
        assert_eq!(label::APPEND_DISC, "追加为新碟");
        assert_eq!(label::EMBED_BACK, "嵌入封底");
        assert_eq!(label::DISC_FOLDERS, "按碟分子文件夹");
        assert_eq!(label::JOIN, "终点留空时接到同一文件的下一轨起点，末轨到该文件结束");
    }

    #[test]
    fn splitter_buttons_use_the_electron_names() {
        assert_eq!(label::NEW_SESSION, "新建分轨场次");
        assert_eq!(label::IMPORT_AUDIO, "导入录音文件");
        assert_eq!(label::RECOGNIZE_AUDIO, "识别录音标签");
        assert_eq!(label::CONCAT, "按此顺序拼接");
        assert_eq!(label::BACK_FIVE, "↶ 后退 5 秒");
        assert_eq!(label::FORWARD_FIVE, "↷ 前进 5 秒");
        assert_eq!(label::PREVIEW_FROM, "从指定时间试听 20 秒");
        assert_eq!(label::RETURN_PREVIEW, "返回整场试听");
        assert_eq!(label::ENGLISH, "一键转英文并核对");
        assert_eq!(label::UPLOAD_COVER, "上传场照制作封面");
        assert_eq!(label::RUN, "运行 FFmpeg");
        assert_eq!(label::STOP, "停止处理");
        assert_eq!(label::PREVIEW_FX, "试听处理效果");
        assert_eq!(label::LOCAL_SCRIPT, "生成本机 PowerShell 脚本");
        assert_eq!(label::ADD_TRACK, "＋ 添加曲目");
        assert_eq!(label::SCRIPT_CARD, "PowerShell 预览");
        assert_eq!(label::SCRIPT_EMPTY, "请先创建录音项目。");
    }

    #[test]
    fn project_groups_follow_the_dashboard_stats() {
        assert_eq!(groups().map(|item| item.2), ["全部项目", "整理中", "已完成", "待修复"]);
        assert!(in_group("已完成", "all"));
        assert!(in_group("已完成", "complete"));
        assert!(!in_group("已完成", "open"));
        assert!(!in_group("已完成", "repair"));
        assert!(in_group("处理失败", "repair"));
        assert!(in_group("需修复", "repair"));
        assert!(!in_group("处理失败", "open"));
        assert!(in_group("整理中", "open"));
        assert!(in_group("待核对时间点", "open"));
        assert!(in_group("", "open"));
        assert!(!in_group("整理中", "complete"));
        assert!(!in_group("整理中", "nope"));
        assert!(in_group("已完成", ""));
    }

    #[test]
    fn typing_does_not_become_a_command() {
        for key in [
            "a", "b", "c", "q", "z", "y", "1", "0", "space", "left", "right", "enter", "return", "escape", "esc", "tab", "backspace",
            "delete", "up", "down", "home", "end", "f5", "pageup", "pagedown", "arrowleft", "arrowright",
        ] {
            assert_eq!(press(key, Focus::typing()), None, "{key}");
            assert_eq!(press(key, Focus::page()), None, "{key}");
        }
    }

    #[test]
    fn edit_menu_shortcuts_only_apply_while_typing() {
        let cases = [
            ("c", false, Command::Copy),
            ("x", false, Command::Cut),
            ("v", false, Command::Paste),
            ("a", false, Command::SelectAll),
            ("z", false, Command::Undo),
            ("z", true, Command::Redo),
            ("y", false, Command::Redo),
            ("C", false, Command::Copy),
            ("V", false, Command::Paste),
        ];
        for (key, shift, command) in cases {
            assert_eq!(chord(key, true, shift, false, false, Focus::typing()), Some(command), "{key}");
            assert_eq!(chord(key, true, shift, false, false, Focus::lookup()), Some(command), "{key}");
            assert_eq!(chord(key, true, shift, false, false, Focus::page()), None, "{key}");
            assert_eq!(chord(key, true, shift, false, false, Focus::preview()), None, "{key}");
            assert!(keeps_text_editing(command));
            assert_eq!(chord(key, true, shift, true, false, Focus::typing()), None, "{key}");
            assert_eq!(chord(key, true, shift, false, true, Focus::typing()), None, "{key}");
        }
        assert_eq!(chord("c", true, true, false, false, Focus::typing()), None);
        assert_eq!(chord("y", true, true, false, false, Focus::typing()), None);
        assert!(!keeps_text_editing(Command::Confirm));
        assert!(!keeps_text_editing(Command::Dismiss));
        assert!(!keeps_text_editing(Command::PlayPause));
        assert!(!keeps_text_editing(Command::SeekBack));
        assert!(!keeps_text_editing(Command::SeekForward));
    }

    #[test]
    fn enter_confirms_only_the_lookup_fields() {
        assert_eq!(press("Enter", Focus::lookup()), Some(Command::Confirm));
        assert_eq!(press("return", Focus::lookup()), Some(Command::Confirm));
        assert_eq!(press("enter", Focus { composing: true, ..Focus::lookup() }), None);
        assert_eq!(chord("enter", true, false, false, false, Focus::lookup()), None);
        assert_eq!(chord("enter", false, true, false, false, Focus::lookup()), None);
        assert_eq!(chord("enter", false, false, true, false, Focus::lookup()), None);
        assert_eq!(press("enter", Focus::dialog()), None);
        assert_eq!(press("enter", Focus::typing()), None);
        assert_eq!(press("enter", Focus::preview()), None);
        assert_eq!(press("enter", Focus::page()), None);
        assert_eq!(press("", Focus::lookup()), None);
    }

    #[test]
    fn escape_dismisses_a_dialog_even_while_typing() {
        assert_eq!(press("Escape", Focus::dialog()), Some(Command::Dismiss));
        assert_eq!(press("esc", Focus::dialog()), Some(Command::Dismiss));
        assert_eq!(press("escape", Focus { typing: true, ..Focus::dialog() }), Some(Command::Dismiss));
        assert_eq!(press("escape", Focus { composing: true, ..Focus::dialog() }), None);
        assert_eq!(chord("escape", true, false, false, false, Focus::dialog()), None);
        assert_eq!(chord("escape", false, false, true, false, Focus::dialog()), None);
        assert_eq!(press("escape", Focus::page()), None);
        assert_eq!(press("escape", Focus::typing()), None);
        assert_eq!(press("escape", Focus::preview()), None);
        assert_eq!(press("escape", Focus::lookup()), Some(Command::Dismiss));
        assert_eq!(chord("s", true, false, false, false, Focus::page()), Some(Command::Save));
        assert_eq!(chord("s", true, false, false, false, Focus::typing()), Some(Command::Save));
        assert!(!keeps_text_editing(Command::Save));
        assert!(veto_close(true));
        assert!(!veto_close(false));
    }

    #[test]
    fn media_keys_only_move_the_preview() {
        assert_eq!(SEEK_SECONDS, 5.0);
        assert_eq!(press("space", Focus::preview()), Some(Command::PlayPause));
        assert_eq!(press("Left", Focus::preview()), Some(Command::SeekBack));
        assert_eq!(press("right", Focus::preview()), Some(Command::SeekForward));
        assert_eq!(press("arrowleft", Focus::preview()), Some(Command::SeekBack));
        assert_eq!(press("arrowright", Focus::preview()), Some(Command::SeekForward));
        for focus in [Focus::typing(), Focus::page(), Focus::dialog(), Focus::lookup()] {
            assert_eq!(press("space", focus), None);
            assert_eq!(press("left", focus), None);
            assert_eq!(press("right", focus), None);
        }
        let blocked = Focus { dialog: true, ..Focus::preview() };
        assert_eq!(press("space", blocked), None);
        assert_eq!(press("left", blocked), None);
        assert_eq!(press("right", blocked), None);
        let typing = Focus { typing: true, ..Focus::preview() };
        assert_eq!(press("space", typing), None);
        assert_eq!(press("left", typing), None);
        assert_eq!(chord("left", true, false, false, false, Focus::preview()), None);
        assert_eq!(chord("right", false, true, false, false, Focus::preview()), None);
        assert_eq!(chord("space", false, false, true, false, Focus::preview()), None);
        assert_eq!(chord("left", false, false, false, true, Focus::preview()), None);
        assert_eq!(press("up", Focus::preview()), None);
        assert_eq!(press("down", Focus::preview()), None);
    }

    #[test]
    fn playhead_follows_the_five_second_buttons() {
        assert_eq!(nudge_playhead(12.0, SEEK_SECONDS), 17.0);
        assert_eq!(nudge_playhead(12.0, -SEEK_SECONDS), 7.0);
        assert_eq!(nudge_playhead(3.0, -SEEK_SECONDS), 0.0);
        assert_eq!(nudge_playhead(0.0, -SEEK_SECONDS), 0.0);
        assert_eq!(nudge_playhead(0.0, 0.0), 0.0);
        assert_eq!(nudge_playhead(1.25, 0.0), 1.25);
        assert_eq!(nudge_playhead(-4.0, 1.0), 0.0);
        assert_eq!(nudge_playhead(-4.0, 9.0), 5.0);
        assert_eq!(nudge_playhead(f64::NAN, 5.0), 0.0);
        assert_eq!(nudge_playhead(1.0, f64::NAN), 0.0);
        assert_eq!(nudge_playhead(f64::INFINITY, 1.0), 0.0);
        assert_eq!(nudge_playhead(1.0, f64::NEG_INFINITY), 0.0);
    }

    #[test]
    fn script_preview_keeps_the_electron_instruction() {
        let text = script_preview("ffmpeg -i concert.wav");
        assert!(text.starts_with("# 古典录音工作台\n"));
        assert!(text.contains(heading(Place::Scripts).1));
        assert!(text.contains("ffmpeg -i concert.wav"));
    }

    #[test]
    fn backup_roundtrip_uses_the_electron_wrapper() {
        let mut workspace = store::import_backup("{}").expect("raw workspace");
        assert_eq!(workspace.naming, DEFAULT_NAMING);
        assert!(workspace.projects.is_empty());
        let mut project = Project::blank();
        project.name = "备份场次".into();
        workspace.projects.push(project);
        let text = store::export_backup(&workspace).expect("export");
        assert!(text.contains("\"version\": 1"));
        assert!(text.contains("备份场次"));
        assert!(!text.contains("api_key"));
        assert!(!text.contains("sk-live"));
        let again = store::import_backup(&text).expect("import");
        assert_eq!(again.projects[0].name, "备份场次");
        assert_eq!(again.naming, workspace.naming);
        let custom = serde_json::json!({"version": 1, "data": {"naming": "自定义"}});
        let imported = store::import_backup(&custom.to_string()).expect("wrapped");
        assert_eq!(imported.naming, "自定义");
        let empty = serde_json::json!({"version": 1, "data": {}});
        assert!(store::import_backup(&empty.to_string()).is_ok());
    }

    #[test]
    fn backup_rejects_the_wrong_shape() {
        for sample in ["", "not-json", "[]", "null", "{\"version\":2,\"data\":{}}", "{\"data\":{}}", "{\"version\":1,\"data\":\"nope\"}", "{\"version\":\"1\",\"data\":{}}"] {
            assert!(store::import_backup(sample).is_err(), "{sample}");
        }
    }

    #[test]
    fn original_screen_uses_the_zip_layout() {
        assert_eq!(navigation()[2].mark, "›_");
        assert_eq!(
            project_columns(),
            ["录音项目", "演出日期", "城市 / 音乐厅", "录音来源", "曲目", "状态", "操作"]
        );
        assert_eq!(detail_tabs(), ["项目概览", "分轨与标签", "项目历史"]);
        assert_eq!(sorts()[3], ("manual", "手动顺序"));
        assert_eq!(history_columns(), ["时间", "项目", "操作", "结果", "详情"]);
        assert_eq!(split_track_columns()[0], "顺序");
        assert_eq!(split_track_columns()[5], "试听与标签");
        assert_eq!(detail_fields()[1], ("conductor", "指挥家"));
        assert_eq!(detail_fields()[7], ("status", "项目状态"));
        assert_eq!(project_track_columns()[1], "Title / Composer");
        assert_eq!(project_track_columns()[4], "英文文件名（不含扩展名）");
        assert_eq!(label::BACK_LIST, "← 返回项目列表");
        assert_eq!(label::OPEN_ROW, "打开 ↗");
        assert_eq!(label::OPEN_SPLIT, "在软件中分轨 ↗");
        assert_eq!(label::OPEN_SCRIPT, "生成脚本 ↗");
        assert_eq!(label::EMPTY_PROJECTS, "没有符合条件的录音项目。");
        assert_eq!(label::NAMING_CARD, "默认曲目命名规则");
        assert_eq!(label::SESSION_CARD, "演出与录音");
    }

    fn sample<'a>(status: &'a str) -> Listing<'a> {
        Listing {
            name: "贝多芬",
            performer: "陈韵集",
            conductor: "张指挥",
            composer: "Beethoven / Mozart",
            date: "2024-05-01",
            city: "上海",
            venue: "交响音乐厅",
            origin: "现场",
            status,
            extra: "Piano Sonata\nMozart",
        }
    }

    fn open_filter<'a>() -> ListingFilter<'a> {
        ListingFilter {
            group: "all",
            query: "",
            status: "",
            origin: "",
            date: "",
            performer: "",
            conductor: "",
            composer: "",
            city: "",
            venue: "",
        }
    }

    #[test]
    fn listing_uses_the_original_groups_and_fields() {
        let row = sample("整理中");
        assert!(listing_visible(&row, &open_filter()));
        let mut grouped = open_filter();
        grouped.group = "complete";
        assert!(!listing_visible(&row, &grouped));
        grouped.group = "open";
        assert!(listing_visible(&row, &grouped));
        let mut repair = sample("需修复");
        grouped.group = "repair";
        assert!(listing_visible(&repair, &grouped));
        repair.status = "已完成";
        assert!(!listing_visible(&repair, &grouped));

        let mut filtered = open_filter();
        filtered.city = "上海";
        assert!(listing_visible(&row, &filtered));
        filtered.city = "北京";
        assert!(!listing_visible(&row, &filtered));
        filtered.city = "";
        filtered.composer = "Mozart";
        assert!(listing_visible(&row, &filtered));
        filtered.composer = "Bach";
        assert!(!listing_visible(&row, &filtered));
        filtered.composer = "";
        filtered.query = "piano";
        assert!(listing_visible(&row, &filtered));
        filtered.query = "不存在";
        assert!(!listing_visible(&row, &filtered));
        filtered.query = "  ";
        assert!(listing_visible(&row, &filtered));
    }
}
