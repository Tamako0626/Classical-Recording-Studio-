//! GPUI Kit 界面. 文本框, 按钮, 表单和对话框都用组件库, 不自己画输入法.

mod work;

use std::time::Duration;

use gpui_kit::component::{
    ActiveTheme as _, Disableable as _, Sizable as _,
    button::{Button, ButtonVariants as _},
    form::Field,
    input::{Input, InputEvent, InputState},
    Theme,
};
use gpui_kit::{
    App, AppContext as _, Context, Div, Entity, InteractiveElement as _, IntoElement, ParentElement, Render,
    SharedString, StatefulInteractiveElement as _, Styled, Subscription, Task, Window, WindowBounds,
    WindowOptions, application, assets, div, init, open_window, px, rgb, size,
};

use crate::export::TrackReport;
use crate::ffmpeg::{self, Binaries, Cancel};
use crate::llm::{EnglishSuggestion, LlmConfig};
use crate::mb::WorkHit;
use crate::model::{Project, Settings, Track, Workspace, TAG_KEYS};
use crate::playback::Player;
use crate::store;
use crate::video::VideoMode;
use crate::cue;

#[derive(Clone, Copy, PartialEq, Eq)]
enum Page {
    Projects,
    Split,
    Cd,
    Video,
    History,
    Templates,
    Llm,
}

enum JobMsg {
    Status(String),
    Done { operation: &'static str, result: Result<String, String> },
    English(Result<Vec<EnglishSuggestion>, String>),
    Works(Result<Vec<WorkHit>, String>),
    Play(Result<std::path::PathBuf, String>),
}

pub struct Studio {
    workspace: Workspace,
    llm: LlmConfig,
    page: Page,
    project_index: Option<usize>,
    track_index: Option<usize>,
    ffmpeg: Option<Binaries>,
    cancel: Cancel,
    player: Player,
    busy: bool,
    ready: bool,
    save_scheduled: bool,
    limiter_on: bool,
    status: SharedString,
    error: Option<SharedString>,
    name: Entity<InputState>,
    performer: Entity<InputState>,
    conductor: Entity<InputState>,
    orchestra: Entity<InputState>,
    composer: Entity<InputState>,
    date: Entity<InputState>,
    album_date: Entity<InputState>,
    city: Entity<InputState>,
    venue: Entity<InputState>,
    origin: Entity<InputState>,
    output: Entity<InputState>,
    naming: Entity<InputState>,
    notes: Entity<InputState>,
    start: Entity<InputState>,
    end: Entity<InputState>,
    filename: Entity<InputState>,
    gain: Entity<InputState>,
    width: Entity<InputState>,
    reverb: Entity<InputState>,
    tags: [Entity<InputState>; 9],
    noise_reduction: Entity<InputState>,
    highpass: Entity<InputState>,
    lowpass: Entity<InputState>,
    bass: Entity<InputState>,
    mid: Entity<InputState>,
    treble: Entity<InputState>,
    gate: Entity<InputState>,
    compress: Entity<InputState>,
    threshold: Entity<InputState>,
    lufs: Entity<InputState>,
    fade_in: Entity<InputState>,
    fade_out: Entity<InputState>,
    limiter: Entity<InputState>,
    effects_open: bool,
    cd_cue: bool,
    cd_files: Vec<(String, String)>,
    cd_rows: Vec<cue::CueTrack>,
    cd_notice: SharedString,
    cd_title: Entity<InputState>,
    cd_artist: Entity<InputState>,
    cd_composer: Entity<InputState>,
    cd_date: Entity<InputState>,
    cd_index: Option<usize>,
    cd_row_title: Entity<InputState>,
    cd_row_artist: Entity<InputState>,
    cd_row_composer: Entity<InputState>,
    cd_row_file: Entity<InputState>,
    video_files: Vec<std::path::PathBuf>,
    video_cover: SharedString,
    video_mode: VideoMode,
    llm_base: Entity<InputState>,
    llm_model: Entity<InputState>,
    llm_key: Entity<InputState>,
    review: Vec<EnglishSuggestion>,
    mb_query: Entity<InputState>,
    mb_hits: Vec<WorkHit>,
    preview_raw: Option<std::path::PathBuf>,
    preview_wet: Option<std::path::PathBuf>,
    save_task: Option<Task<()>>,
    job_task: Option<Task<()>>,
    progress_task: Option<Task<()>>,
    _subscriptions: Vec<Subscription>,
}

impl Studio {
    fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        let mut input = |placeholder: &str, cx: &mut Context<Self>| cx.new(|cx| InputState::new(window, cx).placeholder(placeholder));
        let name = input("场次 / 专辑名", cx);
        let performer = input("演奏家", cx);
        let conductor = input("指挥", cx);
        let orchestra = input("乐团", cx);
        let composer = input("作曲家", cx);
        let date = input("演出日期", cx);
        let album_date = input("专辑标签日期", cx);
        let city = input("城市", cx);
        let venue = input("音乐厅", cx);
        let origin = input("录音来源", cx);
        let output = input("留空则按场次命名", cx);
        let naming = input("{Track} - {Title}", cx);
        let notes = input("备注", cx);
        let start = input("00:00", cx);
        let end = input("留空到文件结束", cx);
        let filename = input("留空则用命名规则", cx);
        let gain = input("0", cx);
        let width = input("100", cx);
        let reverb = input("0", cx);
        let tags = [
            input("Title", cx),
            input("Artist", cx),
            input("Album", cx),
            input("01", cx),
            input("Date", cx),
            input("Composer", cx),
            input("Conductor", cx),
            input("Soloist", cx),
            input("Orchestra", cx),
        ];
        let noise_reduction = input("8", cx);
        let highpass = input("0", cx);
        let lowpass = input("0", cx);
        let bass = input("0", cx);
        let mid = input("0", cx);
        let treble = input("0", cx);
        let gate = input("0", cx);
        let compress = input("1", cx);
        let threshold = input("-18", cx);
        let lufs = input("-18", cx);
        let fade_in = input("0", cx);
        let fade_out = input("0", cx);
        let limiter = input("-1", cx);
        let cd_title = input("专辑名称", cx);
        let cd_artist = input("专辑演出者", cx);
        let cd_composer = input("作曲家", cx);
        let cd_date = input("日期或年份", cx);
        let cd_row_title = input("曲名", cx);
        let cd_row_artist = input("演出者", cx);
        let cd_row_composer = input("作曲家", cx);
        let cd_row_file = input("文件名, 不含扩展名", cx);
        let mb_query = input("曲名, 作品号", cx);
        drop(input);
        let llm = store::load_llm();
        let llm_base = cx.new(|cx| InputState::new(window, cx).placeholder("https://api.deepseek.com").default_value(llm.base_url.clone()));
        let llm_model = cx.new(|cx| InputState::new(window, cx).placeholder("deepseek-chat").default_value(llm.model.clone()));
        let llm_key = cx.new(|cx| InputState::new(window, cx).placeholder("API Key").default_value(llm.api_key.clone()).masked(true));
        let watched = [
            name.clone(),
            performer.clone(),
            conductor.clone(),
            orchestra.clone(),
            composer.clone(),
            date.clone(),
            album_date.clone(),
            city.clone(),
            venue.clone(),
            origin.clone(),
            output.clone(),
            naming.clone(),
            notes.clone(),
            start.clone(),
            end.clone(),
            filename.clone(),
            gain.clone(),
            width.clone(),
            reverb.clone(),
            noise_reduction.clone(),
            highpass.clone(),
            lowpass.clone(),
            bass.clone(),
            mid.clone(),
            treble.clone(),
            gate.clone(),
            compress.clone(),
            threshold.clone(),
            lufs.clone(),
            fade_in.clone(),
            fade_out.clone(),
            limiter.clone(),
        ]
        .into_iter()
        .chain(tags.clone())
        .collect::<Vec<_>>();
        let mut subscriptions = Vec::new();
        for field in watched {
            subscriptions.push(cx.subscribe_in(&field, window, |this, _, event, _, cx| {
                if this.ready && matches!(event, InputEvent::Change) {
                    this.pull(cx);
                    this.mark_dirty(cx);
                }
            }));
        }
        let workspace = store::load_workspace();
        let project_index = if workspace.projects.is_empty() { None } else { Some(0) };
        let track_index = project_index.and_then(|index| workspace.projects.get(index).and_then(|project| if project.tracks.is_empty() { None } else { Some(0) }));
        let ffmpeg = ffmpeg::locate().ok();
        let status = if ffmpeg.is_some() {
            "已找到裁剪版 FFmpeg."
        } else {
            "未找到裁剪版 FFmpeg. 请把 ffmpeg 与 ffprobe 放在程序同目录."
        };
        let mut studio = Self {
            workspace,
            llm,
            page: Page::Projects,
            project_index,
            track_index,
            ffmpeg,
            cancel: Cancel::default(),
            player: Player::new(),
            busy: false,
            ready: false,
            save_scheduled: false,
            limiter_on: false,
            status: status.into(),
            error: None,
            name,
            performer,
            conductor,
            orchestra,
            composer,
            date,
            album_date,
            city,
            venue,
            origin,
            output,
            naming,
            notes,
            start,
            end,
            filename,
            gain,
            width,
            reverb,
            tags,
            noise_reduction,
            highpass,
            lowpass,
            bass,
            mid,
            treble,
            gate,
            compress,
            threshold,
            lufs,
            fade_in,
            fade_out,
            limiter,
            effects_open: false,
            cd_cue: true,
            cd_files: Vec::new(),
            cd_rows: Vec::new(),
            cd_notice: "选择音频, 或粘贴带轨号的文字.".into(),
            cd_title,
            cd_artist,
            cd_composer,
            cd_date,
            cd_index: None,
            cd_row_title,
            cd_row_artist,
            cd_row_composer,
            cd_row_file,
            video_files: Vec::new(),
            video_cover: SharedString::default(),
            video_mode: VideoMode::Auto,
            llm_base,
            llm_model,
            llm_key,
            review: Vec::new(),
            mb_query,
            mb_hits: Vec::new(),
            preview_raw: None,
            preview_wet: None,
            save_task: None,
            job_task: None,
            progress_task: None,
            _subscriptions: subscriptions,
        };
        studio.push(window, cx);
        studio.ready = true;
        studio
    }

    fn project(&self) -> Option<&Project> {
        self.project_index.and_then(|index| self.workspace.projects.get(index))
    }

    fn project_mut(&mut self) -> Option<&mut Project> {
        let index = self.project_index?;
        self.workspace.projects.get_mut(index)
    }

    fn track_mut(&mut self) -> Option<&mut Track> {
        let index = self.track_index?;
        self.project_mut()?.tracks.get_mut(index)
    }

    fn mark_dirty(&mut self, cx: &mut Context<Self>) {
        self.save_scheduled = true;
        cx.notify();
        self.save_task = Some(cx.spawn(async move |this, cx| {
            cx.background_executor().timer(Duration::from_millis(400)).await;
            this.update(cx, |this, _| {
                this.save_scheduled = false;
                let _ = store::save_workspace(&this.workspace);
            })
            .ok();
        }));
    }

    fn record(&mut self, operation: &str, result: &str, message: impl Into<String>, cx: &mut Context<Self>) {
        let Some(project) = self.project().cloned() else {
            return;
        };
        self.workspace.logs.push(crate::model::LogEntry::new(&project, operation, result, message));
        if self.workspace.logs.len() > 400 {
            let extra = self.workspace.logs.len() - 400;
            self.workspace.logs.drain(0..extra);
        }
        self.mark_dirty(cx);
    }

    fn show_error(&mut self, message: impl Into<String>, cx: &mut Context<Self>) {
        self.error = Some(message.into().into());
        cx.notify();
    }

    fn pull(&mut self, cx: &mut Context<Self>) {
        if !self.ready {
            return;
        }
        let name = text(&self.name, cx);
        let performer = text(&self.performer, cx);
        let conductor = text(&self.conductor, cx);
        let orchestra = text(&self.orchestra, cx);
        let composer = text(&self.composer, cx);
        let date = text(&self.date, cx);
        let album_date = text(&self.album_date, cx);
        let city = text(&self.city, cx);
        let venue = text(&self.venue, cx);
        let origin = text(&self.origin, cx);
        let output = text(&self.output, cx);
        let naming = text(&self.naming, cx);
        let notes = text(&self.notes, cx);
        let gain = number(&self.gain, cx, 0.0);
        let width = number(&self.width, cx, 100.0);
        let reverb = number(&self.reverb, cx, 0.0);
        let noise_reduction = number(&self.noise_reduction, cx, 8.0);
        let highpass = number(&self.highpass, cx, 0.0);
        let lowpass = number(&self.lowpass, cx, 0.0);
        let bass = number(&self.bass, cx, 0.0);
        let mid = number(&self.mid, cx, 0.0);
        let treble = number(&self.treble, cx, 0.0);
        let gate = number(&self.gate, cx, 0.0);
        let compress = number(&self.compress, cx, 1.0);
        let threshold = number(&self.threshold, cx, -18.0);
        let lufs = number(&self.lufs, cx, -18.0);
        let fade_in = number(&self.fade_in, cx, 0.0);
        let fade_out = number(&self.fade_out, cx, 0.0);
        let limiter = if self.limiter_on { number(&self.limiter, cx, -1.0) } else { 0.0 };
        let track_index = self.track_index;
        let start = text(&self.start, cx);
        let end = text(&self.end, cx);
        let filename = text(&self.filename, cx);
        let tags = self.tags.iter().map(|input| text(input, cx)).collect::<Vec<_>>();
        let Some(project) = self.project_mut() else {
            return;
        };
        project.name = name;
        project.performer = performer;
        project.conductor = conductor;
        project.orchestra = orchestra;
        project.composer = composer;
        project.date = date;
        project.album_date = album_date;
        project.city = city;
        project.venue = venue;
        project.origin = origin;
        project.output = output;
        project.auto_output = project.output.trim().is_empty();
        project.naming_rule = naming;
        project.notes = notes;
        project.settings.gain = gain;
        project.settings.width = width;
        project.settings.reverb = reverb;
        let effects = &mut project.settings.effects;
        effects.noise_reduction = noise_reduction;
        effects.highpass = highpass;
        effects.lowpass = lowpass;
        effects.bass = bass;
        effects.mid = mid;
        effects.treble = treble;
        effects.gate = gate;
        effects.compress = compress;
        effects.threshold = threshold;
        effects.lufs = lufs;
        effects.fade_in = fade_in;
        effects.fade_out = fade_out;
        effects.limiter = limiter;
        if let Some(track) = track_index.and_then(|index| project.tracks.get_mut(index)) {
            track.start = start;
            track.end = end;
            track.filename = filename;
            for (key, value) in TAG_KEYS.iter().zip(tags) {
                track.tags.set(key, value);
            }
        }
    }

    fn push(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(project) = self.project().cloned() else {
            return;
        };
        set_text(&self.name, &project.name, window, cx);
        set_text(&self.performer, &project.performer, window, cx);
        set_text(&self.conductor, &project.conductor, window, cx);
        set_text(&self.orchestra, &project.orchestra, window, cx);
        set_text(&self.composer, &project.composer, window, cx);
        set_text(&self.date, &project.date, window, cx);
        set_text(&self.album_date, &project.album_date, window, cx);
        set_text(&self.city, &project.city, window, cx);
        set_text(&self.venue, &project.venue, window, cx);
        set_text(&self.origin, &project.origin, window, cx);
        set_text(&self.output, if project.auto_output { "" } else { &project.output }, window, cx);
        set_text(&self.naming, if project.naming_rule.is_empty() { crate::names::DEFAULT_NAMING } else { &project.naming_rule }, window, cx);
        set_text(&self.notes, &project.notes, window, cx);
        set_text(&self.gain, &fmt_num(project.settings.gain), window, cx);
        set_text(&self.width, &fmt_num(project.settings.width), window, cx);
        set_text(&self.reverb, &fmt_num(project.settings.reverb), window, cx);
        self.push_effects(&project.settings, window, cx);
        self.push_track(window, cx);
    }

    fn push_effects(&mut self, settings: &Settings, window: &mut Window, cx: &mut Context<Self>) {
        let effects = &settings.effects;
        set_text(&self.noise_reduction, &fmt_num(effects.noise_reduction), window, cx);
        set_text(&self.highpass, &fmt_num(effects.highpass), window, cx);
        set_text(&self.lowpass, &fmt_num(effects.lowpass), window, cx);
        set_text(&self.bass, &fmt_num(effects.bass), window, cx);
        set_text(&self.mid, &fmt_num(effects.mid), window, cx);
        set_text(&self.treble, &fmt_num(effects.treble), window, cx);
        set_text(&self.gate, &fmt_num(effects.gate), window, cx);
        set_text(&self.compress, &fmt_num(effects.compress), window, cx);
        set_text(&self.threshold, &fmt_num(effects.threshold), window, cx);
        set_text(&self.lufs, &fmt_num(effects.lufs), window, cx);
        set_text(&self.fade_in, &fmt_num(effects.fade_in), window, cx);
        set_text(&self.fade_out, &fmt_num(effects.fade_out), window, cx);
        self.limiter_on = effects.limiter != 0.0;
        if self.limiter_on {
            set_text(&self.limiter, &fmt_num(effects.limiter), window, cx);
        }
    }

    fn push_track(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let track = self.track_index.and_then(|index| self.project().and_then(|project| project.tracks.get(index)).cloned());
        let track = track.unwrap_or_else(Track::blank);
        set_text(&self.start, &track.start, window, cx);
        set_text(&self.end, &track.end, window, cx);
        set_text(&self.filename, &track.filename, window, cx);
        for (key, input) in TAG_KEYS.iter().zip(self.tags.iter()) {
            set_text(input, track.tags.get(key), window, cx);
        }
    }

    fn select_project(&mut self, index: usize, window: &mut Window, cx: &mut Context<Self>) {
        self.pull(cx);
        self.project_index = Some(index);
        self.track_index = self.project().and_then(|project| if project.tracks.is_empty() { None } else { Some(0) });
        self.page = Page::Split;
        self.push(window, cx);
        cx.notify();
    }

    fn spawn_job(&mut self, cx: &mut Context<Self>, work: impl FnOnce() -> JobMsg + Send + 'static) {
        if self.busy {
            return;
        }
        self.busy = true;
        self.error = None;
        cx.notify();
        self.job_task = Some(cx.spawn(async move |this, cx| {
            let msg = cx.background_spawn(async move { work() }).await;
            this.update(cx, |this, cx| this.finish(msg, cx)).ok();
        }));
    }

    fn finish(&mut self, msg: JobMsg, cx: &mut Context<Self>) {
        self.busy = false;
        match msg {
            JobMsg::Status(text) => self.status = text.into(),
            JobMsg::Done { operation, result } => match result {
                Ok(text) => {
                    self.status = text.clone().into();
                    self.record(operation, "成功", text, cx);
                }
                Err(text) => {
                    self.record(operation, "失败", text.clone(), cx);
                    self.show_error(text, cx);
                }
            },
            JobMsg::English(Ok(items)) => {
                self.review = items;
                self.status = "请核对英文建议, 再应用选中项.".into();
            }
            JobMsg::English(Err(text)) => self.show_error(text, cx),
            JobMsg::Works(Ok(items)) => {
                self.mb_hits = items;
                self.status = format!("MusicBrainz 返回 {} 条.", self.mb_hits.len()).into();
            }
            JobMsg::Works(Err(text)) => self.show_error(text, cx),
            JobMsg::Play(Ok(path)) => {
                if let Err(error) = self.player.play(path) {
                    self.show_error(error.to_string(), cx);
                } else {
                    self.status = "正在试听.".into();
                }
            }
            JobMsg::Play(Err(text)) => self.show_error(text, cx),
        }
        cx.notify();
    }

    fn nav_button(&self, id: &'static str, mark: &'static str, label: &'static str, page: Page, cx: &mut Context<Self>) -> gpui_kit::AnyElement {
        let selected = self.page == page;
        let mut item = div()
            .id(id)
            .flex()
            .flex_row()
            .items_center()
            .gap(px(12.))
            .px(px(12.))
            .py(px(10.))
            .rounded(px(9.))
            .text_color(if selected { rgb(0xffffff) } else { rgb(0xe1d9ee) })
            .on_click(cx.listener(move |this, _, _, cx| {
                this.pull(cx);
                this.page = page;
                cx.notify();
            }));
        if selected {
            item = item.bg(rgb(0x42335f));
        }
        item.child(div().w(px(19.)).text_color(rgb(0xc6afff)).child(mark)).child(label).into_any_element()
    }

    fn projects_page(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let total = self.workspace.projects.len();
        let done = self.workspace.projects.iter().filter(|project| project.status == "已完成").count();
        let repair = self.workspace.projects.iter().filter(|project| matches!(project.status.as_str(), "处理失败" | "需修复")).count();
        let open = total.saturating_sub(done + repair);
        let mut rows = div().flex().flex_col();
        if self.workspace.projects.is_empty() {
            rows = rows.child(div().p(px(28.)).text_color(rgb(0x66647a)).child("还没有录音项目."));
        }
        for index in 0..self.workspace.projects.len() {
            let project = &self.workspace.projects[index];
            let id = project.id.clone();
            let title = if project.name.trim().is_empty() { "未命名项目".to_string() } else { project.name.clone() };
            let mark = title.chars().next().unwrap_or('项').to_string();
            let place = [project.date.as_str(), project.city.as_str(), project.venue.as_str()]
                .into_iter()
                .filter(|item| !item.trim().is_empty())
                .collect::<Vec<_>>()
                .join(" · ");
            let meta = if place.is_empty() {
                format!("{} 个文件", project.sources.len())
            } else {
                format!("{place}  ·  {} 个文件", project.sources.len())
            };
            let status = project.status.clone();
            let last = index + 1 == total;
            rows = rows.child(
                div()
                    .px(px(19.))
                    .py(px(11.))
                    .border_t_1()
                    .border_color(rgb(0xede9f3))
                    .flex()
                    .flex_row()
                    .items_center()
                    .gap(px(10.))
                    .child(
                        div()
                            .w(px(34.))
                            .h(px(34.))
                            .rounded(px(8.))
                            .bg(rgb(0xeee6fb))
                            .text_color(rgb(0x6446a1))
                            .flex()
                            .items_center()
                            .justify_center()
                            .child(mark),
                    )
                    .child(
                        div()
                            .flex_1()
                            .min_w(px(0.))
                            .flex()
                            .flex_col()
                            .gap(px(3.))
                            .child(title)
                            .child(div().text_size(px(12.)).text_color(rgb(0x66647a)).child(meta)),
                    )
                    .child(status_badge(&status))
                    .child(Button::new(SharedString::from(format!("open-{id}"))).small().label("打开").on_click(cx.listener(move |this, _, window, cx| this.select_project(index, window, cx))))
                    .child(Button::new(SharedString::from(format!("up-{id}"))).xsmall().ghost().label("上移").disabled(index == 0).on_click(cx.listener(move |this, _, window, cx| this.move_project(index, -1, window, cx))))
                    .child(Button::new(SharedString::from(format!("down-{id}"))).xsmall().ghost().label("下移").disabled(last).on_click(cx.listener(move |this, _, window, cx| this.move_project(index, 1, window, cx))))
                    .child(Button::new(SharedString::from(format!("del-{id}"))).xsmall().ghost().danger().label("删除").on_click(cx.listener(move |this, _, window, cx| this.delete_project(index, window, cx)))),
            );
        }
        div()
            .flex()
            .flex_col()
            .gap(px(16.))
            .child(
                div()
                    .p(px(29.))
                    .rounded(px(18.))
                    .bg(rgb(0xeee8fa))
                    .border_1()
                    .border_color(rgb(0xe5dcf5))
                    .flex()
                    .flex_row()
                    .items_center()
                    .justify_between()
                    .gap(px(22.))
                    .child(
                        div()
                            .flex()
                            .flex_col()
                            .gap(px(8.))
                            .child(div().text_size(px(28.)).child("录音项目"))
                            .child(div().text_color(rgb(0x5c5470)).child("归档演出录音, 管理分轨, 专辑标签, 封面和处理进度.")),
                    )
                    .child(
                        div()
                            .flex()
                            .flex_row()
                            .gap_2()
                            .child(Button::new("open-file").label("打开项目文件").on_click(cx.listener(|this, _, window, cx| this.open_file(window, cx))))
                            .child(Button::new("save-file").label("保存为项目文件").on_click(cx.listener(|this, _, window, cx| this.save_file(window, cx)))),
                    ),
            )
            .child(
                div()
                    .flex()
                    .flex_row()
                    .gap(px(12.))
                    .child(stat_card(total, "全部项目", "▤"))
                    .child(stat_card(open, "整理中", "◷"))
                    .child(stat_card(done, "已完成", "✓"))
                    .child(stat_card(repair, "待修复", "◇")),
            )
            .child(
                div()
                    .bg(rgb(0xffffff))
                    .border_1()
                    .border_color(rgb(0xe5e1ed))
                    .rounded(px(14.))
                    .flex()
                    .flex_col()
                    .child(
                        div()
                            .px(px(19.))
                            .py(px(14.))
                            .flex()
                            .flex_row()
                            .items_center()
                            .justify_between()
                            .child(div().text_size(px(16.)).child("项目列表"))
                            .child(Button::new("new-project").primary().label("新建项目").on_click(cx.listener(|this, _, window, cx| this.create_project(window, cx)))),
                    )
                    .child(rows)
                    .child(div().px(px(19.)).py(px(11.)).border_t_1().border_color(rgb(0xede9f3)).text_size(px(11.)).text_color(rgb(0x66647a)).child("删除项目只移出工作台, 不删除磁盘上的录音和导出文件.")),
            )
    }

    fn create_project(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.pull(cx);
        self.workspace.projects.push(Project::blank());
        self.project_index = Some(self.workspace.projects.len() - 1);
        self.track_index = Some(0);
        self.page = Page::Split;
        self.push(window, cx);
        self.mark_dirty(cx);
    }

    fn move_project(&mut self, index: usize, delta: isize, window: &mut Window, cx: &mut Context<Self>) {
        self.pull(cx);
        let next = index as isize + delta;
        if next < 0 || next as usize >= self.workspace.projects.len() {
            return;
        }
        self.workspace.projects.swap(index, next as usize);
        if self.project_index == Some(index) {
            self.project_index = Some(next as usize);
        }
        self.push(window, cx);
        self.mark_dirty(cx);
    }

    fn delete_project(&mut self, index: usize, window: &mut Window, cx: &mut Context<Self>) {
        let id = self.workspace.projects.get(index).map(|project| project.id.clone());
        let Some(id) = id else { return };
        self.pull(cx);
        self.workspace.logs.retain(|log| log.project_id != id);
        self.workspace.projects.remove(index);
        if self.workspace.projects.is_empty() {
            self.project_index = None;
            self.track_index = None;
        } else if self.project_index.map(|current| current >= self.workspace.projects.len()).unwrap_or(true) {
            self.project_index = Some(self.workspace.projects.len() - 1);
            self.track_index = Some(0);
        }
        self.push(window, cx);
        self.mark_dirty(cx);
    }

    fn open_file(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.pull(cx);
        let Some(path) = rfd::FileDialog::new().set_title("打开项目文件").add_filter("项目", &["json"]).pick_file() else {
            return;
        };
        let file = match store::read_project_file(&path) {
            Ok(file) => file,
            Err(error) => {
                self.show_error(error.to_string(), cx);
                return;
            }
        };
        let source_id = file.project.id.clone();
        match crate::model::import_project(&mut self.workspace, file) {
            crate::model::ProjectImport::Existing => {
                let index = self.workspace.projects.iter().position(|project| project.id == source_id).unwrap_or(0);
                self.select_project(index, window, cx);
                self.status = "已打开现有项目.".into();
                return;
            }
            crate::model::ProjectImport::Copy => self.status = "已作为副本打开项目文件.".into(),
            crate::model::ProjectImport::New => self.status = "已打开项目文件.".into(),
        }
        self.project_index = Some(self.workspace.projects.len() - 1);
        self.track_index = Some(0);
        self.page = Page::Split;
        self.push(window, cx);
        self.mark_dirty(cx);
    }

    fn save_file(&mut self, _window: &mut Window, cx: &mut Context<Self>) {
        self.pull(cx);
        let Some(project) = self.project().cloned() else {
            self.show_error("请先建立项目.", cx);
            return;
        };
        let leaf = crate::names::sanitize_leaf(&format!("CR Studio - {}", if project.name.trim().is_empty() { "recording-project" } else { project.name.trim() }), 110);
        let Some(path) = rfd::FileDialog::new().set_title("保存项目文件").set_file_name(format!("{leaf}.crstudio.json")).save_file() else {
            return;
        };
        let file = crate::model::ProjectFile::from_project(&project, &self.workspace.logs);
        if let Err(error) = store::write_project_file(&path, &file) {
            self.show_error(error.to_string(), cx);
            return;
        }
        self.status = format!("已保存 {}", path.display()).into();
        cx.notify();
    }

    fn header(&self) -> impl IntoElement {
        let (title, detail) = page_copy(self.page);
        div()
            .h(px(64.))
            .px(px(30.))
            .bg(rgb(0xffffff))
            .border_b_1()
            .border_color(rgb(0xe5e1ed))
            .flex()
            .flex_row()
            .items_center()
            .justify_between()
            .gap(px(12.))
            .child(
                div()
                    .flex()
                    .flex_row()
                    .items_center()
                    .gap(px(12.))
                    .child(div().text_color(rgb(0x66647a)).child("工作区"))
                    .child(div().text_color(rgb(0x66647a)).child("/"))
                    .child(title)
                    .child(div().text_size(px(12.)).text_color(rgb(0x66647a)).child(detail)),
            )
            .child(div().text_size(px(12.)).text_color(rgb(0x66647a)).child(self.status.clone()))
    }

    fn footer(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let Some(error) = &self.error else {
            return div();
        };
        div()
            .mx(px(30.))
            .mb(px(16.))
            .px(px(11.))
            .py(px(9.))
            .rounded(px(9.))
            .bg(rgb(0xfff5e4))
            .border_1()
            .border_color(rgb(0xedd09b))
            .text_color(rgb(0x705018))
            .flex()
            .flex_row()
            .items_center()
            .justify_between()
            .gap_2()
            .child(error.clone())
            .child(Button::new("clear-error").xsmall().ghost().label("关闭").on_click(cx.listener(|this, _, _, cx| {
                this.error = None;
                cx.notify();
            })))
    }
}

impl Render for Studio {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let nav = div()
            .w(px(226.))
            .h_full()
            .px(px(16.))
            .pt(px(27.))
            .pb(px(18.))
            .flex()
            .flex_col()
            .bg(rgb(0x211b35))
            .text_color(rgb(0xf5efff))
            .child(
                div()
                    .flex()
                    .flex_row()
                    .items_center()
                    .gap(px(11.))
                    .px(px(5.))
                    .child(
                        div()
                            .w(px(38.))
                            .h(px(38.))
                            .rounded(px(11.))
                            .bg(rgb(0x7655ca))
                            .text_color(rgb(0xffffff))
                            .flex()
                            .items_center()
                            .justify_center()
                            .text_size(px(20.))
                            .child("♫"),
                    )
                    .child(
                        div()
                            .flex()
                            .flex_col()
                            .gap(px(4.))
                            .child("古典录音工作台")
                            .child(div().text_size(px(8.)).text_color(rgb(0xc7b7ea)).child("CLASSICAL RECORDINGS")),
                    ),
            )
            .child(
                div()
                    .mt(px(43.))
                    .flex()
                    .flex_col()
                    .gap(px(5.))
                    .child(div().px(px(12.)).mb(px(4.)).text_size(px(11.)).text_color(rgb(0xbcb0d2)).child("工作区"))
                    .child(self.nav_button("nav-projects", "▤", "录音项目", Page::Projects, cx))
                    .child(self.nav_button("nav-split", "✂", "整场录音分轨", Page::Split, cx))
                    .child(self.nav_button("nav-cd", "◉", "CD 抓轨整理", Page::Cd, cx))
                    .child(self.nav_button("nav-video", "▶", "封面视频", Page::Video, cx))
                    .child(self.nav_button("nav-history", "◷", "处理记录", Page::History, cx))
                    .child(self.nav_button("nav-templates", "▧", "模板与命名", Page::Templates, cx))
                    .child(self.nav_button("nav-llm", "✦", "模型配置", Page::Llm, cx)),
            )
            .child(div().flex_1())
            .child(
                div()
                    .pt(px(12.))
                    .px(px(8.))
                    .border_t_1()
                    .border_color(rgb(0x453b59))
                    .flex()
                    .flex_row()
                    .items_center()
                    .gap(px(9.))
                    .text_size(px(11.))
                    .text_color(rgb(0xd6c9e9))
                    .child(div().w(px(7.)).h(px(7.)).rounded(px(7.)).bg(rgb(0xa5e4be)))
                    .child("本机工作区"),
            )
            .into_any_element();
        let header = self.header().into_any_element();
        let footer = self.footer(cx).into_any_element();
        let page = match self.page {
            Page::Projects => self.projects_page(window, cx).into_any_element(),
            Page::Split => self.split_page(cx),
            Page::Cd => self.cd_page(cx),
            Page::Video => self.video_page(cx),
            Page::History => self.history_page(cx),
            Page::Templates => self.templates_page(cx),
            Page::Llm => self.llm_page(cx),
        };
        div()
            .flex()
            .flex_row()
            .size_full()
            .bg(cx.theme().background)
            .text_color(cx.theme().foreground)
            .child(nav)
            .child(
                div()
                    .flex_1()
                    .h_full()
                    .min_w(px(0.))
                    .flex()
                    .flex_col()
                    .child(header)
                    .child(div().id("page-body").flex_1().min_h(px(0.)).overflow_y_scroll().px(px(30.)).py(px(24.)).child(div().w_full().max_w(px(1540.)).child(page)))
                    .child(footer),
            )
    }
}

pub fn run() {
    application().with_assets(assets::Assets).run(|cx| {
        init(cx);
        apply_studio_theme(cx);
        let mut options = WindowOptions::default();
        options.window_bounds = Some(WindowBounds::centered(size(px(1280.), px(840.)), cx));
        open_window(options, cx, |window, cx| cx.new(|cx| Studio::new(window, cx))).expect("无法打开窗口");
    });
}

fn apply_studio_theme(cx: &mut App) {
    Theme::update(cx, |theme| {
        theme.background = rgb(0xf8f7fb).into();
        theme.foreground = rgb(0x222037).into();
        theme.border = rgb(0xe5e1ed).into();
        theme.primary = rgb(0x5a3da8).into();
        theme.primary_foreground = rgb(0xffffff).into();
        theme.primary_hover = rgb(0x4b2e93).into();
        theme.primary_active = rgb(0x3d2778).into();
        theme.muted_foreground = rgb(0x66647a).into();
        theme.radius = px(8.);
        theme.radius_lg = px(14.);
        theme.font_family = "Microsoft YaHei".into();
        theme.font_size = px(14.);
    });
}

pub(super) fn surface() -> Div {
    div().bg(rgb(0xffffff)).border_1().border_color(rgb(0xe5e1ed)).rounded(px(14.)).p(px(19.)).flex().flex_col().gap_2()
}

fn page_copy(page: Page) -> (&'static str, &'static str) {
    match page {
        Page::Projects => ("录音项目", "归档演出录音, 管理分轨, 专辑标签, 封面和处理进度."),
        Page::Split => ("整场录音分轨", "按时间点切开整场录音, 写入标签, 封面和增益."),
        Page::Cd => ("CD 抓轨整理", "读取 CUE 和分轨音频, 导入到当前项目."),
        Page::Video => ("封面视频", "用静态封面和音轨做成 1920x1080 的 H.264 视频."),
        Page::History => ("处理记录", "查看导出, 试听和导入留下的记录."),
        Page::Templates => ("模板与命名", "保存格式, 采样率, 增益, 后期和命名规则."),
        Page::Llm => ("模型配置", "API Key 只留在本机, 不写入项目文件."),
    }
}

fn stat_card(value: usize, label: &'static str, mark: &'static str) -> impl IntoElement {
    div()
        .flex_1()
        .flex()
        .flex_row()
        .items_center()
        .gap(px(12.))
        .p(px(14.))
        .bg(rgb(0xffffff))
        .border_1()
        .border_color(rgb(0xe5e1ed))
        .rounded(px(14.))
        .child(
            div()
                .w(px(36.))
                .h(px(36.))
                .rounded(px(9.))
                .bg(rgb(0xf0e9fb))
                .text_color(rgb(0x5b3a9c))
                .flex()
                .items_center()
                .justify_center()
                .child(mark),
        )
        .child(div().flex().flex_col().gap(px(4.)).child(div().text_size(px(24.)).child(value.to_string())).child(div().text_size(px(11.)).text_color(rgb(0x66647a)).child(label)))
}

fn status_badge(status: &str) -> Div {
    let (background, foreground) = match status {
        "已完成" => (0xe2f3e9, 0x256249),
        "处理失败" | "需修复" => (0xfce7eb, 0xa32747),
        "待核对时间点" => (0xfff0d7, 0x815617),
        _ => (0xf0eaf8, 0x5d3d98),
    };
    div().px(px(8.)).py(px(4.)).rounded(px(999.)).bg(rgb(background)).text_color(rgb(foreground)).text_size(px(11.)).child(status.to_string())
}

fn text(input: &Entity<InputState>, cx: &App) -> String {
    input.read(cx).value().to_string()
}

fn number(input: &Entity<InputState>, cx: &App, default: f64) -> f64 {
    let value = text(input, cx);
    if value.trim().is_empty() {
        default
    } else {
        value.trim().parse().unwrap_or(default)
    }
}

fn set_text(input: &Entity<InputState>, value: &str, window: &mut Window, cx: &mut App) {
    if input.read(cx).value().to_string() != value {
        input.update(cx, |state, cx| state.set_value(value, window, cx));
    }
}

fn fmt_num(value: f64) -> String {
    if value.fract() == 0.0 && value.abs() < 1_000_000.0 {
        format!("{}", value as i64)
    } else {
        let text = format!("{value:.4}");
        text.trim_end_matches('0').trim_end_matches('.').to_string()
    }
}

fn field(label: &'static str, input: &Entity<InputState>) -> Field {
    Field::new().label(label).child(Input::new(input).small())
}

pub(super) fn reports_message(reports: &[TrackReport]) -> String {
    let ok = reports.iter().filter(|item| item.error.is_none()).count();
    let failed = reports.len() - ok;
    let mut lines = vec![format!("完成 {ok} 轨, 失败 {failed} 轨.")];
    for item in reports.iter().filter(|item| item.error.is_some()).take(8) {
        lines.push(format!("{}: {}", item.target, item.error.as_deref().unwrap_or("")));
    }
    lines.join("\n")
}

