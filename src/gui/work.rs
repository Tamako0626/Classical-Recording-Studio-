//! 分轨, CD, 封面视频, 记录, 模板和模型页.

use gpui_kit::component::{
    Disableable as _, Selectable as _, Sizable as _,
    button::{Button, ButtonVariants as _},
    checkbox::Checkbox,
    form::{Field, Form},
    input::Input,
    label::Label,
    radio::RadioGroup,
    switch::Switch,
};
use gpui_kit::{AnyElement, Context, IntoElement, ParentElement as _, SharedString, Styled as _, Window, div, px};

use crate::catalog;
use crate::cue::{self, CueTrack};
use crate::effects;
use crate::export::{self, TrackReport};
use crate::ffmpeg;
use crate::llm::{self, LlmConfig};
use crate::mb;
use crate::model::{self, Project, Settings, Track, TAG_KEYS};
use crate::names;
use crate::video::{self, VideoMode};

use super::{set_text, text, JobMsg, Studio};

const AUDIO_EXT: &[&str] = &["wav", "flac", "mp3", "mkv", "ape", "m4a", "aac", "aiff", "aif", "ogg", "opus", "wv"];
const CD_EXT: &[&str] = &["wav", "flac", "mp3", "mkv", "ape", "m4a", "aac", "aiff", "aif", "ogg", "opus", "wv", "cue"];

#[derive(Clone, Copy)]
enum CdImport {
    Append,
    Replace,
    NextDisc,
}

impl Studio {
    pub(super) fn split_page(&mut self, cx: &mut Context<Self>) -> AnyElement {
        let Some(project) = self.project().cloned() else {
            return super::surface().child(Label::new("请先建立或打开一个项目.")).into_any_element();
        };
        let mut page = div().flex().flex_col().gap(px(16.));
        page = page
            .child(super::surface().child(Label::new("演出与录音")).child(session_form(self)))
            .child(super::surface().child(Label::new("输出")).child(output_form(self)))
            .child(super::surface().child(self.settings_block(&project.settings, cx)))
            .child(super::surface().child(self.effects_block(&project, cx)));
        page = page.child(super::surface().child(self.source_block(&project, cx))).child(super::surface().child(self.track_block(&project, cx))).child(super::surface().child(self.actions(cx)));
        if !self.preflight.is_empty() {
            page = page.child(super::surface().child(Label::new("导出预检")).child(preflight_lines(&self.preflight)));
        }
        if !self.mb_hits.is_empty() {
            page = page.child(super::surface().child(self.mb_block(cx)));
        }
        page.into_any_element()
    }

    fn settings_block(&self, settings: &Settings, cx: &mut Context<Self>) -> impl IntoElement {
        let format_index = ["FLAC", "WAV", "MP3", "MKV"].iter().position(|item| *item == settings.format);
        let rate_index = [44100, 48000, 96000].iter().position(|item| *item == settings.rate);
        div()
            .flex()
            .flex_col()
            .gap_2()
            .child(Label::new("输出"))
            .child(
                RadioGroup::new("format")
                    .children(["FLAC", "WAV", "MP3", "MKV"])
                    .selected_index(format_index)
                    .on_change(cx.listener(|this, index: &usize, _, cx| {
                        if let Some(project) = this.project_mut() {
                            let formats = ["FLAC", "WAV", "MP3", "MKV"];
                            project.settings.format = formats[*index].to_string();
                        }
                        this.mark_dirty(cx);
                    })),
            )
            .child(
                RadioGroup::new("rate")
                    .children(["44100", "48000", "96000"])
                    .selected_index(rate_index)
                    .on_change(cx.listener(|this, index, _, cx| {
                        if let Some(project) = this.project_mut() {
                            project.settings.rate = [44100, 48000, 96000][*index];
                        }
                        this.mark_dirty(cx);
                    })),
            )
            .child(div().flex().flex_row().gap_2().child(super::field("增益 dB", &self.gain)).child(super::field("声场", &self.width)).child(super::field("混响", &self.reverb)))
            .child(check("embed-cover", "嵌入封面", settings.cover, cx, |project, value| project.settings.cover = value))
            .child(check("split-tracks", "按时间点分轨", settings.split, cx, |project, value| project.settings.split = value))
            .child(check("write-tags", "写入标签", settings.metadata, cx, |project, value| project.settings.metadata = value))
            .child(check("embed-back", "嵌入封底", settings.embed_back, cx, |project, value| project.settings.embed_back = value))
            .child(check("auto-join", "空的终点接到下一轨起点", self.project().is_some_and(|project| project.auto_join), cx, |project, value| project.auto_join = value))
            .child(check("disc-folders", "按碟分子文件夹", self.project().is_some_and(|project| project.disc_folders), cx, |project, value| project.disc_folders = value))
            .child(
                div()
                    .flex()
                    .flex_row()
                    .gap_2()
                    .child(Button::new("pick-cover").small().label("选择封面").on_click(cx.listener(|this, _, window, cx| this.pick_cover(false, window, cx))))
                    .child(Button::new("pick-back").small().label("选择封底").on_click(cx.listener(|this, _, window, cx| this.pick_cover(true, window, cx))))
                    .child(Label::new(cover_label(self.project()))),
            )
    }

    fn effects_block(&self, project: &Project, cx: &mut Context<Self>) -> impl IntoElement {
        let effects = &project.settings.effects;
        let mut block = div().flex().flex_col().gap_2().child(
            Button::new("toggle-effects")
                .small()
                .ghost()
                .label(if self.effects_open { "收起后期处理" } else { "展开后期处理" })
                .on_click(cx.listener(|this, _, _, cx| {
                    this.effects_open = !this.effects_open;
                    cx.notify();
                })),
        );
        if !self.effects_open {
            return block;
        }
        block = block
            .child(mode_row("denoise", &effects.denoise, &[("off", "降噪关"), ("fft", "FFT"), ("nlm", "NLM")], cx, set_denoise))
            .child(mode_row("hum", &effects.hum, &[("off", "哼声关"), ("50", "50 Hz"), ("60", "60 Hz")], cx, set_hum))
            .child(mode_row("channels", &effects.channels, &[("source", "原声道"), ("mono", "单声道"), ("swap", "交换"), ("invert", "反转右声道")], cx, set_channels))
            .child(mode_row("normalize", &effects.normalize, &[("off", "响度关"), ("loud", "EBU"), ("dynamic", "动态")], cx, set_normalize))
            .child(check("declick", "去点击", effects.declick, cx, |project, value| project.settings.effects.declick = value))
            .child(check("declip", "去削波", effects.declip, cx, |project, value| project.settings.effects.declip = value))
            .child(check("deess", "齿音", effects.deess, cx, |project, value| project.settings.effects.deess = value))
            .child(
                div()
                    .flex()
                    .flex_row()
                    .gap_2()
                    .child(super::field("降噪", &self.noise_reduction))
                    .child(super::field("高通", &self.highpass))
                    .child(super::field("低通", &self.lowpass))
                    .child(super::field("低频", &self.bass))
                    .child(super::field("中频", &self.mid))
                    .child(super::field("高频", &self.treble)),
            )
            .child(
                div()
                    .flex()
                    .flex_row()
                    .gap_2()
                    .child(super::field("噪声门", &self.gate))
                    .child(super::field("压缩比", &self.compress))
                    .child(super::field("阈值", &self.threshold))
                    .child(super::field("LUFS", &self.lufs))
                    .child(super::field("淡入", &self.fade_in))
                    .child(super::field("淡出", &self.fade_out)),
            )
            .child(
                div().flex().flex_row().gap_2().items_center().child(
                    Switch::new("limiter")
                        .label("限制器")
                        .checked(self.limiter_on)
                        .on_change(cx.listener(|this, value, _, cx| {
                            this.limiter_on = *value;
                            this.pull(cx);
                            this.mark_dirty(cx);
                        })),
                ).child(Input::new(&self.limiter).small()),
            );
        block
    }

    fn source_block(&self, project: &Project, cx: &mut Context<Self>) -> impl IntoElement {
        let mut list = div().flex().flex_col().gap_1();
        if project.sources.is_empty() {
            list = list.child(Label::new("还没有录音."));
        }
        for (index, source) in project.sources.iter().enumerate() {
            let linked = project.source_paths.get(source).cloned().unwrap_or_else(|| "未关联".into());
            list = list.child(div().flex().flex_row().gap_2().items_center().child(Label::new(format!("{}. {source}", index + 1))).child(Label::new(linked)));
        }
        div()
            .flex()
            .flex_col()
            .gap_2()
            .child(Label::new("录音"))
            .child(
                div()
                    .flex()
                    .flex_row()
                    .gap_2()
                    .child(Button::new("add-audio").small().label("添加录音").on_click(cx.listener(|this, _, window, cx| this.add_audio(window, cx))))
                    .child(Button::new("relink-audio").small().label("重新关联当前曲目").on_click(cx.listener(|this, _, window, cx| this.relink(window, cx))))
                    .child(Button::new("tracks-from-sources").small().label("按录音各加一轨").on_click(cx.listener(|this, _, window, cx| this.tracks_from_sources(window, cx))))
                    .child(Button::new("concat-sources").small().label("合并为 FLAC").on_click(cx.listener(|this, _, window, cx| this.concat_sources(window, cx))))
                    .child(Button::new("relink-tree").small().label("按文件夹重新关联").on_click(cx.listener(|this, _, window, cx| this.relink_tree(window, cx))))
                    .child(Button::new("source-root").small().label("设置素材根目录").on_click(cx.listener(|this, _, window, cx| this.choose_root(window, cx)))),
            )
            .child(list)
    }

    fn track_block(&mut self, project: &Project, cx: &mut Context<Self>) -> impl IntoElement {
        let selected = self.track_index;
        let mut list = div().flex().flex_col().gap_1();
        for (index, track) in project.tracks.iter().enumerate() {
            let title = if track.tags.title.trim().is_empty() { "未命名曲目".to_string() } else { track.tags.title.clone() };
            let leaf = if track.source.is_empty() { "未选择".to_string() } else { names::source_leaf(&track.source) };
            let disc = if track.disc > 0 { format!("D{:02}  ", track.disc) } else { String::new() };
            let extra = if track.parts.len() > 1 {
                match names::parts_duration(&track.parts) {
                    Some(seconds) => format!("  ({} 段, {})", track.parts.len(), names::format_seconds(seconds)),
                    None => format!("  ({} 段)", track.parts.len()),
                }
            } else {
                String::new()
            };
            let label = format!("{disc}{:02}  {leaf}  {} - {title}{extra}", index + 1, track.start);
            let id = track.id.clone();
            let picked = self.track_picks.contains(&index);
            list = list.child(
                div()
                    .flex()
                    .flex_row()
                    .gap_2()
                    .items_center()
                    .child(Checkbox::new(SharedString::from(format!("pick-{id}"))).checked(picked).on_change(cx.listener(move |this, value: &bool, _, cx| {
                        this.pull(cx);
                        if *value {
                            this.track_picks.insert(index);
                        } else {
                            this.track_picks.remove(&index);
                        }
                        cx.notify();
                    })))
                    .child(
                        Button::new(SharedString::from(format!("track-{id}")))
                            .small()
                            .ghost()
                            .selected(selected == Some(index))
                            .label(label)
                            .on_click(cx.listener(move |this, _, window, cx| this.select_track(index, window, cx))),
                    ),
            );
        }
        let mut editor = div().flex().flex_col().gap_2();
        if selected.is_some() {
            editor = editor
                .child(div().flex().flex_row().gap_2().child(super::field("起点", &self.start)).child(super::field("终点", &self.end)).child(super::field("文件名", &self.filename)))
                .child(tag_form(self))
                .child(self.source_picker(project, cx));
        }
        div()
            .flex()
            .flex_col()
            .gap_2()
            .child(Label::new("曲目"))
            .child(
                div()
                    .flex()
                    .flex_row()
                    .gap_2()
                    .child(Button::new("add-track").small().label("添加曲目").on_click(cx.listener(|this, _, window, cx| this.add_track(window, cx))))
                    .child(Button::new("remove-track").small().danger().label("删除当前曲目").on_click(cx.listener(|this, _, window, cx| this.remove_track(window, cx))))
                    .child(Button::new("fill-tags").small().label("补齐空白标签").on_click(cx.listener(|this, _, window, cx| this.fill_tags(window, cx))))
                    .child(Button::new("apply-names").small().label("按规则填写空文件名").on_click(cx.listener(|this, _, window, cx| this.apply_names(window, cx))))
                    .child(Button::new("local-work").small().label("采用本地英文曲名").on_click(cx.listener(|this, _, window, cx| this.apply_catalog(window, cx))))
                    .child(Button::new("merge-tracks").small().label("合并所选为一轨").disabled(self.track_picks.len() < 2).on_click(cx.listener(|this, _, window, cx| {
                        this.merge_selected(window, cx);
                    })))
                    .child(Button::new("split-merged").small().label("拆回乐章").on_click(cx.listener(|this, _, window, cx| this.split_merged(window, cx))))
                    .child(Button::new("merge-groups").small().label("合并建议的乐章").on_click(cx.listener(|this, _, window, cx| this.merge_groups(window, cx))))
                    .child(Button::new("cycle-status").small().label(format!("状态: {}", project.status)).on_click(cx.listener(|this, _, _, cx| this.cycle_status(cx)))),
            )
            .child(group_block(project, cx))
            .child(list)
            .child(editor)
    }

    fn source_picker(&self, project: &Project, cx: &mut Context<Self>) -> impl IntoElement {
        let current = self.track_index.and_then(|index| project.tracks.get(index)).map(|track| track.source.clone()).unwrap_or_default();
        let mut row = div().flex().flex_row().gap_1();
        for source in &project.sources {
            let name = source.clone();
            let selected = name == current;
            let label = if name.contains('/') || name.contains('\\') { name.clone() } else { names::source_leaf(&name) };
            row = row.child(
                Button::new(SharedString::from(format!("use-{name}")))
                    .xsmall()
                    .selected(selected)
                    .label(label)
                    .on_click(cx.listener(move |this, _, _, cx| {
                        this.pull(cx);
                        if let Some(track) = this.track_mut() {
                            track.source = name.clone();
                        }
                        this.mark_dirty(cx);
                    })),
            );
        }
        row
    }

    fn actions(&self, cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .flex()
            .flex_row()
            .gap_2()
            .child(Button::new("preflight").label("导出前检查").disabled(self.busy).on_click(cx.listener(|this, _, _, cx| this.preflight(cx))))
            .child(Button::new("export-album").primary().label("导出专辑").loading(self.busy).disabled(self.busy).on_click(cx.listener(|this, _, window, cx| this.export_album(window, cx))))
            .child(Button::new("stop-job").danger().label("停止").disabled(!self.busy).on_click(cx.listener(|this, _, _, cx| this.stop_job(cx))))
            .child(Button::new("preview-raw").label("试听原声 20 秒").disabled(self.busy).on_click(cx.listener(|this, _, _, cx| this.preview(false, cx))))
            .child(Button::new("preview-wet").label("试听处理后 20 秒").disabled(self.busy).on_click(cx.listener(|this, _, _, cx| this.preview(true, cx))))
            .child(super::field("MusicBrainz", &self.mb_query))
            .child(Button::new("search-work").small().label("检索作品").disabled(self.busy).on_click(cx.listener(|this, _, _, cx| this.search_work(cx))))
    }

    fn mb_block(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let mut list = div().flex().flex_col().gap_1();
        for (index, hit) in self.mb_hits.iter().enumerate() {
            let title = hit.title.clone();
            let artist = hit.artist.clone();
            let label = format!("{title}  {artist}");
            list = list.child(
                Button::new(SharedString::from(format!("mb-{index}-{title}")))
                    .small()
                    .ghost()
                    .label(label)
                    .on_click(cx.listener(move |this, _, window, cx| this.use_work(&title, &artist, window, cx))),
            );
        }
        div().flex().flex_col().gap_1().child(Label::new("MusicBrainz")).child(list)
    }

    pub(super) fn cd_page(&mut self, cx: &mut Context<Self>) -> AnyElement {
        let mut discs = div().flex().flex_col().gap_1();
        for (index, disc) in self.cd_discs.iter().enumerate() {
            let issue = if disc.issue.is_empty() { String::new() } else { format!("  {}", disc.issue) };
            let label = format!("碟 {:02}  {}  {} 轨{issue}", disc.number, disc.label, disc.rows.len());
            discs = discs.child(
                Button::new(SharedString::from(format!("cd-disc-{index}")))
                    .small()
                    .ghost()
                    .selected(self.cd_disc == Some(index))
                    .label(label)
                    .on_click(cx.listener(move |this, _, window, cx| this.select_disc(index, window, cx))),
            );
        }
        let mut rows = div().flex().flex_col().gap_1();
        for (index, row) in self.cd_rows.iter().enumerate() {
            let label = format!("{:02}  {}  {}", row.number, names::source_leaf(&row.source), row.title);
            rows = rows.child(
                Button::new(SharedString::from(format!("cd-row-{index}")))
                    .small()
                    .ghost()
                    .selected(self.cd_index == Some(index))
                    .label(label)
                    .on_click(cx.listener(move |this, _, window, cx| this.select_cd(index, window, cx))),
            );
        }
        super::surface()
            .child(Label::new("一套 CD 可以一次加入多张整轨和对应 CUE, 封面封底全套共用."))
            .child(Label::new(self.cd_notice.clone()))
            .child(
                div()
                    .flex()
                    .flex_row()
                    .gap_2()
                    .child(Button::new("cd-files").label("批量添加文件").on_click(cx.listener(|this, _, window, cx| this.pick_cd(window, cx))))
                    .child(Button::new("cd-folder").label("添加文件夹").on_click(cx.listener(|this, _, window, cx| this.pick_cd_folder(window, cx))))
                    .child(Button::new("cd-paste").label("从剪贴板读取曲目").on_click(cx.listener(|this, _, window, cx| this.paste_tracks(window, cx))))
                    .child(Button::new("cd-up").small().label("上移碟").on_click(cx.listener(|this, _, window, cx| this.move_disc(-1, window, cx))))
                    .child(Button::new("cd-down").small().label("下移碟").on_click(cx.listener(|this, _, window, cx| this.move_disc(1, window, cx))))
                    .child(Button::new("cd-remove").small().danger().label("移除当前碟").on_click(cx.listener(|this, _, window, cx| this.remove_disc(window, cx)))),
            )
            .child(
                div()
                    .flex()
                    .flex_row()
                    .gap_2()
                    .child(Button::new("cd-cover").small().label("套装封面").on_click(cx.listener(|this, _, window, cx| {
                        this.ensure_project();
                        this.pick_cover(false, window, cx);
                    })))
                    .child(Button::new("cd-back").small().label("套装封底").on_click(cx.listener(|this, _, window, cx| {
                        this.ensure_project();
                        this.pick_cover(true, window, cx);
                    })))
                    .child(Label::new(cover_label(self.project())))
                    .child(Button::new("cd-cue").small().label("为当前碟指定 CUE").on_click(cx.listener(|this, _, window, cx| this.assign_cue(window, cx))))
                    .child(Button::new("cd-audio").small().label("为当前碟添加音频").on_click(cx.listener(|this, _, window, cx| this.add_disc_audio(window, cx)))),
            )
            .child(
                div()
                    .flex()
                    .flex_row()
                    .gap_2()
                    .child(Button::new("cd-import").primary().label("导入到当前项目").on_click(cx.listener(|this, _, window, cx| this.import_cd(CdImport::Append, window, cx))))
                    .child(Button::new("cd-next").label("追加为新碟").on_click(cx.listener(|this, _, window, cx| this.import_cd(CdImport::NextDisc, window, cx))))
                    .child(Button::new("cd-replace").danger().label("替换全部曲目").on_click(cx.listener(|this, _, window, cx| this.import_cd(CdImport::Replace, window, cx)))),
            )
            .child(Form::new().child(super::field("专辑", &self.cd_title)).child(super::field("演出者", &self.cd_artist)).child(super::field("作曲家", &self.cd_composer)).child(super::field("日期", &self.cd_date)))
            .child(Label::new("碟"))
            .child(discs)
            .child(Label::new("当前碟的曲目"))
            .child(rows)
            .child(Form::new().child(super::field("曲名", &self.cd_row_title)).child(super::field("演出者", &self.cd_row_artist)).child(super::field("作曲家", &self.cd_row_composer)).child(super::field("文件名", &self.cd_row_file)))
            .into_any_element()
    }

    pub(super) fn video_page(&mut self, cx: &mut Context<Self>) -> AnyElement {
        let mut files = div().flex().flex_col().gap_1();
        for path in &self.video_files {
            files = files.child(Label::new(path.display().to_string()));
        }
        super::surface()
            .child(Label::new("用一张静态封面和音轨做成 1920x1080 的 H.264 视频. 48 kHz 及以上的无损音轨在自动模式下使用 MKV."))
            .child(
                div()
                    .flex()
                    .flex_row()
                    .gap_2()
                    .child(mode_button("video-auto", "自动", self.video_mode == VideoMode::Auto, cx, VideoMode::Auto))
                    .child(mode_button("video-mp4", "MP4", self.video_mode == VideoMode::Mp4, cx, VideoMode::Mp4))
                    .child(mode_button("video-mkv", "MKV 48", self.video_mode == VideoMode::Mkv48, cx, VideoMode::Mkv48)),
            )
            .child(
                div()
                    .flex()
                    .flex_row()
                    .gap_2()
                    .child(Button::new("video-audio").label("选择音轨").on_click(cx.listener(|this, _, _, cx| this.pick_video_audio(cx))))
                    .child(Button::new("video-cover").label("选择封面").on_click(cx.listener(|this, _, _, cx| this.pick_video_cover(cx))))
                    .child(Button::new("video-run").primary().label("生成封面视频").loading(self.busy).disabled(self.busy).on_click(cx.listener(|this, _, window, cx| this.make_videos(window, cx))))
                    .child(Button::new("video-stop").danger().label("停止").disabled(!self.busy).on_click(cx.listener(|this, _, _, cx| this.stop_job(cx)))),
            )
            .child(Label::new(if self.video_cover.is_empty() { "未选择封面".into() } else { self.video_cover.clone() }))
            .child(files)
            .into_any_element()
    }

    pub(super) fn history_page(&self, cx: &mut Context<Self>) -> AnyElement {
        let current = self.project().map(|project| project.id.clone());
        let mut list = div().flex().flex_col().gap_2();
        for entry in self.workspace.logs.iter().rev().filter(|entry| current.as_ref().is_none_or(|id| entry.project_id == *id)).take(80) {
            list = list.child(Label::new(format!("{}  {}  {}  {}  {}", entry.time, entry.project_name, entry.operation, entry.result, entry.message)));
        }
        super::surface()
            .child(Label::new("只显示当前项目. 没有打开项目时显示全部."))
            .child(Button::new("clear-logs").small().danger().label("清除当前项目的记录").on_click(cx.listener(|this, _, _, cx| {
                if let Some(id) = this.project().map(|project| project.id.clone()) {
                    this.workspace.logs.retain(|entry| entry.project_id != id);
                } else {
                    this.workspace.logs.clear();
                }
                this.mark_dirty(cx);
            })))
            .child(list)
            .into_any_element()
    }

    pub(super) fn templates_page(&mut self, cx: &mut Context<Self>) -> AnyElement {
        let mut list = div().flex().flex_col().gap_2();
        for (index, template) in self.workspace.templates.iter().enumerate() {
            let name = template.name.clone();
            let id = template.id.clone();
            list = list.child(
                div().flex().flex_row().gap_2().items_center().child(Label::new(format!("{name}  {}", template.rule))).child(
                    Button::new(SharedString::from(format!("apply-{id}")))
                        .small()
                        .label("应用到当前项目")
                        .on_click(cx.listener(move |this, _, window, cx| this.apply_template(index, window, cx))),
                ),
            );
        }
        super::surface()
            .child(Label::new("模板保存格式, 采样率, 增益, 后期, 命名规则和输出文件夹."))
            .child(Button::new("save-template").primary().label("把当前项目存为模板").on_click(cx.listener(|this, _, _, cx| this.save_template(cx))))
            .child(list)
            .into_any_element()
    }

    pub(super) fn llm_page(&mut self, cx: &mut Context<Self>) -> AnyElement {
        let mut review = div().flex().flex_col().gap_2();
        for (index, item) in self.review.iter().enumerate() {
            let label = format!("{}: {} -> {}", item.label, item.before, item.after);
            review = review.child(
                Checkbox::new(SharedString::from(format!("review-{index}")))
                    .label(label)
                    .checked(item.apply)
                    .on_change(cx.listener(move |this, value, _, cx| {
                        if let Some(item) = this.review.get_mut(index) {
                            item.apply = *value;
                        }
                        cx.notify();
                    })),
            );
            if !item.note.is_empty() {
                review = review.child(Label::new(item.note.clone()));
            }
        }
        super::surface()
            .child(Label::new("API Key 只保存在本机, 不写入项目文件."))
            .child(Form::new().child(super::field("接口", &self.llm_base)).child(super::field("模型", &self.llm_model)).child(Field::new().label("API Key").child(Input::new(&self.llm_key).small().content_type(gpui_kit::component::input::InputContentType::Password).mask_toggle())))
            .child(
                div()
                    .flex()
                    .flex_row()
                    .gap_2()
                    .child(Button::new("save-llm").label("保存配置").on_click(cx.listener(|this, _, window, cx| this.save_llm(window, cx))))
                    .child(Button::new("list-models").label("读取模型").disabled(self.busy).on_click(cx.listener(|this, _, _, cx| this.list_models(cx))))
                    .child(Button::new("english").primary().label("生成英文场次").loading(self.busy).disabled(self.busy).on_click(cx.listener(|this, _, _, cx| this.request_english(cx))))
                    .child(Button::new("apply-english").label("应用选中项").on_click(cx.listener(|this, _, window, cx| this.apply_english(window, cx)))),
            )
            .child(review)
            .into_any_element()
    }

    fn select_track(&mut self, index: usize, window: &mut Window, cx: &mut Context<Self>) {
        self.pull(cx);
        self.track_index = Some(index);
        self.push_track(window, cx);
        cx.notify();
    }

    fn add_audio(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.pull(cx);
        let Some(files) = rfd::FileDialog::new().set_title("添加录音").add_filter("音频", AUDIO_EXT).pick_files() else {
            return;
        };
        let Some(project) = self.project_mut() else {
            self.show_error("请先建立项目.", cx);
            return;
        };
        for file in files {
            let Some(name) = file.file_name().map(|item| item.to_string_lossy().to_string()) else {
                continue;
            };
            if !names::audio_name(&name) {
                continue;
            }
            if !project.sources.iter().any(|source| source == &name) {
                project.sources.push(name.clone());
            }
            project.source_paths.insert(name.clone(), file.display().to_string());
            if project.tracks.len() == 1 && project.tracks[0].source.is_empty() {
                project.tracks[0].source = name;
            }
        }
        self.push_track(window, cx);
        self.mark_dirty(cx);
    }

    fn relink(&mut self, _window: &mut Window, cx: &mut Context<Self>) {
        self.pull(cx);
        let Some(source) = self.track_index.and_then(|index| self.project().and_then(|project| project.tracks.get(index)).map(|track| track.source.clone())) else {
            self.show_error("请先选择曲目.", cx);
            return;
        };
        if source.trim().is_empty() {
            self.show_error("当前曲目还没有录音名.", cx);
            return;
        }
        let Some(file) = rfd::FileDialog::new().set_title("重新关联录音").add_filter("音频", AUDIO_EXT).pick_file() else {
            return;
        };
        if let Some(project) = self.project_mut() {
            project.source_paths.insert(source, file.display().to_string());
        }
        self.status = "已重新关联录音.".into();
        self.mark_dirty(cx);
    }

    fn tracks_from_sources(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.pull(cx);
        let Some(project) = self.project_mut() else {
            return;
        };
        for source in project.sources.clone() {
            if project.tracks.iter().any(|track| track.source == source) {
                continue;
            }
            let mut track = Track::blank();
            track.source = source;
            project.tracks.push(track);
        }
        if self.track_index.is_none() && !self.project().is_some_and(|project| project.tracks.is_empty()) {
            self.track_index = Some(0);
        }
        self.push_track(window, cx);
        self.mark_dirty(cx);
    }

    fn add_track(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.pull(cx);
        let source = self
            .track_index
            .and_then(|index| self.project().and_then(|project| project.tracks.get(index)).map(|track| track.source.clone()))
            .unwrap_or_default();
        let Some(project) = self.project_mut() else {
            return;
        };
        let mut track = Track::blank();
        track.source = source;
        project.tracks.push(track);
        self.track_index = Some(project.tracks.len() - 1);
        self.push_track(window, cx);
        self.mark_dirty(cx);
    }

    fn remove_track(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.pull(cx);
        let Some(index) = self.track_index else {
            return;
        };
        let Some(project) = self.project_mut() else {
            return;
        };
        if project.tracks.len() <= 1 {
            project.tracks = vec![Track::blank()];
            self.track_index = Some(0);
        } else {
            project.tracks.remove(index);
            self.track_index = Some(index.min(project.tracks.len() - 1));
        }
        self.push_track(window, cx);
        self.mark_dirty(cx);
    }

    fn fill_tags(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.pull(cx);
        let Some(project) = self.project_mut() else {
            return;
        };
        names::apply_session_tags(project);
        self.push_track(window, cx);
        self.mark_dirty(cx);
    }

    fn apply_names(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.pull(cx);
        let Some(project) = self.project_mut() else {
            return;
        };
        let rule = names::effective_naming(project);
        for index in 0..project.tracks.len() {
            if !project.tracks[index].filename.trim().is_empty() {
                continue;
            }
            let tags = names::track_tags(project, &project.tracks[index], index);
            let name = names::filename_for(&rule, &project.tracks[index], &tags, index);
            project.tracks[index].filename = name;
        }
        self.push_track(window, cx);
        self.mark_dirty(cx);
    }

    fn apply_catalog(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.pull(cx);
        let title = self.track_index.and_then(|index| self.project().and_then(|project| project.tracks.get(index)).map(|track| track.tags.title.clone())).unwrap_or_default();
        let Some(hit) = catalog::suggest_work(&title) else {
            self.show_error("本地曲目表没有匹配.", cx);
            return;
        };
        if let Some(track) = self.track_mut() {
            track.tags.title = hit.title.into();
            if track.tags.composer.trim().is_empty() {
                track.tags.composer = hit.composer.into();
            }
        }
        self.push_track(window, cx);
        self.mark_dirty(cx);
    }

    fn cycle_status(&mut self, cx: &mut Context<Self>) {
        self.pull(cx);
        let Some(project) = self.project_mut() else {
            return;
        };
        let index = model::STATUSES.iter().position(|item| *item == project.status).unwrap_or(0);
        project.status = model::STATUSES[(index + 1) % model::STATUSES.len()].into();
        self.mark_dirty(cx);
    }

    fn pick_cover(&mut self, back: bool, _window: &mut Window, cx: &mut Context<Self>) {
        self.pull(cx);
        let Some(file) = rfd::FileDialog::new().set_title(if back { "选择封底" } else { "选择封面" }).add_filter("图片", &["png", "jpg", "jpeg"]).pick_file() else {
            return;
        };
        let leaf = file.file_name().map(|item| item.to_string_lossy().to_string()).unwrap_or_default();
        if !names::cover_name(&leaf) {
            self.show_error("封面须为 PNG 或 JPG.", cx);
            return;
        }
        let Some(project) = self.project_mut() else {
            return;
        };
        if back {
            project.back_cover_file = leaf;
            project.back_cover_path = file.display().to_string();
        } else {
            project.cover_file = leaf;
            project.cover_path = file.display().to_string();
            project.cover_status = "已选择".into();
        }
        self.mark_dirty(cx);
    }

    fn merge_selected(&mut self, window: &mut Window, cx: &mut Context<Self>) -> bool {
        self.pull(cx);
        let mut indexes: Vec<usize> = self.track_picks.iter().copied().collect();
        indexes.sort_unstable();
        indexes.dedup();
        let Some(project) = self.project().cloned() else {
            self.show_error("请先建立项目.", cx);
            return false;
        };
        let mut durations = std::collections::BTreeMap::new();
        if let Some(binaries) = &self.ffmpeg {
            for index in &indexes {
                let Some(track) = project.tracks.get(*index) else {
                    continue;
                };
                let sources = if track.parts.is_empty() {
                    vec![track.source.clone()]
                } else {
                    track.parts.iter().map(|part| part.source.clone()).collect::<Vec<_>>()
                };
                for source in sources {
                    if source.is_empty() || durations.contains_key(&source) {
                        continue;
                    }
                    let Ok(path) = export::resolve_source(&project, &source) else {
                        continue;
                    };
                    let Ok(info) = ffmpeg::probe(binaries, &path, &self.cancel) else {
                        continue;
                    };
                    if let Some(duration) = info.duration {
                        durations.insert(source, duration);
                    }
                }
            }
        }
        let planned = match names::plan_merge(&project.tracks, &indexes, &durations) {
            Ok(planned) => planned,
            Err(error) => {
                self.show_error(error.to_string(), cx);
                return false;
            }
        };
        let titles = indexes.iter().filter_map(|index| project.tracks.get(*index).map(|track| track.tags.title.clone())).collect::<Vec<_>>();
        let title = names::shared_title(&titles);
        let Some(first) = indexes.first().copied() else {
            return false;
        };
        let Some(seed) = project.tracks.get(first) else {
            return false;
        };
        let mut merged = seed.clone();
        merged.id = model::new_id();
        merged.filename.clear();
        if !title.is_empty() {
            merged.tags.title = title;
        }
        let segmented = matches!(planned, names::MergePlan::Segments { .. });
        match planned {
            names::MergePlan::Contiguous { source, start, end, parts } => {
                merged.source = source;
                merged.start = start;
                merged.end = end;
                merged.parts = parts;
            }
            names::MergePlan::Segments { parts } => {
                merged.source = parts.first().map(|part| part.source.clone()).unwrap_or_default();
                merged.start = parts.first().map(|part| part.start.clone()).unwrap_or_else(|| "00:00".into());
                merged.end = parts.last().map(|part| part.end.clone()).unwrap_or_default();
                merged.parts = parts;
            }
        }
        let part_count = merged.parts.len();
        let Some(project) = self.project_mut() else {
            return false;
        };
        for index in indexes.iter().rev() {
            if *index < project.tracks.len() {
                project.tracks.remove(*index);
            }
        }
        let insert_at = first.min(project.tracks.len());
        project.tracks.insert(insert_at, merged);
        self.track_index = Some(insert_at);
        self.track_picks.clear();
        self.track_picks.insert(insert_at);
        self.error = None;
        self.status = if segmented {
            format!("已合并 {part_count} 段. 导出时会拼接, 空隙不会保留.").into()
        } else {
            "已合并为一段连续区间.".into()
        };
        self.push_track(window, cx);
        self.mark_dirty(cx);
        true
    }

    fn split_merged(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.pull(cx);
        let Some(index) = self.track_index else {
            self.show_error("请先选择曲目.", cx);
            return;
        };
        let Some(track) = self.project().and_then(|project| project.tracks.get(index)).cloned() else {
            return;
        };
        if track.parts.len() < 2 {
            self.show_error("当前曲目不是合并后的乐章.", cx);
            return;
        }
        let restored = track
            .parts
            .iter()
            .map(|part| {
                let mut item = Track::blank();
                item.disc = track.disc;
                item.source = part.source.clone();
                item.start = if part.start.is_empty() { "00:00".into() } else { part.start.clone() };
                item.end = part.end.clone();
                item.tags = track.tags.clone();
                item.tags.title = if part.title.trim().is_empty() { track.tags.title.clone() } else { part.title.clone() };
                item.tags.track.clear();
                item
            })
            .collect::<Vec<_>>();
        let count = restored.len();
        let Some(project) = self.project_mut() else {
            return;
        };
        project.tracks.splice(index..=index, restored);
        self.track_index = Some(index);
        self.track_picks.clear();
        self.status = format!("已拆回 {count} 段乐章.").into();
        self.push_track(window, cx);
        self.mark_dirty(cx);
    }

    fn merge_groups(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.pull(cx);
        let Some(tracks) = self.project().map(|project| project.tracks.clone()) else {
            return;
        };
        let groups = names::suggest_movement_groups(&tracks);
        if groups.is_empty() {
            self.show_error("没有发现可合并的乐章.", cx);
            return;
        }
        for group in groups.into_iter().rev() {
            self.track_picks = group.indexes.into_iter().collect();
            if !self.merge_selected(window, cx) {
                return;
            }
        }
    }

    fn preflight(&mut self, cx: &mut Context<Self>) {
        self.pull(cx);
        let Some(project) = self.project() else {
            self.show_error("请先建立项目.", cx);
            return;
        };
        match export::prepare(project) {
            Ok(plan) => {
                self.preflight = export::describe_plan(&plan);
                self.error = None;
                self.status = "已生成导出预检.".into();
            }
            Err(error) => {
                self.preflight.clear();
                self.show_error(error.to_string(), cx);
            }
        }
        cx.notify();
    }

    fn relink_tree(&mut self, _window: &mut Window, cx: &mut Context<Self>) {
        self.pull(cx);
        let Some(folder) = rfd::FileDialog::new().set_title("选择录音所在文件夹").pick_folder() else {
            return;
        };
        let Some(project) = self.project_mut() else {
            self.show_error("请先建立项目.", cx);
            return;
        };
        match export::relink_directory(project, &folder) {
            Ok(count) => self.status = format!("已重新关联 {count} 个录音.").into(),
            Err(error) => self.show_error(error.to_string(), cx),
        }
        self.mark_dirty(cx);
    }

    fn choose_root(&mut self, _window: &mut Window, cx: &mut Context<Self>) {
        self.pull(cx);
        let Some(folder) = rfd::FileDialog::new().set_title("选择素材根目录").pick_folder() else {
            return;
        };
        let Some(project) = self.project_mut() else {
            self.show_error("请先建立项目.", cx);
            return;
        };
        project.source_root = folder.display().to_string();
        for path in project.source_paths.values_mut() {
            let file = std::path::PathBuf::from(path.as_str());
            if let Ok(relative) = file.strip_prefix(&folder) {
                *path = relative.to_string_lossy().replace('\\', "/");
            }
        }
        self.status = format!("素材根目录 {}", folder.display()).into();
        self.mark_dirty(cx);
    }

    fn export_album(&mut self, _window: &mut Window, cx: &mut Context<Self>) {
        self.pull(cx);
        if self.busy {
            return;
        }
        let Some(binaries) = self.ffmpeg.clone() else {
            self.show_error("未找到裁剪版 FFmpeg.", cx);
            return;
        };
        let Some(project) = self.project().cloned() else {
            self.show_error("请先建立项目.", cx);
            return;
        };
        if let Err(error) = effects::filter_chain(&project.settings, Some(60.0), 2) {
            if project.settings.effects.fade_out == 0.0 || !error.to_string().contains("淡出") {
                self.show_error(error.to_string(), cx);
                return;
            }
        }
        let plan = match export::prepare(&project) {
            Ok(plan) => plan,
            Err(error) => {
                self.show_error(error.to_string(), cx);
                return;
            }
        };
        let Some(parent) = rfd::FileDialog::new().set_title("选择导出位置").pick_folder() else {
            return;
        };
        let note = plan.note.clone();
        self.cancel.reset();
        let cancel = self.cancel.clone();
        let (tx, rx) = async_channel::unbounded();
        self.follow(rx, cx);
        self.spawn_job(cx, move || match export::export_album(&binaries, &plan, &parent, &cancel, &tx) {
            Ok(reports) => {
                let mut message = super::reports_message(&reports);
                if !note.is_empty() {
                    message = format!("{note}\n{message}");
                }
                JobMsg::Done { operation: "导出专辑", result: outcome(&reports, message) }
            }
            Err(error) => JobMsg::Done { operation: "导出专辑", result: Err(error.to_string()) },
        });
    }

    fn concat_sources(&mut self, _window: &mut Window, cx: &mut Context<Self>) {
        self.pull(cx);
        if self.busy {
            return;
        }
        let Some(binaries) = self.ffmpeg.clone() else {
            self.show_error("未找到裁剪版 FFmpeg.", cx);
            return;
        };
        let Some(project) = self.project().cloned() else {
            return;
        };
        let mut paths = Vec::new();
        for source in &project.sources {
            match export::resolve_source(&project, source) {
                Ok(path) => paths.push(path),
                Err(error) => {
                    self.show_error(error.to_string(), cx);
                    return;
                }
            }
        }
        let Some(output) = rfd::FileDialog::new().set_title("保存合并后的 FLAC").set_file_name("joined.flac").save_file() else {
            return;
        };
        self.cancel.reset();
        let cancel = self.cancel.clone();
        self.spawn_job(cx, move || match export::concatenate(&binaries, &paths, &output, &cancel) {
            Ok(info) => JobMsg::Done {
                operation: "合并录音",
                result: Ok(format!("已写出 {}, {} Hz.", output.display(), info.rate)),
            },
            Err(error) => JobMsg::Done { operation: "合并录音", result: Err(error.to_string()) },
        });
    }

    fn preview(&mut self, processed: bool, cx: &mut Context<Self>) {
        self.pull(cx);
        if self.busy {
            return;
        }
        let Some(binaries) = self.ffmpeg.clone() else {
            self.show_error("未找到裁剪版 FFmpeg.", cx);
            return;
        };
        let Some(project) = self.project().cloned() else {
            return;
        };
        let Some(track) = self.track_index.and_then(|index| project.tracks.get(index)).cloned() else {
            self.show_error("请先选择曲目.", cx);
            return;
        };
        let source = match export::resolve_source(&project, &track.source) {
            Ok(path) => path,
            Err(error) => {
                self.show_error(error.to_string(), cx);
                return;
            }
        };
        let start = match names::time_seconds(&track.start) {
            Ok(value) => value.unwrap_or(0.0),
            Err(error) => {
                self.show_error(error.to_string(), cx);
                return;
            }
        };
        self.cancel.reset();
        let cancel = self.cancel.clone();
        let settings = project.settings.clone();
        self.spawn_job(cx, move || {
            let played = (|| {
                let info = ffmpeg::probe(&binaries, &source, &cancel)?;
                let filters = if processed { effects::filter_chain(&settings, info.duration, info.channels)? } else { Vec::new() };
                export::preview_span(&binaries, &source, start, 20.0, &filters, settings.rate, &cancel)
            })();
            JobMsg::Play(played.map_err(|error| error.to_string()))
        });
    }

    fn stop_job(&mut self, cx: &mut Context<Self>) {
        self.cancel.request();
        self.player.stop();
        self.status = "已请求停止.".into();
        cx.notify();
    }

    fn search_work(&mut self, cx: &mut Context<Self>) {
        if self.busy {
            return;
        }
        let query = text(&self.mb_query, cx);
        self.spawn_job(cx, move || match mb::search_works(&query) {
            Ok(hits) => JobMsg::Works(Ok(hits)),
            Err(error) => JobMsg::Works(Err(error.to_string())),
        });
    }

    fn use_work(&mut self, title: &str, artist: &str, window: &mut Window, cx: &mut Context<Self>) {
        self.pull(cx);
        if let Some(track) = self.track_mut() {
            track.tags.title = title.to_string();
            if track.tags.composer.trim().is_empty() {
                track.tags.composer = artist.to_string();
            }
        }
        self.push_track(window, cx);
        self.mark_dirty(cx);
    }

    fn follow(&mut self, rx: async_channel::Receiver<String>, cx: &mut Context<Self>) {
        self.progress_task = Some(cx.spawn(async move |this, cx| {
            while let Ok(text) = rx.recv().await {
                this.update(cx, |this, cx| {
                    this.status = text.into();
                    cx.notify();
                })
                .ok();
            }
        }));
    }

    fn pick_cd(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(files) = rfd::FileDialog::new().set_title("选择音频和 CUE").add_filter("音频或 CUE", CD_EXT).pick_files() else {
            return;
        };
        self.add_cd_paths(files, window, cx);
    }

    fn pick_cd_folder(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(folder) = rfd::FileDialog::new().set_title("选择 CD 文件夹").pick_folder() else {
            return;
        };
        self.add_cd_paths(vec![folder], window, cx);
    }

    fn add_cd_paths(&mut self, paths: Vec<std::path::PathBuf>, window: &mut Window, cx: &mut Context<Self>) {
        self.pull_cd(cx);
        self.store_disc();
        let entries = cue::gather_entries(&paths);
        let fresh = cue::collect_discs(&entries);
        if fresh.is_empty() {
            self.show_error("没有找到音频或 CUE.", cx);
            return;
        }
        let base = self.cd_discs.iter().map(|disc| disc.number).max().unwrap_or(0);
        for mut disc in fresh {
            if !disc.cue_path.is_empty() && self.cd_discs.iter().any(|item| item.cue_path == disc.cue_path) {
                continue;
            }
            if base > 0 {
                disc.number = disc.number.saturating_add(base);
            }
            self.cd_discs.push(disc);
        }
        if self.cd_disc.is_none() && !self.cd_discs.is_empty() {
            self.cd_disc = Some(self.cd_discs.len() - 1);
        }
        self.fill_suite(window, cx);
        self.load_disc(window, cx);
        self.cd_notice = format!("已读取 {} 张碟.", self.cd_discs.len()).into();
        cx.notify();
    }

    fn paste_tracks(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(item) = cx.read_from_clipboard() else {
            self.show_error("剪贴板是空的.", cx);
            return;
        };
        let Some(raw) = item.text() else {
            self.show_error("剪贴板里没有文字.", cx);
            return;
        };
        let listed = cue::parse_track_list(&raw);
        if !listed.issues.is_empty() {
            self.cd_notice = listed.issues.join(" ").into();
        }
        self.show_album(&listed.album, window, cx);
        let rows = if self.cd_files.is_empty() {
            listed
                .ordered
                .iter()
                .enumerate()
                .map(|(index, (title, composer))| CueTrack {
                    number: (index + 1) as u32,
                    source: String::new(),
                    path: String::new(),
                    start: "00:00".into(),
                    end: String::new(),
                    title: title.clone(),
                    artist: listed.album.artist.clone(),
                    composer: if composer.is_empty() { listed.album.composer.clone() } else { composer.clone() },
                    filename: String::new(),
                })
                .collect()
        } else {
            self.cd_files
                .iter()
                .enumerate()
                .map(|(index, (name, path))| {
                    let number = (index + 1) as u32;
                    let pair = listed.numbered.get(&number).cloned().or_else(|| listed.ordered.get(index).cloned()).unwrap_or_default();
                    CueTrack {
                        number,
                        source: name.clone(),
                        path: path.clone(),
                        start: "00:00".into(),
                        end: String::new(),
                        title: pair.0,
                        artist: listed.album.artist.clone(),
                        composer: if pair.1.is_empty() { listed.album.composer.clone() } else { pair.1 },
                        filename: String::new(),
                    }
                })
                .collect()
        };
        let mut draft = cue::DiscDraft {
            number: 1,
            label: "曲目文字".into(),
            cue_path: String::new(),
            directory: String::new(),
            files: self.cd_files.clone(),
            album: listed.album.clone(),
            rows,
            issue: listed.issues.join(" "),
        };
        if let Some(index) = self.cd_disc {
            if let Some(disc) = self.cd_discs.get_mut(index) {
                draft.number = disc.number;
                draft.files = if disc.files.is_empty() { draft.files } else { disc.files.clone() };
                *disc = draft;
            }
        } else {
            self.cd_discs.push(draft);
            self.cd_disc = Some(0);
        }
        self.load_disc(window, cx);
        cx.notify();
    }

    fn show_album(&mut self, album: &cue::CueAlbum, window: &mut Window, cx: &mut Context<Self>) {
        if !album.title.is_empty() {
            set_text(&self.cd_title, &album.title, window, cx);
        }
        if !album.artist.is_empty() {
            set_text(&self.cd_artist, &album.artist, window, cx);
        }
        if !album.composer.is_empty() {
            set_text(&self.cd_composer, &album.composer, window, cx);
        }
        if !album.date.is_empty() {
            set_text(&self.cd_date, &album.date, window, cx);
        }
    }

    fn select_cd(&mut self, index: usize, window: &mut Window, cx: &mut Context<Self>) {
        self.pull_cd(cx);
        self.cd_index = Some(index);
        self.push_cd(window, cx);
        cx.notify();
    }

    fn pull_cd(&mut self, cx: &mut Context<Self>) {
        let Some(index) = self.cd_index else {
            return;
        };
        let Some(row) = self.cd_rows.get_mut(index) else {
            return;
        };
        row.title = text(&self.cd_row_title, cx);
        row.artist = text(&self.cd_row_artist, cx);
        row.composer = text(&self.cd_row_composer, cx);
        row.filename = text(&self.cd_row_file, cx);
    }

    fn push_cd(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let row = self.cd_index.and_then(|index| self.cd_rows.get(index)).cloned().unwrap_or(CueTrack {
            number: 0,
            source: String::new(),
            path: String::new(),
            start: String::new(),
            end: String::new(),
            title: String::new(),
            artist: String::new(),
            composer: String::new(),
            filename: String::new(),
        });
        set_text(&self.cd_row_title, &row.title, window, cx);
        set_text(&self.cd_row_artist, &row.artist, window, cx);
        set_text(&self.cd_row_composer, &row.composer, window, cx);
        set_text(&self.cd_row_file, &row.filename, window, cx);
    }

    fn select_disc(&mut self, index: usize, window: &mut Window, cx: &mut Context<Self>) {
        self.pull_cd(cx);
        self.store_disc();
        self.cd_disc = Some(index);
        self.load_disc(window, cx);
    }

    fn store_disc(&mut self) {
        let Some(index) = self.cd_disc else {
            return;
        };
        if let Some(disc) = self.cd_discs.get_mut(index) {
            disc.rows = self.cd_rows.clone();
        }
    }

    fn load_disc(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let disc = self.cd_disc.and_then(|index| self.cd_discs.get(index)).cloned();
        if let Some(disc) = disc {
            self.cd_files = disc.files;
            self.cd_rows = disc.rows;
            if !disc.issue.is_empty() {
                self.cd_notice = disc.issue.into();
            }
        } else {
            self.cd_files.clear();
            self.cd_rows.clear();
        }
        self.cd_index = if self.cd_rows.is_empty() { None } else { Some(0) };
        self.push_cd(window, cx);
        cx.notify();
    }

    fn fill_suite(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let albums: Vec<cue::CueAlbum> = self.cd_discs.iter().map(|disc| disc.album.clone()).collect();
        let title = cue::suite_title(&albums);
        let artist = albums.iter().find(|album| !album.artist.is_empty()).map(|album| album.artist.clone()).unwrap_or_default();
        let composer = albums.iter().find(|album| !album.composer.is_empty()).map(|album| album.composer.clone()).unwrap_or_default();
        let date = albums.iter().find(|album| !album.date.is_empty()).map(|album| album.date.clone()).unwrap_or_default();
        if text(&self.cd_title, cx).trim().is_empty() && !title.is_empty() {
            set_text(&self.cd_title, &title, window, cx);
        }
        if text(&self.cd_artist, cx).trim().is_empty() && !artist.is_empty() {
            set_text(&self.cd_artist, &artist, window, cx);
        }
        if text(&self.cd_composer, cx).trim().is_empty() && !composer.is_empty() {
            set_text(&self.cd_composer, &composer, window, cx);
        }
        if text(&self.cd_date, cx).trim().is_empty() && !date.is_empty() {
            set_text(&self.cd_date, &date, window, cx);
        }
    }

    fn move_disc(&mut self, delta: isize, window: &mut Window, cx: &mut Context<Self>) {
        self.pull_cd(cx);
        self.store_disc();
        let Some(index) = self.cd_disc else {
            return;
        };
        let next = index as isize + delta;
        if next < 0 || next as usize >= self.cd_discs.len() {
            return;
        }
        self.cd_discs.swap(index, next as usize);
        self.cd_disc = Some(next as usize);
        for (order, disc) in self.cd_discs.iter_mut().enumerate() {
            disc.number = (order + 1) as u32;
        }
        self.load_disc(window, cx);
    }

    fn remove_disc(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.pull_cd(cx);
        let Some(index) = self.cd_disc else {
            return;
        };
        if index < self.cd_discs.len() {
            self.cd_discs.remove(index);
        }
        for (order, disc) in self.cd_discs.iter_mut().enumerate() {
            disc.number = (order + 1) as u32;
        }
        self.cd_disc = if self.cd_discs.is_empty() { None } else { Some(index.min(self.cd_discs.len() - 1)) };
        self.load_disc(window, cx);
    }

    fn assign_cue(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.pull_cd(cx);
        self.store_disc();
        let Some(index) = self.cd_disc else {
            self.show_error("请先选择一张碟.", cx);
            return;
        };
        let Some(file) = rfd::FileDialog::new().set_title("选择 CUE").add_filter("CUE", &["cue"]).pick_file() else {
            return;
        };
        let files = self.cd_discs.get(index).map(|disc| disc.files.clone()).unwrap_or_default();
        match cue::parse_disc_cue(&file, &files) {
            Ok((album, rows)) => {
                if let Some(disc) = self.cd_discs.get_mut(index) {
                    disc.cue_path = file.display().to_string();
                    disc.album = album;
                    disc.rows = rows;
                    disc.issue.clear();
                }
            }
            Err(error) => {
                if let Some(disc) = self.cd_discs.get_mut(index) {
                    disc.cue_path = file.display().to_string();
                    disc.issue = error.to_string();
                }
            }
        }
        self.fill_suite(window, cx);
        self.load_disc(window, cx);
    }

    fn add_disc_audio(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.pull_cd(cx);
        self.store_disc();
        let Some(index) = self.cd_disc else {
            self.show_error("请先选择一张碟.", cx);
            return;
        };
        let Some(files) = rfd::FileDialog::new().set_title("选择音频").add_filter("音频", AUDIO_EXT).pick_files() else {
            return;
        };
        if let Some(disc) = self.cd_discs.get_mut(index) {
            for file in files {
                let Some(name) = file.file_name().map(|item| item.to_string_lossy().to_string()) else {
                    continue;
                };
                if names::audio_name(&name) && !disc.files.iter().any(|(item, _)| item == &name) {
                    disc.files.push((name, file.display().to_string()));
                }
            }
        }
        let cue_path = self.cd_discs.get(index).map(|disc| disc.cue_path.clone()).unwrap_or_default();
        let audio = self.cd_discs.get(index).map(|disc| disc.files.clone()).unwrap_or_default();
        if !cue_path.is_empty() {
            match cue::parse_disc_cue(std::path::Path::new(&cue_path), &audio) {
                Ok((album, rows)) => {
                    if let Some(disc) = self.cd_discs.get_mut(index) {
                        disc.album = album;
                        disc.rows = rows;
                        disc.issue.clear();
                    }
                }
                Err(error) => {
                    if let Some(disc) = self.cd_discs.get_mut(index) {
                        disc.issue = error.to_string();
                    }
                }
            }
        } else if let Some(disc) = self.cd_discs.get_mut(index) {
            disc.issue = "未配对 CUE.".into();
        }
        self.load_disc(window, cx);
    }

    fn ensure_project(&mut self) {
        if self.project().is_none() {
            self.workspace.projects.push(Project::blank());
            self.project_index = Some(self.workspace.projects.len() - 1);
        }
    }

    fn import_cd(&mut self, mode: CdImport, window: &mut Window, cx: &mut Context<Self>) {
        self.pull(cx);
        self.pull_cd(cx);
        self.store_disc();
        if self.cd_discs.iter().all(|disc| disc.rows.is_empty()) {
            self.show_error("没有可导入的曲目. 请先配对 CUE.", cx);
            return;
        }
        self.ensure_project();
        let album_title = text(&self.cd_title, cx);
        let album_artist = text(&self.cd_artist, cx);
        let album_composer = text(&self.cd_composer, cx);
        let album_date = text(&self.cd_date, cx);
        let discs = self.cd_discs.clone();
        let Some(project) = self.project_mut() else {
            return;
        };
        if project.name.trim().is_empty() {
            project.name = album_title;
        }
        if project.performer.trim().is_empty() {
            project.performer = album_artist.clone();
        }
        if project.composer.trim().is_empty() {
            project.composer = album_composer.clone();
        }
        if project.date.trim().is_empty() {
            project.date = album_date.clone();
        }
        let shift = match mode {
            CdImport::Replace => {
                project.tracks.clear();
                project.sources.clear();
                project.source_paths.clear();
                0
            }
            CdImport::NextDisc => project.tracks.iter().map(|track| track.disc).max().unwrap_or(0),
            CdImport::Append => {
                let blank = project.tracks.len() == 1 && project.tracks[0].source.is_empty() && project.tracks[0].tags.title.trim().is_empty() && project.tracks[0].parts.is_empty();
                if blank {
                    project.tracks.clear();
                }
                0
            }
        };
        let mut imported = 0usize;
        let mut skipped = 0usize;
        for disc in discs {
            if disc.rows.is_empty() {
                skipped += 1;
                continue;
            }
            let number = disc.number.saturating_add(shift);
            for row in disc.rows {
                let source = if row.source.is_empty() { String::new() } else { names::source_id(number, &row.source) };
                if !source.is_empty() && !project.sources.iter().any(|item| item == &source) {
                    project.sources.push(source.clone());
                }
                if !source.is_empty() && !row.path.is_empty() {
                    project.source_paths.insert(source.clone(), row.path.clone());
                }
                let mut track = Track::blank();
                track.source = source;
                track.disc = number;
                track.start = if row.start.is_empty() { "00:00".into() } else { row.start };
                track.end = row.end;
                track.filename = row.filename;
                track.tags.title = row.title;
                track.tags.artist = if row.artist.is_empty() { album_artist.clone() } else { row.artist };
                track.tags.composer = if row.composer.is_empty() { album_composer.clone() } else { row.composer };
                track.tags.album = project.name.clone();
                track.tags.date = album_date.clone();
                if number > 0 {
                    track.tags.track = format!("{:02}", row.number);
                    track.tags.disc = format!("{:02}", number);
                }
                project.tracks.push(track);
                imported += 1;
            }
        }
        if project.tracks.is_empty() {
            project.tracks.push(Track::blank());
        }
        let max_disc = project.tracks.iter().map(|track| track.disc).max().unwrap_or(0);
        project.disc_count = if matches!(mode, CdImport::Replace) { max_disc } else { project.disc_count.max(max_disc) };
        if project.disc_count > 1 && (project.naming_rule.trim().is_empty() || project.naming_rule.trim() == names::DEFAULT_NAMING) {
            project.naming_rule = "{Disc}-{Track} - {Title}".into();
        }
        self.track_index = Some(0);
        self.track_picks.clear();
        self.page = super::Page::Split;
        self.push(window, cx);
        self.status = format!("已导入 {imported} 条曲目, 跳过 {skipped} 张未配对的碟.").into();
        self.mark_dirty(cx);
    }

    fn pick_video_audio(&mut self, cx: &mut Context<Self>) {
        let Some(files) = rfd::FileDialog::new().set_title("选择音轨").add_filter("音频", AUDIO_EXT).pick_files() else {
            return;
        };
        self.video_files = files;
        cx.notify();
    }

    fn pick_video_cover(&mut self, cx: &mut Context<Self>) {
        let Some(file) = rfd::FileDialog::new().set_title("选择封面").add_filter("图片", &["png", "jpg", "jpeg"]).pick_file() else {
            return;
        };
        self.video_cover = file.display().to_string().into();
        cx.notify();
    }

    fn make_videos(&mut self, _window: &mut Window, cx: &mut Context<Self>) {
        if self.busy {
            return;
        }
        let Some(binaries) = self.ffmpeg.clone() else {
            self.show_error("未找到裁剪版 FFmpeg.", cx);
            return;
        };
        if self.video_files.is_empty() || self.video_cover.is_empty() {
            self.show_error("请选择音轨和封面.", cx);
            return;
        }
        let Some(folder) = rfd::FileDialog::new().set_title("选择视频输出位置").pick_folder() else {
            return;
        };
        let sources = self.video_files.clone();
        let cover = std::path::PathBuf::from(self.video_cover.to_string());
        let mode = self.video_mode;
        self.cancel.reset();
        let cancel = self.cancel.clone();
        let (tx, rx) = async_channel::unbounded();
        self.follow(rx, cx);
        self.spawn_job(cx, move || match video::make_cover_videos(&binaries, &sources, &cover, &folder, mode, &cancel, &tx) {
            Ok(results) => {
                let ok = results.iter().filter(|(_, item)| item.is_ok()).count();
                let mut lines = vec![format!("封面视频完成 {ok} 条, 失败 {} 条.", results.len() - ok)];
                for (name, item) in results.iter().filter(|(_, item)| item.is_err()).take(8) {
                    lines.push(format!("{name}: {}", item.as_ref().err().map(|error| error.to_string()).unwrap_or_default()));
                }
                JobMsg::Done { operation: "封面视频", result: Ok(lines.join("\n")) }
            }
            Err(error) => JobMsg::Done { operation: "封面视频", result: Err(error.to_string()) },
        });
    }

    fn save_template(&mut self, cx: &mut Context<Self>) {
        self.pull(cx);
        let Some(project) = self.project().cloned() else {
            self.show_error("请先建立项目.", cx);
            return;
        };
        let name = if project.name.trim().is_empty() { "未命名模板".into() } else { project.name.clone() };
        self.workspace.templates.push(model::Template {
            id: model::new_id(),
            name,
            rule: project.naming_rule.clone(),
            output: project.output.clone(),
            settings: project.settings.clone(),
        });
        self.status = "已保存模板.".into();
        self.mark_dirty(cx);
    }

    fn apply_template(&mut self, index: usize, window: &mut Window, cx: &mut Context<Self>) {
        self.pull(cx);
        let Some(template) = self.workspace.templates.get(index).cloned() else {
            return;
        };
        let Some(project) = self.project_mut() else {
            self.show_error("请先建立项目.", cx);
            return;
        };
        project.naming_rule = template.rule;
        project.output = template.output.clone();
        project.auto_output = template.output.trim().is_empty();
        project.settings = template.settings;
        self.push(window, cx);
        self.mark_dirty(cx);
    }

    fn save_llm(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let previous = self.llm.clone();
        let next = read_llm(self, cx);
        self.llm = llm::config_after_endpoint_change(&previous, next);
        if self.llm.api_key.is_empty() {
            set_text(&self.llm_key, "", window, cx);
        }
        match crate::store::save_llm(&self.llm) {
            Ok(()) => self.status = "已保存模型配置.".into(),
            Err(error) => self.show_error(error.to_string(), cx),
        }
        cx.notify();
    }

    fn list_models(&mut self, cx: &mut Context<Self>) {
        if self.busy {
            return;
        }
        let config = llm::config_after_endpoint_change(&self.llm, read_llm(self, cx));
        self.spawn_job(cx, move || match llm::list_models(&config) {
            Ok(names) => JobMsg::Done { operation: "读取模型", result: Ok(names.join(", ")) },
            Err(error) => JobMsg::Done { operation: "读取模型", result: Err(error.to_string()) },
        });
    }

    fn request_english(&mut self, cx: &mut Context<Self>) {
        self.pull(cx);
        if self.busy {
            return;
        }
        let config = llm::config_after_endpoint_change(&self.llm, read_llm(self, cx));
        let Some(project) = self.project().cloned() else {
            self.show_error("请先建立项目.", cx);
            return;
        };
        self.page = super::Page::Llm;
        self.spawn_job(cx, move || match llm::suggest_english(&config, &project) {
            Ok(items) => JobMsg::English(Ok(items)),
            Err(error) => JobMsg::English(Err(error.to_string())),
        });
    }

    fn apply_english(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let chosen = self.review.iter().filter(|item| item.apply && !item.after.trim().is_empty()).map(|item| (item.key.clone(), item.after.clone())).collect::<Vec<_>>();
        let Some(project) = self.project_mut() else {
            return;
        };
        for (key, value) in &chosen {
            llm::apply_field(project, key, value);
        }
        self.push(window, cx);
        self.status = format!("已应用 {} 项.", chosen.len()).into();
        self.mark_dirty(cx);
    }
}

fn session_form(studio: &Studio) -> impl IntoElement {
    Form::new()
        .child(super::field("场次 / 专辑名", &studio.name))
        .child(super::field("演奏家", &studio.performer))
        .child(super::field("指挥", &studio.conductor))
        .child(super::field("乐团", &studio.orchestra))
        .child(super::field("作曲家", &studio.composer))
        .child(super::field("演出日期", &studio.date))
        .child(super::field("专辑日期", &studio.album_date))
        .child(super::field("城市", &studio.city))
        .child(super::field("音乐厅", &studio.venue))
        .child(super::field("录音来源", &studio.origin))
}

fn output_form(studio: &Studio) -> impl IntoElement {
    Form::new().child(super::field("输出文件夹", &studio.output)).child(super::field("命名规则", &studio.naming)).child(super::field("备注", &studio.notes))
}

fn tag_form(studio: &Studio) -> impl IntoElement {
    Form::new()
        .child(super::field(TAG_KEYS[0], &studio.tags[0]))
        .child(super::field(TAG_KEYS[1], &studio.tags[1]))
        .child(super::field(TAG_KEYS[2], &studio.tags[2]))
        .child(super::field(TAG_KEYS[3], &studio.tags[3]))
        .child(super::field(TAG_KEYS[4], &studio.tags[4]))
        .child(super::field(TAG_KEYS[5], &studio.tags[5]))
        .child(super::field(TAG_KEYS[6], &studio.tags[6]))
        .child(super::field(TAG_KEYS[7], &studio.tags[7]))
        .child(super::field(TAG_KEYS[8], &studio.tags[8]))
        .child(super::field(TAG_KEYS[9], &studio.tags[9]))
        .child(super::field(TAG_KEYS[10], &studio.tags[10]))
}

fn check(id: &'static str, label: &'static str, on: bool, cx: &mut Context<Studio>, set: fn(&mut Project, bool)) -> impl IntoElement {
    Checkbox::new(id).label(label).checked(on).on_change(cx.listener(move |this, value, _, cx| {
        this.pull(cx);
        if let Some(project) = this.project_mut() {
            set(project, *value);
        }
        this.mark_dirty(cx);
    }))
}

fn mode_row(group: &'static str, current: &str, options: &[(&'static str, &'static str)], cx: &mut Context<Studio>, set: fn(&mut Project, &str)) -> impl IntoElement {
    let mut row = div().flex().flex_row().gap_1();
    for (value, label) in options {
        let value = *value;
        row = row.child(
            Button::new(SharedString::from(format!("{group}-{value}")))
                .xsmall()
                .selected(value == current)
                .label(*label)
                .on_click(cx.listener(move |this, _, _, cx| {
                    this.pull(cx);
                    if let Some(project) = this.project_mut() {
                        set(project, value);
                    }
                    this.mark_dirty(cx);
                })),
        );
    }
    row
}

fn mode_button(id: &'static str, label: &'static str, on: bool, cx: &mut Context<Studio>, mode: VideoMode) -> impl IntoElement {
    Button::new(id).small().selected(on).label(label).on_click(cx.listener(move |this, _, _, cx| {
        this.video_mode = mode;
        cx.notify();
    }))
}

fn set_denoise(project: &mut Project, value: &str) {
    project.settings.effects.denoise = value.into();
}
fn set_hum(project: &mut Project, value: &str) {
    project.settings.effects.hum = value.into();
}
fn set_channels(project: &mut Project, value: &str) {
    project.settings.effects.channels = value.into();
}
fn set_normalize(project: &mut Project, value: &str) {
    project.settings.effects.normalize = value.into();
}

fn cover_label(project: Option<&Project>) -> String {
    let Some(project) = project else {
        return "未选择封面".into();
    };
    match (project.cover_file.is_empty(), project.back_cover_file.is_empty()) {
        (true, true) => "未选择封面".into(),
        (false, true) => format!("封面 {}", project.cover_file),
        (true, false) => format!("封底 {}", project.back_cover_file),
        (false, false) => format!("封面 {} / 封底 {}", project.cover_file, project.back_cover_file),
    }
}

fn read_llm(studio: &Studio, cx: &gpui_kit::App) -> LlmConfig {
    LlmConfig {
        base_url: text(&studio.llm_base, cx),
        model: text(&studio.llm_model, cx),
        api_key: text(&studio.llm_key, cx),
    }
}

fn group_block(project: &Project, cx: &mut Context<Studio>) -> impl IntoElement {
    let groups = names::suggest_movement_groups(&project.tracks);
    let mut row = div().flex().flex_row().gap_1();
    for (index, group) in groups.iter().enumerate() {
        let indexes = group.indexes.clone();
        let label = format!("建议合并 {} ({} 段)", group.title, indexes.len());
        row = row.child(
            Button::new(SharedString::from(format!("group-{index}")))
                .xsmall()
                .label(label)
                .on_click(cx.listener(move |this, _, window, cx| {
                    this.track_picks = indexes.iter().copied().collect();
                    let _ = this.merge_selected(window, cx);
                })),
        );
    }
    row
}

fn preflight_lines(text: &str) -> impl IntoElement {
    let mut column = div().flex().flex_col().gap_1();
    for line in text.lines() {
        column = column.child(Label::new(line.to_string()));
    }
    column
}

fn outcome(reports: &[TrackReport], message: String) -> Result<String, String> {
    if reports.iter().any(|item| item.error.is_some()) && reports.iter().all(|item| item.error.is_some()) {
        Err(message)
    } else {
        Ok(message)
    }
}
