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
use gpui_kit::{AnyElement, Context, IntoElement, ParentElement as _, SharedString, Styled as _, Window, div};

use crate::catalog;
use crate::cue::{self, CueTrack};
use crate::effects;
use crate::export::{self, TrackReport};
use crate::ffmpeg;
use crate::llm::{self, LlmConfig};
use crate::mb;
use crate::model::{self, Project, Settings, Track, TAG_KEYS};
use crate::names::{self, session_tag_values};
use crate::video::{self, VideoMode};

use super::{set_text, text, JobMsg, Studio};

const AUDIO_EXT: &[&str] = &["wav", "flac", "mp3", "mkv", "ape", "m4a", "aac", "aiff", "aif", "ogg", "opus", "wv"];
const CD_EXT: &[&str] = &["wav", "flac", "mp3", "mkv", "ape", "m4a", "aac", "aiff", "aif", "ogg", "opus", "wv", "cue"];

impl Studio {
    pub(super) fn split_page(&mut self, cx: &mut Context<Self>) -> AnyElement {
        let Some(project) = self.project().cloned() else {
            return div().child(Label::new("请先建立或打开一个项目.")).into_any_element();
        };
        let mut page = div().flex().flex_col().gap_3();
        page = page.child(session_form(self)).child(output_form(self)).child(self.settings_block(&project.settings, cx)).child(self.effects_block(&project, cx));
        page = page.child(self.source_block(&project, cx)).child(self.track_block(&project, cx)).child(self.actions(cx));
        if !self.mb_hits.is_empty() {
            page = page.child(self.mb_block(cx));
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
                    .on_change(cx.listener(|this, index, _, cx| {
                        if let Some(project) = this.project_mut() {
                            project.settings.format = ["FLAC", "WAV", "MP3", "MKV"][*index].into();
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
            .child(check("auto-join", "空的终点接到下一轨起点", self.project().is_some_and(|project| project.auto_join), cx, |project, value| project.auto_join = value))
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
                    .child(Button::new("concat-sources").small().label("合并为 FLAC").on_click(cx.listener(|this, _, window, cx| this.concat_sources(window, cx)))),
            )
            .child(list)
    }

    fn track_block(&mut self, project: &Project, cx: &mut Context<Self>) -> impl IntoElement {
        let selected = self.track_index;
        let mut list = div().flex().flex_col().gap_1();
        for (index, track) in project.tracks.iter().enumerate() {
            let title = if track.tags.title.trim().is_empty() { "未命名曲目".to_string() } else { track.tags.title.clone() };
            let label = format!("{:02}  {}  {} - {}", index + 1, track.source, track.start, title);
            let id = track.id.clone();
            list = list.child(
                Button::new(SharedString::from(format!("track-{id}")))
                    .small()
                    .ghost()
                    .selected(selected == Some(index))
                    .label(label)
                    .on_click(cx.listener(move |this, _, window, cx| this.select_track(index, window, cx))),
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
                    .child(Button::new("cycle-status").small().label(format!("状态: {}", project.status)).on_click(cx.listener(|this, _, _, cx| this.cycle_status(cx)))),
            )
            .child(list)
            .child(editor)
    }

    fn source_picker(&self, project: &Project, cx: &mut Context<Self>) -> impl IntoElement {
        let current = self.track_index.and_then(|index| project.tracks.get(index)).map(|track| track.source.clone()).unwrap_or_default();
        let mut row = div().flex().flex_row().gap_1();
        for source in &project.sources {
            let name = source.clone();
            let selected = name == current;
            row = row.child(
                Button::new(SharedString::from(format!("use-{name}")))
                    .xsmall()
                    .selected(selected)
                    .label(name.clone())
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
        let mut rows = div().flex().flex_col().gap_1();
        for (index, row) in self.cd_rows.iter().enumerate() {
            let label = format!("{:02}  {}  {}", row.number, row.source, row.title);
            rows = rows.child(
                Button::new(SharedString::from(format!("cd-row-{index}")))
                    .small()
                    .ghost()
                    .selected(self.cd_index == Some(index))
                    .label(label)
                    .on_click(cx.listener(move |this, _, window, cx| this.select_cd(index, window, cx))),
            );
        }
        div()
            .flex()
            .flex_col()
            .gap_3()
            .child(Label::new(self.cd_notice.clone()))
            .child(
                div()
                    .flex()
                    .flex_row()
                    .gap_2()
                    .child(Button::new("cd-files").label("选择音频和 CUE").on_click(cx.listener(|this, _, window, cx| this.pick_cd(window, cx))))
                    .child(Button::new("cd-paste").label("从剪贴板读取曲目").on_click(cx.listener(|this, _, window, cx| this.paste_tracks(window, cx))))
                    .child(Button::new("cd-import").primary().label("导入到当前项目").on_click(cx.listener(|this, _, window, cx| this.import_cd(false, window, cx))))
                    .child(Button::new("cd-replace").danger().label("替换全部曲目").on_click(cx.listener(|this, _, window, cx| this.import_cd(true, window, cx)))),
            )
            .child(Form::new().child(super::field("专辑", &self.cd_title)).child(super::field("演出者", &self.cd_artist)).child(super::field("作曲家", &self.cd_composer)).child(super::field("日期", &self.cd_date)))
            .child(rows)
            .child(Form::new().child(super::field("曲名", &self.cd_row_title)).child(super::field("演出者", &self.cd_row_artist)).child(super::field("作曲家", &self.cd_row_composer)).child(super::field("文件名", &self.cd_row_file)))
            .into_any_element()
    }

    pub(super) fn video_page(&mut self, cx: &mut Context<Self>) -> AnyElement {
        let mut files = div().flex().flex_col().gap_1();
        for path in &self.video_files {
            files = files.child(Label::new(path.display().to_string()));
        }
        div()
            .flex()
            .flex_col()
            .gap_3()
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
        div()
            .flex()
            .flex_col()
            .gap_2()
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
        div()
            .flex()
            .flex_col()
            .gap_3()
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
        div()
            .flex()
            .flex_col()
            .gap_3()
            .child(Label::new("API Key 只保存在本机, 不写入项目文件."))
            .child(Form::new().child(super::field("接口", &self.llm_base)).child(super::field("模型", &self.llm_model)).child(Field::new().label("API Key").child(Input::new(&self.llm_key).small().content_type(gpui_kit::component::input::InputContentType::Password).mask_toggle())))
            .child(
                div()
                    .flex()
                    .flex_row()
                    .gap_2()
                    .child(Button::new("save-llm").label("保存配置").on_click(cx.listener(|this, _, _, cx| this.save_llm(cx))))
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
        let Some(project) = self.project_mut() else {
            return;
        };
        let source = self.track_index.and_then(|index| project.tracks.get(index)).map(|track| track.source.clone()).unwrap_or_default();
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
        let session = session_tag_values(project);
        for (index, track) in project.tracks.iter_mut().enumerate() {
            for key in TAG_KEYS {
                if !track.tags.get(key).trim().is_empty() || key == "Title" {
                    continue;
                }
                let value = if key == "Track" { format!("{:02}", index + 1) } else { session.get(key).to_string() };
                if !value.is_empty() {
                    track.tags.set(key, value);
                }
            }
        }
        self.push_track(window, cx);
        self.mark_dirty(cx);
    }

    fn apply_names(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.pull(cx);
        let Some(project) = self.project_mut() else {
            return;
        };
        let rule = if project.naming_rule.trim().is_empty() { names::DEFAULT_NAMING.to_string() } else { project.naming_rule.clone() };
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
        let mut cue_path = None;
        self.cd_files.clear();
        for file in files {
            let name = file.file_name().map(|item| item.to_string_lossy().to_string()).unwrap_or_default();
            if name.to_ascii_lowercase().ends_with(".cue") {
                cue_path = Some(file);
            } else if names::audio_name(&name) {
                self.cd_files.push((name, file.display().to_string()));
            }
        }
        if let Some(path) = cue_path {
            let bytes = match std::fs::read(&path) {
                Ok(bytes) => bytes,
                Err(error) => {
                    self.show_error(error.to_string(), cx);
                    return;
                }
            };
            match cue::parse_cue(&cue::decode_cue(&bytes), &self.cd_files) {
                Ok((album, tracks)) => self.show_cd(album, tracks, window, cx),
                Err(error) => self.show_error(error.to_string(), cx),
            }
        } else {
            self.cd_notice = format!("已选择 {} 个音频. 可以再粘贴曲目文字.", self.cd_files.len()).into();
            cx.notify();
        }
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
        self.cd_rows = rows;
        self.cd_index = if self.cd_rows.is_empty() { None } else { Some(0) };
        self.push_cd(window, cx);
        cx.notify();
    }

    fn show_cd(&mut self, album: cue::CueAlbum, tracks: Vec<CueTrack>, window: &mut Window, cx: &mut Context<Self>) {
        self.show_album(&album, window, cx);
        self.cd_rows = tracks;
        self.cd_index = if self.cd_rows.is_empty() { None } else { Some(0) };
        self.cd_notice = format!("已读取 {} 轨.", self.cd_rows.len()).into();
        self.push_cd(window, cx);
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

    fn import_cd(&mut self, replace: bool, window: &mut Window, cx: &mut Context<Self>) {
        self.pull(cx);
        self.pull_cd(cx);
        if self.project().is_none() {
            self.workspace.projects.push(Project::blank());
            self.project_index = Some(self.workspace.projects.len() - 1);
        }
        let album_title = text(&self.cd_title, cx);
        let album_artist = text(&self.cd_artist, cx);
        let album_composer = text(&self.cd_composer, cx);
        let album_date = text(&self.cd_date, cx);
        let rows = self.cd_rows.clone();
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
        if replace {
            project.tracks.clear();
        }
        for row in rows {
            if !row.source.is_empty() && !project.sources.iter().any(|source| source == &row.source) {
                project.sources.push(row.source.clone());
            }
            if !row.path.is_empty() {
                project.source_paths.insert(row.source.clone(), row.path.clone());
            }
            let mut track = Track::blank();
            track.source = row.source;
            track.start = if row.start.is_empty() { "00:00".into() } else { row.start };
            track.end = row.end;
            track.filename = row.filename;
            track.tags.title = row.title;
            track.tags.artist = if row.artist.is_empty() { album_artist.clone() } else { row.artist };
            track.tags.composer = if row.composer.is_empty() { album_composer.clone() } else { row.composer };
            track.tags.album = project.name.clone();
            track.tags.date = album_date.clone();
            project.tracks.push(track);
        }
        if project.tracks.is_empty() {
            project.tracks.push(Track::blank());
        }
        self.track_index = Some(0);
        self.page = super::Page::Split;
        self.push(window, cx);
        self.status = "已导入 CD 曲目.".into();
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

    fn save_llm(&mut self, cx: &mut Context<Self>) {
        self.llm = read_llm(self, cx);
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
        let config = read_llm(self, cx);
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
        let config = read_llm(self, cx);
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

fn outcome(reports: &[TrackReport], message: String) -> Result<String, String> {
    if reports.iter().any(|item| item.error.is_some()) && reports.iter().all(|item| item.error.is_some()) {
        Err(message)
    } else {
        Ok(message)
    }
}
