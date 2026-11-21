//! GPUI Kit 界面. 文本框, 按钮, 表单和对话框都用组件库, 不自己画输入法.

mod work;

use std::time::Duration;

use gpui_kit::component::{
    ActiveTheme as _, Disableable as _, Sizable as _,
    button::{Button, ButtonVariants as _},
    form::{Field, Form},
    input::{Input, InputEvent, InputState, TextareaState},
    Theme,
};
use gpui_kit::{
    App, AppContext as _, Context, Div, Entity, InteractiveElement as _, IntoElement, ParentElement, Render,
    SharedString, StatefulInteractiveElement as _, Styled, Subscription, Task, Window, WindowBounds,
    WindowOptions, application, assets, div, init, open_window, px, rgb, size,
};

use crate::chrome;
use crate::export::TrackReport;
use crate::ffmpeg::{self, Binaries, Cancel};
use crate::llm::{EnglishSuggestion, LlmConfig, NameHit, SessionCandidate};
use crate::program::ImportedProgram;
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
    Scripts,
    History,
    Templates,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Overlay {
    Cd,
    Video,
    Llm,
    Cover,
}

#[derive(Clone)]
pub(super) struct FileCandidate {
    pub title: String,
    pub composer: String,
    pub source: String,
    pub stem: String,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum DetailTab {
    Overview,
    Tracks,
    History,
}

fn page_from(place: chrome::Place) -> Page {
    match place {
        chrome::Place::Projects => Page::Projects,
        chrome::Place::Split => Page::Split,
        chrome::Place::Scripts => Page::Scripts,
        chrome::Place::History => Page::History,
        chrome::Place::Templates => Page::Templates,
    }
}

struct PlayedClip {
    path: std::path::PathBuf,
    origin: f64,
    span: f64,
    limited: bool,
}

enum JobMsg {
    Status(String),
    Done { operation: &'static str, result: Result<String, String> },
    English(Result<Vec<EnglishSuggestion>, String>),
    Works(Result<Vec<WorkHit>, String>),
    Play(Result<PlayedClip, String>),
    Import { cd: bool, include_tracks: bool, text: String, task: String, request: String, images: Vec<std::path::PathBuf> },
    Sessions(Result<Vec<SessionCandidate>, String>),
    Names(Result<Vec<NameHit>, String>),
    Parts(Result<String, String>),
    Groups(Result<Vec<crate::llm::MergeSuggestion>, String>),
    Models(Result<Vec<String>, String>),
    Exported { note: String, reports: Vec<crate::export::TrackReport> },
    Compare { raw: std::path::PathBuf, wet: std::path::PathBuf, note: String },
    CoverPreview(String),
    CoverPhoto(String),
    Artwork { back: bool, path: String, name: String },
    CoverDesign(crate::coverart::CoverDesign),
}

#[derive(Clone, Debug)]
pub(super) enum AiKind {
    Facts { include_tracks: bool, cd: bool },
    Names,
    Movements,
    Groups,
    English,
    Cover,
}

#[derive(Clone, Debug)]
pub(super) struct AiLine {
    pub key: String,
    pub label: String,
    pub on: bool,
    pub value: String,
    pub extra: String,
    pub show_extra: bool,
    pub start: String,
    pub end: String,
    pub indexes: Vec<usize>,
}

impl AiLine {
    pub(super) fn field(key: &str, label: &str, value: &str) -> Self {
        Self { key: key.into(), label: label.into(), on: true, value: value.into(), extra: String::new(), show_extra: false, start: String::new(), end: String::new(), indexes: Vec::new() }
    }

    pub(super) fn pair(key: &str, label: &str, value: &str, extra: &str) -> Self {
        Self { key: key.into(), label: label.into(), on: true, value: value.into(), extra: extra.into(), show_extra: true, start: String::new(), end: String::new(), indexes: Vec::new() }
    }

    pub(super) fn track(index: usize, title: &str, composer: &str, start: &str, end: &str) -> Self {
        Self { key: format!("track-{index}"), label: format!("曲目 {}", index + 1), on: true, value: title.into(), extra: composer.into(), show_extra: true, start: start.into(), end: end.into(), indexes: Vec::new() }
    }

    pub(super) fn group(label: &str, title: &str, indexes: Vec<usize>) -> Self {
        Self { key: "group".into(), label: label.into(), on: true, value: title.into(), extra: String::new(), show_extra: false, start: String::new(), end: String::new(), indexes }
    }
}

#[derive(Clone, Debug)]
pub(super) struct AiDraft {
    pub title: String,
    pub kind: AiKind,
    pub lines: Vec<AiLine>,
    pub retry_task: String,
    pub retry_text: String,
    pub retry_images: Vec<std::path::PathBuf>,
}

const PROGRAM_FIELDS: &[(&str, &str)] = &[
    ("name", "场次 / 专辑名"),
    ("date", "演出日期"),
    ("performer", "演奏家"),
    ("conductor", "指挥家"),
    ("composer", "作曲家"),
    ("orchestra", "乐团"),
    ("city", "城市"),
    ("venue", "音乐厅"),
    ("origin", "录音来源"),
];

struct ProgramRow {
    on: bool,
    title: Entity<InputState>,
    composer: Entity<InputState>,
    start: String,
    end: String,
}

struct AiField {
    key: String,
    label: String,
    on: bool,
    value: Entity<InputState>,
    extra: Entity<InputState>,
    show_extra: bool,
    start: String,
    end: String,
    indexes: Vec<usize>,
}

#[derive(Clone, Debug)]
struct AiRetry {
    task: String,
    text: String,
    images: Vec<std::path::PathBuf>,
}

pub struct Studio {
    workspace: Workspace,
    llm: LlmConfig,
    page: Page,
    overlay: Option<Overlay>,
    project_group: &'static str,
    detail: bool,
    detail_tab: DetailTab,
    sort_key: &'static str,
    more_filters: bool,
    filter_query: Entity<InputState>,
    filter_status: String,
    filter_origin: String,
    filter_date: String,
    filter_performer: String,
    filter_conductor: String,
    filter_composer: String,
    filter_city: String,
    filter_venue: String,
    playhead: f64,
    playing: bool,
    open_menu: Option<String>,
    preview_key: String,
    preview_miss: String,
    preview_duration: Option<f64>,
    play_origin: f64,
    play_span: f64,
    play_mark: std::time::Instant,
    play_generation: u64,
    pending_listen: Option<(f64, Option<f64>)>,
    movement_open: bool,
    intake_files: Vec<std::path::PathBuf>,
    intake_session: bool,
    program_open: bool,
    program_session: bool,
    program_text: Entity<TextareaState>,
    program_seen: String,
    program_files: Vec<std::path::PathBuf>,
    program_note: String,
    program_overwrite: bool,
    program_replace: bool,
    program_pending: Option<String>,
    program_inputs: Vec<Entity<InputState>>,
    program_rows: Vec<ProgramRow>,
    program_track_sig: String,
    session_hits: Vec<SessionCandidate>,
    name_hits: Vec<NameHit>,
    model_choices: Vec<String>,
    pending_import: Option<ImportedProgram>,
    pending_tracks: bool,
    pending_cd: bool,
    pending_parts: String,
    ai_draft: Option<AiDraft>,
    ai_fields: Vec<AiField>,
    ai_gen: u64,
    ai_field_gen: u64,
    ai_note: Entity<InputState>,
    ai_retry: Option<AiRetry>,
    stitch_on: bool,
    stitch_paths: Vec<std::path::PathBuf>,
    stitch_slices: Vec<crate::names::StitchSlice>,
    stitch_total: f64,
    stitch_at: f64,
    stitch_index: usize,
    stitch_virtual_origin: f64,
    stitch_note: bool,
    project_index: Option<usize>,
    track_index: Option<usize>,
    track_picks: std::collections::BTreeSet<usize>,
    ffmpeg: Option<Binaries>,
    cancel: Cancel,
    player: Player,
    busy: bool,
    ready: bool,
    save_scheduled: bool,
    dirty: bool,
    clean_workspace: Workspace,
    exit_ask: bool,
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
    tags: [Entity<InputState>; 11],
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
    tags_open: bool,
    cd_cue: bool,
    cd_files: Vec<(String, String)>,
    cd_rows: Vec<cue::CueTrack>,
    cd_discs: Vec<cue::DiscDraft>,
    cd_disc: Option<usize>,
    cd_notice: SharedString,
    preflight: String,
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
    movements: Entity<InputState>,
    mb_hits: Vec<WorkHit>,
    preview_raw: Option<std::path::PathBuf>,
    preview_wet: Option<std::path::PathBuf>,
    listen_source: String,
    exports: Vec<crate::export::TrackReport>,
    session_undo: Vec<(String, String, String)>,
    session_note: String,
    name_open: bool,
    name_update_tags: bool,
    name_choices: Vec<FileCandidate>,
    movement_parts: Vec<String>,
    movement_on: Vec<bool>,
    fx_open: bool,
    fx_track: Option<usize>,
    fx_ending: bool,
    fx_note: String,
    cover_layout: u8,
    cover_zoom: f32,
    cover_x: f32,
    cover_y: f32,
    cover_photo: String,
    cover_preview: String,
    cover_undo: Option<(String, String, String, u8)>,
    cover_pending: Option<crate::coverart::CoverDesign>,
    cover_title: Entity<InputState>,
    cover_performer: Entity<InputState>,
    cover_detail: Entity<InputState>,
    page_scroll: gpui_kit::ScrollHandle,
    page_scroll_grab: std::rc::Rc<std::cell::Cell<Option<f32>>>,
    save_task: Option<Task<()>>,
    job_task: Option<Task<()>>,
    progress_task: Option<Task<()>>,
    duration_task: Option<Task<()>>,
    clock_task: Option<Task<()>>,
    stitch_task: Option<Task<()>>,
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
        let date = input("年/月/日", cx);
        let album_date = input("专辑标签日期 / 年份 (CD)", cx);
        let city = input("城市", cx);
        let venue = input("音乐厅", cx);
        let origin = input("现场自录", cx);
        let output = input("留空则按场次命名", cx);
        let naming = input("{Track} - {Title}", cx);
        let notes = input("备注", cx);
        let start = input("00:00", cx);
        let end = input("自动", cx);
        let filename = input("留空时按命名规则生成", cx);
        let gain = input("0", cx);
        let width = input("100", cx);
        let reverb = input("0", cx);
        let tags = [
            input("例如 Piano Sonata No. 11", cx),
            input("Artist", cx),
            input("Album", cx),
            input("01", cx),
            input("Date", cx),
            input("Composer", cx),
            input("Conductor", cx),
            input("Soloist", cx),
            input("Orchestra", cx),
            input("Disc", cx),
            input("TotalDiscs", cx),
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
        let movements = input("I. Allegro | II. Adagio | III. Rondo", cx);
        let ai_note = input("写下要改的地方, 再打回重新识别", cx);
        let cover_title = input("专辑标题", cx);
        let cover_performer = input("演出者", cx);
        let cover_detail = input("音乐厅, 城市与日期", cx);
        let filter_query = input("项目、人物、曲目或文件名", cx);
        drop(input);
        let llm = store::load_llm();
        let llm_base = cx.new(|cx| InputState::new(window, cx).placeholder("https://api.deepseek.com").default_value(llm.base_url.clone()));
        let llm_model = cx.new(|cx| InputState::new(window, cx).placeholder("deepseek-flash").default_value(llm.model.clone()));
        let llm_key = cx.new(|cx| InputState::new(window, cx).placeholder("API Key").default_value(llm.api_key.clone()).masked(true));
        let program_text = cx.new(|cx| TextareaState::new(window, cx).placeholder("音乐会: 钢琴独奏会\n演奏家: 姓名\n日期: 2026-11-06\n音乐厅: 上海交响音乐厅\n1. 曲名"));
        let program_inputs = (0..PROGRAM_FIELDS.len()).map(|_| cx.new(|cx| InputState::new(window, cx))).collect::<Vec<_>>();
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
        subscriptions.push(cx.subscribe_in(&filter_query, window, |_, _, event, _, cx| {
            if matches!(event, InputEvent::Change) {
                cx.notify();
            }
        }));
        for field in [cover_title.clone(), cover_performer.clone(), cover_detail.clone()] {
            subscriptions.push(cx.subscribe_in(&field, window, |_, _, event, _, cx| {
                if matches!(event, InputEvent::Change) {
                    cx.notify();
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
        let clean_workspace = workspace.clone();
        let mut studio = Self {
            workspace,
            llm,
            page: Page::Projects,
            overlay: None,
            project_group: "all",
            detail: false,
            detail_tab: DetailTab::Overview,
            sort_key: "recent",
            more_filters: false,
            filter_query,
            filter_status: String::new(),
            filter_origin: String::new(),
            filter_date: String::new(),
            filter_performer: String::new(),
            filter_conductor: String::new(),
            filter_composer: String::new(),
            filter_city: String::new(),
            filter_venue: String::new(),
            playhead: 0.0,
            playing: false,
            open_menu: None,
            preview_key: String::new(),
            preview_miss: String::new(),
            preview_duration: None,
            play_origin: 0.0,
            play_span: 0.0,
            play_mark: std::time::Instant::now(),
            play_generation: 0,
            pending_listen: None,
            movement_open: false,
            intake_files: Vec::new(),
            intake_session: false,
            program_open: false,
            program_session: false,
            program_text,
            program_seen: String::new(),
            program_files: Vec::new(),
            program_note: String::new(),
            program_overwrite: false,
            program_replace: false,
            program_pending: None,
            program_inputs,
            program_rows: Vec::new(),
            program_track_sig: String::new(),
            session_hits: Vec::new(),
            name_hits: Vec::new(),
            model_choices: Vec::new(),
            pending_import: None,
            pending_tracks: false,
            pending_cd: false,
            pending_parts: String::new(),
            ai_draft: None,
            ai_fields: Vec::new(),
            ai_gen: 0,
            ai_field_gen: 0,
            ai_note,
            ai_retry: None,
            stitch_on: false,
            stitch_paths: Vec::new(),
            stitch_slices: Vec::new(),
            stitch_total: 0.0,
            stitch_at: 0.0,
            stitch_index: 0,
            stitch_virtual_origin: 0.0,
            stitch_note: false,
            project_index,
            track_index,
            track_picks: std::collections::BTreeSet::new(),
            ffmpeg,
            cancel: Cancel::default(),
            player: Player::new(),
            busy: false,
            ready: false,
            save_scheduled: false,
            dirty: false,
            clean_workspace,
            exit_ask: false,
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
            tags_open: false,
            cd_cue: true,
            cd_files: Vec::new(),
            cd_rows: Vec::new(),
            cd_discs: Vec::new(),
            cd_disc: None,
            cd_notice: "可以一次选择多张碟的整轨和 CUE, 或粘贴带轨号的文字.".into(),
            preflight: String::new(),
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
            movements,
            mb_hits: Vec::new(),
            preview_raw: None,
            preview_wet: None,
            listen_source: String::new(),
            exports: Vec::new(),
            session_undo: Vec::new(),
            session_note: String::new(),
            name_open: false,
            name_update_tags: false,
            name_choices: Vec::new(),
            movement_parts: Vec::new(),
            movement_on: Vec::new(),
            fx_open: false,
            fx_track: None,
            fx_ending: false,
            fx_note: String::new(),
            cover_layout: 0,
            cover_zoom: 1.0,
            cover_x: 0.5,
            cover_y: 0.5,
            cover_photo: String::new(),
            cover_preview: String::new(),
            cover_undo: None,
            cover_pending: None,
            cover_title,
            cover_performer,
            cover_detail,
            page_scroll: gpui_kit::ScrollHandle::new(),
            page_scroll_grab: std::rc::Rc::new(std::cell::Cell::new(None)),
            save_task: None,
            job_task: None,
            progress_task: None,
            duration_task: None,
            clock_task: None,
            stitch_task: None,
            _subscriptions: subscriptions,
        };
        studio.push(window, cx);
        studio.ready = true;
        let view = cx.entity().downgrade();
        window.on_window_should_close(cx, move |_window, app| {
            let Some(view) = view.upgrade() else {
                return true;
            };
            if !chrome::veto_close(view.read(app).dirty) {
                return true;
            }
            view.update(app, |studio, cx| {
                studio.pull(cx);
                studio.exit_ask = true;
                cx.notify();
            });
            false
        });
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
        self.dirty = true;
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

    pub(super) fn save_now(&mut self, cx: &mut Context<Self>) -> bool {
        self.pull(cx);
        self.save_task = None;
        if let Err(error) = store::save_workspace(&self.workspace) {
            self.show_error(error.to_string(), cx);
            return false;
        }
        self.clean_workspace = self.workspace.clone();
        self.dirty = false;
        self.save_scheduled = false;
        self.status = "已保存.".into();
        cx.notify();
        true
    }

    fn exit_save(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.save_now(cx) {
            self.exit_ask = false;
            window.remove_window();
        }
    }

    fn exit_discard(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.save_task = None;
        self.workspace = self.clean_workspace.clone();
        if let Err(error) = store::save_workspace(&self.workspace) {
            self.show_error(error.to_string(), cx);
            return;
        }
        self.dirty = false;
        self.save_scheduled = false;
        self.exit_ask = false;
        window.remove_window();
    }

    fn exit_cancel(&mut self, cx: &mut Context<Self>) {
        self.exit_ask = false;
        cx.notify();
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
            if track.parts.len() < 2 {
                track.start = start;
                track.end = end;
            }
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
        self.track_picks.clear();
        self.track_index = self.project().and_then(|project| if project.tracks.is_empty() { None } else { Some(0) });
        self.page = Page::Split;
        self.detail = false;
        self.overlay = None;
        self.push(window, cx);
        cx.notify();
    }

    fn open_detail(&mut self, index: usize, window: &mut Window, cx: &mut Context<Self>) {
        self.pull(cx);
        self.project_index = Some(index);
        self.track_picks.clear();
        self.track_index = self.project().and_then(|project| if project.tracks.is_empty() { None } else { Some(0) });
        self.detail = true;
        self.detail_tab = DetailTab::Overview;
        self.page = Page::Projects;
        self.overlay = None;
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
                self.stage_english(cx);
            }
            JobMsg::English(Err(text)) => self.show_error(text, cx),
            JobMsg::Works(Ok(items)) => {
                self.mb_hits = items;
                if self.name_open {
                    self.rebuild_name_choices();
                }
                self.status = format!("MusicBrainz 返回 {} 条.", self.mb_hits.len()).into();
            }
            JobMsg::Works(Err(text)) => self.show_error(text, cx),
            JobMsg::Import { cd, include_tracks, text, task, request, images } => {
                if self.program_open && !cd {
                    self.program_pending = Some(text);
                    self.program_note = "模型已返回. 请在弹窗里核对后再填入.".into();
                } else {
                    self.stage_import(cd, include_tracks, &text, &task, &request, images, cx);
                }
            }
            JobMsg::Sessions(Ok(items)) => {
                self.session_hits = items;
                let note = if self.session_hits.is_empty() { "没有找到有来源的场次.".into() } else { format!("找到 {} 个场次候选. 点一条就填入核对表.", self.session_hits.len()) };
                if self.program_open {
                    self.program_note = note.clone();
                }
                self.status = note.into();
            }
            JobMsg::Sessions(Err(text)) => self.show_error(text, cx),
            JobMsg::Names(Ok(items)) => {
                self.name_hits = items;
                if self.name_open {
                    if matches!(self.ai_draft.as_ref().map(|draft| &draft.kind), Some(AiKind::Names)) {
                        self.ai_draft = None;
                        self.ai_gen = self.ai_gen.wrapping_add(1);
                    }
                    self.rebuild_name_choices();
                    self.status = if self.name_hits.is_empty() { "模型没有给出足够明确的曲名. 点列表里的一条就会填入.".into() } else { "点一条就填入当前曲目.".into() };
                } else {
                    self.stage_names();
                }
            }
            JobMsg::Names(Err(text)) => self.show_error(text, cx),
            JobMsg::Parts(Ok(text)) => {
                self.movement_parts = crate::program::movement_titles(&text);
                self.movement_on = vec![true; self.movement_parts.len()];
                self.pending_parts = text;
                self.movement_open = true;
                self.stage_parts();
            }
            JobMsg::Parts(Err(text)) => self.show_error(text, cx),
            JobMsg::Models(Ok(names)) => {
                self.model_choices = names;
                self.status = format!("已读取 {} 个模型, 可在模型页点选.", self.model_choices.len()).into();
            }
            JobMsg::Models(Err(text)) => self.show_error(text, cx),
            JobMsg::Play(Ok(clip)) => {
                self.playhead = clip.origin;
                if let Err(error) = self.player.play(clip.path) {
                    self.playing = false;
                    self.show_error(error.to_string(), cx);
                } else {
                    let limited = clip.limited;
                    self.begin_clock(clip.origin, clip.span, cx);
                    if limited {
                        self.status = "这个格式先试听 20 秒.".into();
                    }
                }
            }
            JobMsg::Play(Err(text)) => {
                self.playing = false;
                self.show_error(text, cx);
            }
            JobMsg::Exported { note, reports } => {
                let mut message = reports_message(&reports);
                if !note.is_empty() {
                    message = format!("{note}\n{message}");
                }
                self.exports = reports;
                self.status = message.clone().into();
                self.record("导出专辑", "成功", message, cx);
            }
            JobMsg::Compare { raw, wet, note } => {
                self.preview_raw = Some(raw);
                self.preview_wet = Some(wet);
                self.fx_note = note;
                self.fx_open = true;
                self.status = self.fx_note.clone().into();
            }
            JobMsg::CoverPreview(path) => {
                self.cover_preview = path;
                self.status = "封面预览已更新. 1600 x 1600 PNG.".into();
            }
            JobMsg::CoverPhoto(path) => {
                self.cover_photo = path;
                self.status = "已读取场照. 可以调整裁剪后更新预览.".into();
            }
            JobMsg::Artwork { back, path, name } => {
                if let Some(project) = self.project_mut() {
                    if back {
                        project.back_cover_file = name;
                        project.back_cover_path = path;
                    } else {
                        project.cover_file = name;
                        project.cover_path = path;
                        project.cover_status = "已准备".into();
                    }
                }
                self.status = "封面已准备.".into();
                self.mark_dirty(cx);
            }
            JobMsg::Groups(Ok(groups)) => self.stage_groups(groups),
            JobMsg::Groups(Err(text)) => self.show_error(text, cx),
            JobMsg::CoverDesign(design) => self.stage_cover(design),
        }
        cx.notify();
    }

    fn handle_key(&mut self, event: &gpui_kit::KeyDownEvent, window: &mut Window, cx: &mut Context<Self>) {
        let lookup = self.lookup_focused(window, cx);
        let command = chrome::resolve(
            &event.keystroke.key,
            chrome::Mods {
                primary: event.keystroke.modifiers.platform,
                shift: event.keystroke.modifiers.shift,
                alt: event.keystroke.modifiers.alt,
                function: event.keystroke.modifiers.function,
            },
            chrome::Focus {
                typing: event.prefer_character_input,
                dialog: self.layer_open(),
                lookup,
                preview: self.page == Page::Split && !self.layer_open(),
                composing: event.keystroke.is_ime_in_progress(),
            },
        );
        let Some(command) = command else {
            return;
        };
        if chrome::keeps_text_editing(command) {
            return;
        }
        match command {
            chrome::Command::Save => {
                cx.stop_propagation();
                self.save_now(cx);
            }
            chrome::Command::Confirm => self.search_work(cx),
            chrome::Command::Dismiss => {
                if self.exit_ask {
                    self.exit_cancel(cx);
                } else {
                    self.dismiss_layer(cx);
                }
            }
            chrome::Command::PlayPause => self.toggle_preview(cx),
            chrome::Command::SeekBack => self.seek_preview(-chrome::SEEK_SECONDS, cx),
            chrome::Command::SeekForward => self.seek_preview(chrome::SEEK_SECONDS, cx),
            chrome::Command::Copy | chrome::Command::Cut | chrome::Command::Paste | chrome::Command::SelectAll | chrome::Command::Undo | chrome::Command::Redo => {}
        }
    }

    fn lookup_focused(&self, window: &Window, cx: &App) -> bool {
        use gpui_kit::Focusable as _;
        self.mb_query.read(cx).focus_handle(cx).is_focused(window)
    }

    fn export_backup(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.pull(cx);
        let text = match store::export_backup(&self.workspace) {
            Ok(text) => text,
            Err(error) => {
                self.show_error(error.to_string(), cx);
                return;
            }
        };
        defer_dialog(window, cx, || rfd::FileDialog::new().set_title(chrome::label::EXPORT_BACKUP).set_file_name("classical-recording-studio.json").save_file(), move |this, path, _, cx| {
            let Some(path) = path else { return };
            if let Err(error) = std::fs::write(&path, text) {
                this.show_error(error.to_string(), cx);
                return;
            }
            this.status = format!("已导出备份 {}", path.display()).into();
            cx.notify();
        });
    }

    fn import_backup(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        defer_dialog(window, cx, || rfd::FileDialog::new().set_title(chrome::label::IMPORT_BACKUP).add_filter("JSON", &["json"]).pick_file(), |this, path, window, cx| {
        let Some(path) = path else { return };
        let text = match std::fs::read_to_string(&path) {
            Ok(text) => text,
            Err(error) => {
                this.show_error(error.to_string(), cx);
                return;
            }
        };
        let workspace = match store::import_backup(&text) {
            Ok(workspace) => workspace,
            Err(error) => {
                this.show_error(error.to_string(), cx);
                return;
            }
        };
        this.workspace = workspace;
        this.project_index = if this.workspace.projects.is_empty() { None } else { Some(0) };
        this.track_index = this.project().and_then(|project| if project.tracks.is_empty() { None } else { Some(0) });
        this.track_picks.clear();
        this.overlay = None;
        this.page = Page::Projects;
        this.detail = false;
        this.project_group = "all";
        this.push(window, cx);
        this.status = "已导入备份.".into();
        this.mark_dirty(cx);
        });
    }

    fn nav_button(&self, id: &'static str, mark: &'static str, label: &'static str, page: Page, cx: &mut Context<Self>) -> gpui_kit::AnyElement {
        let selected = self.page == page && self.overlay.is_none();
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
                this.overlay = None;
                if page == Page::Projects {
                    this.detail = false;
                }
                cx.notify();
            }));
        if selected {
            item = item.bg(rgb(0x42335f));
        }
        item.child(div().w(px(19.)).text_color(rgb(0xc6afff)).child(mark)).child(label).into_any_element()
    }

    fn projects_page(&mut self, window: &mut Window, cx: &mut Context<Self>) -> Div {
        if self.detail && self.project().is_some() {
            self.project_detail(window, cx)
        } else {
            self.project_board(window, cx)
        }
    }

    fn project_board(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> Div {
        let total = self.workspace.projects.len();
        let open = self.workspace.projects.iter().filter(|project| chrome::in_group(&project.status, "open")).count();
        let done = self.workspace.projects.iter().filter(|project| chrome::in_group(&project.status, "complete")).count();
        let repair = self.workspace.projects.iter().filter(|project| chrome::in_group(&project.status, "repair")).count();
        let listed = self.listed_projects(cx);
        let shown = listed.len();
        let manual = self.sort_key == "manual";
        let mut rows = div().flex().flex_col().child(project_head());
        if listed.is_empty() {
            rows = rows.child(div().p(px(28.)).text_color(rgb(0x66647a)).child(chrome::label::EMPTY_PROJECTS));
        }
        for index in listed {
            rows = rows.child(self.project_row(index, total, manual, cx));
        }
        let mut filters = div()
            .flex()
            .flex_col()
            .gap_2()
            .child(Form::new().child(field("搜索项目", &self.filter_query)))
            .child(self.filter_line("status", "项目状态", cx))
            .child(self.filter_line("origin", "录音来源", cx))
            .child(self.filter_line("date", "演出日期", cx));
        if self.more_filters {
            filters = filters
                .child(self.filter_line("performer", "演奏家", cx))
                .child(self.filter_line("conductor", "指挥家", cx))
                .child(self.filter_line("composer", "作曲家", cx))
                .child(self.filter_line("city", "城市", cx))
                .child(self.filter_line("venue", "音乐厅", cx));
        }
        filters = filters.child(
            div()
                .flex()
                .flex_row()
                .gap_2()
                .child(Button::new("more-filters").small().ghost().label(chrome::label::MORE_FILTER).on_click(cx.listener(|this, _, _, cx| {
                    this.more_filters = !this.more_filters;
                    cx.notify();
                })))
                .child(Button::new("clear-filters").small().ghost().label(chrome::label::CLEAR_FILTER).on_click(cx.listener(|this, _, window, cx| this.clear_filters(window, cx)))),
        );
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
                            .child(div().text_size(px(28.)).child(chrome::heading(chrome::Place::Projects).0))
                            .child(div().text_color(rgb(0x5c5470)).child(chrome::heading(chrome::Place::Projects).1)),
                    )
                    .child(div().flex().flex_row().gap_2().child(Button::new("open-file").label(chrome::label::OPEN_PROJECT).on_click(cx.listener(|this, _, window, cx| this.open_file(window, cx))))),
            )
            .child(
                div()
                    .flex()
                    .flex_row()
                    .gap(px(12.))
                    .child(stat_card(total, "all", "全部项目", "▤", self.project_group == "all", cx))
                    .child(stat_card(open, "open", "整理中", "◷", self.project_group == "open", cx))
                    .child(stat_card(done, "complete", "已完成", "✓", self.project_group == "complete", cx))
                    .child(stat_card(repair, "repair", "待修复", "◇", self.project_group == "repair", cx)),
            )
            .child(div().bg(rgb(0xffffff)).border_1().border_color(rgb(0xe5e1ed)).rounded(px(14.)).p(px(19.)).flex().flex_col().gap_2().child(filters))
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
                            .gap_2()
                            .child(div().text_size(px(16.)).child("项目列表"))
                            .child(div().flex().flex_row().items_center().gap_2().child(self.sort_bar(cx)).child(Button::new("new-project").primary().label(chrome::label::NEW_PROJECT).on_click(cx.listener(|this, _, window, cx| this.create_project(window, cx))))),
                    )
                    .child(rows)
                    .child(div().px(px(19.)).py(px(11.)).border_t_1().border_color(rgb(0xede9f3)).text_size(px(11.)).text_color(rgb(0x66647a)).child(format!("显示 {shown} / {total} 个项目"))),
            )
    }

    fn project_detail(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> Div {
        let project = self.project().cloned().expect("opened project");
        let index = self.project_index.expect("opened project");
        let title = if project.name.trim().is_empty() { "未命名项目".to_string() } else { project.name.clone() };
        let place = [project.date.as_str(), project.city.as_str(), project.venue.as_str()].into_iter().filter(|item| !item.trim().is_empty()).collect::<Vec<_>>().join(" · ");
        let subtitle = if place.is_empty() { "—".to_string() } else { place };
        let body = match self.detail_tab {
            DetailTab::Overview => self.overview_cards(&project, cx),
            DetailTab::Tracks => self.track_table(&project, cx),
            DetailTab::History => self.project_history(&project),
        };
        div()
            .flex()
            .flex_col()
            .gap(px(16.))
            .child(Button::new("back-projects").small().ghost().label(chrome::label::BACK_LIST).on_click(cx.listener(|this, _, _, cx| {
                this.pull(cx);
                this.detail = false;
                this.mark_dirty(cx);
            })))
            .child(
                div()
                    .flex()
                    .flex_row()
                    .items_center()
                    .justify_between()
                    .gap(px(12.))
                    .child(div().flex().flex_col().gap(px(6.)).child(div().text_size(px(28.)).child(title)).child(div().text_color(rgb(0x66647a)).child(subtitle)))
                    .child(
                        div()
                            .flex()
                            .flex_row()
                            .gap_2()
                            .child(Button::new("detail-save").label(chrome::label::SAVE_LOCAL).on_click(cx.listener(|this, _, window, cx| this.save_file(window, cx))))
                            .child(Button::new("detail-edit").label(chrome::label::EDIT_PROJECT).on_click(cx.listener(|this, _, _, cx| {
                                this.detail_tab = DetailTab::Overview;
                                cx.notify();
                            })))
                            .child(Button::new("detail-split").label(chrome::label::OPEN_SPLIT).on_click(cx.listener(move |this, _, window, cx| this.select_project(index, window, cx))))
                            .child(Button::new("detail-script").primary().label(chrome::label::OPEN_SCRIPT).on_click(cx.listener(|this, _, _, cx| {
                                this.pull(cx);
                                this.page = Page::Scripts;
                                this.overlay = None;
                                cx.notify();
                            }))),
                    ),
            )
            .child(
                div()
                    .flex()
                    .flex_row()
                    .gap_2()
                    .child(self.detail_tab_button(DetailTab::Overview, chrome::detail_tabs()[0], cx))
                    .child(self.detail_tab_button(DetailTab::Tracks, chrome::detail_tabs()[1], cx))
                    .child(self.detail_tab_button(DetailTab::History, chrome::detail_tabs()[2], cx)),
            )
            .child(body)
    }

    fn project_row(&self, index: usize, total: usize, manual: bool, cx: &mut Context<Self>) -> Div {
        let project = &self.workspace.projects[index];
        let id = project.id.clone();
        let title = if project.name.trim().is_empty() { "未命名项目".to_string() } else { project.name.clone() };
        let mark = project.performer.chars().next().or_else(|| project.conductor.chars().next()).or_else(|| project.name.chars().next()).unwrap_or('项').to_string();
        let people = {
            let joined = [project.performer.as_str(), project.conductor.as_str()].into_iter().filter(|item| !item.trim().is_empty()).collect::<Vec<_>>().join(" · ");
            if joined.is_empty() { project.composer.clone() } else { joined }
        };
        let place = {
            let joined = [project.city.as_str(), project.venue.as_str()].into_iter().filter(|item| !item.trim().is_empty()).collect::<Vec<_>>().join(" · ");
            if joined.is_empty() { "—".to_string() } else { joined }
        };
        let date = if project.date.trim().is_empty() { "—".to_string() } else { project.date.clone() };
        let origin = if project.origin.trim().is_empty() { "—".to_string() } else { project.origin.clone() };
        let count = project.tracks.iter().filter(|track| !track.tags.title.trim().is_empty() || !track.source.trim().is_empty()).count();
        let status = project.status.clone();
        let last = index + 1 == total;
        let mut actions = div().flex().flex_row().gap_1().items_center();
        actions = actions.child(Button::new(SharedString::from(format!("open-{id}"))).xsmall().label(chrome::label::OPEN_ROW).on_click(cx.listener(move |this, _, window, cx| this.open_detail(index, window, cx))));
        if manual {
            actions = actions
                .child(Button::new(SharedString::from(format!("up-{id}"))).xsmall().ghost().label("上移").disabled(index == 0).on_click(cx.listener(move |this, _, window, cx| this.move_project(index, -1, window, cx))))
                .child(Button::new(SharedString::from(format!("down-{id}"))).xsmall().ghost().label("下移").disabled(last).on_click(cx.listener(move |this, _, window, cx| this.move_project(index, 1, window, cx))));
        }
        actions = actions.child(Button::new(SharedString::from(format!("del-{id}"))).xsmall().ghost().danger().label("删除").on_click(cx.listener(move |this, _, window, cx| this.delete_project(index, window, cx))));
        div()
            .px(px(19.))
            .py(px(10.))
            .border_t_1()
            .border_color(rgb(0xede9f3))
            .flex()
            .flex_row()
            .items_center()
            .gap(px(8.))
            .child(
                div()
                    .flex_1()
                    .min_w(px(0.))
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
                    .child(div().flex().flex_col().gap(px(3.)).child(title).child(div().text_size(px(12.)).text_color(rgb(0x66647a)).child(people))),
            )
            .child(div().w(px(110.)).child(date))
            .child(div().w(px(150.)).child(place))
            .child(div().w(px(90.)).child(origin))
            .child(div().w(px(48.)).child(count.to_string()))
            .child(div().w(px(88.)).child(status_badge(&status)))
            .child(actions)
    }

    fn sort_bar(&self, cx: &mut Context<Self>) -> Div {
        let mut row = div().flex().flex_row().gap_1();
        for (key, label) in chrome::sorts() {
            let selected = self.sort_key == key;
            row = if selected {
                row.child(Button::new(SharedString::from(format!("sort-{key}"))).xsmall().primary().label(label).on_click(cx.listener(move |this, _, _, cx| {
                    this.sort_key = key;
                    cx.notify();
                })))
            } else {
                row.child(Button::new(SharedString::from(format!("sort-{key}"))).xsmall().ghost().label(label).on_click(cx.listener(move |this, _, _, cx| {
                    this.sort_key = key;
                    cx.notify();
                })))
            };
        }
        row
    }

    fn detail_tab_button(&self, tab: DetailTab, label: &'static str, cx: &mut Context<Self>) -> gpui_kit::AnyElement {
        let selected = self.detail_tab == tab;
        let id = match tab {
            DetailTab::Overview => "tab-overview",
            DetailTab::Tracks => "tab-tracks",
            DetailTab::History => "tab-history",
        };
        if selected {
            Button::new(id).small().primary().label(label).on_click(cx.listener(move |this, _, _, cx| {
                this.detail_tab = tab;
                cx.notify();
            })).into_any_element()
        } else {
            Button::new(id).small().ghost().label(label).on_click(cx.listener(move |this, _, _, cx| {
                this.detail_tab = tab;
                cx.notify();
            })).into_any_element()
        }
    }

    fn overview_cards(&mut self, project: &Project, cx: &mut Context<Self>) -> Div {
        let mut files = div().flex().flex_col().gap_1();
        if project.sources.is_empty() {
            files = files.child(div().text_color(rgb(0x66647a)).child("还没有原始文件。"));
        } else {
            for source in &project.sources {
                files = files.child(crate::names::source_leaf(source));
            }
        }
        let audio = format!(
            "{} · {} Hz · 增益 {} · 声场 {} · 混响 {} · {}",
            project.settings.format,
            project.settings.rate,
            fmt_num(project.settings.gain),
            fmt_num(project.settings.width),
            fmt_num(project.settings.reverb),
            if project.cover_file.trim().is_empty() { "未选择封面".to_string() } else { project.cover_file.clone() }
        );
        div()
            .flex()
            .flex_col()
            .gap(px(12.))
            .child(
                info_card(chrome::label::SESSION_CARD)
                    .child(
                        div().flex().flex_row().items_center().gap_2().child(status_badge(&project.status)).child(
                            Button::new("cycle-project-status").xsmall().ghost().label("切换状态").on_click(cx.listener(|this, _, _, cx| this.cycle_status(cx))),
                        ),
                    )
                    .child(
                        Form::new()
                            .child(field("场次 / 专辑名", &self.name))
                            .child(field("演奏家", &self.performer))
                            .child(field("指挥家", &self.conductor))
                            .child(field("乐团", &self.orchestra))
                            .child(field("作曲家", &self.composer))
                            .child(field("演出日期", &self.date))
                            .child(field("专辑标签日期 / 年份（CD）", &self.album_date))
                            .child(field("城市", &self.city))
                            .child(field("音乐厅", &self.venue))
                            .child(field("录音来源", &self.origin)),
                    ),
            )
            .child(info_card(chrome::label::FILES_CARD).child(files))
            .child(info_card(chrome::label::AUDIO_CARD).child(audio))
    }

    fn track_table(&self, project: &Project, cx: &mut Context<Self>) -> Div {
        let columns = chrome::project_track_columns();
        let rule = if project.naming_rule.trim().is_empty() { crate::names::DEFAULT_NAMING } else { project.naming_rule.as_str() };
        let mut table = div().flex().flex_col().child(
            div()
                .flex()
                .flex_row()
                .items_center()
                .gap_2()
                .px(px(8.))
                .py(px(8.))
                .bg(rgb(0xf6f4fa))
                .text_size(px(11.))
                .text_color(rgb(0x66647a))
                .child(div().w(px(48.)).child(columns[0]))
                .child(div().flex_1().child(columns[1]))
                .child(div().w(px(140.)).child(columns[2]))
                .child(div().w(px(120.)).child(columns[3]))
                .child(div().flex_1().child(columns[4])),
        );
        for (index, track) in project.tracks.iter().enumerate() {
            let title = if track.tags.title.trim().is_empty() { "未命名曲目".to_string() } else { track.tags.title.clone() };
            let composer = track.tags.composer.clone();
            let source = if track.source.trim().is_empty() { "—".to_string() } else { crate::names::source_leaf(&track.source) };
            let span = if track.end.trim().is_empty() { track.start.clone() } else { format!("{} – {}", track.start, track.end) };
            let filename = crate::names::filename_for(rule, track, &track.tags, index);
            let last = index + 1 == project.tracks.len();
            let id = track.id.clone();
            table = table.child(
                div()
                    .flex()
                    .flex_row()
                    .items_center()
                    .gap_2()
                    .px(px(8.))
                    .py(px(8.))
                    .border_t_1()
                    .border_color(rgb(0xede9f3))
                    .child(div().w(px(48.)).child(format!("{:02}", index + 1)))
                    .child(div().flex_1().min_w(px(0.)).flex().flex_col().gap(px(2.)).child(title).child(div().text_size(px(12.)).text_color(rgb(0x66647a)).child(composer)))
                    .child(div().w(px(140.)).child(source))
                    .child(div().w(px(120.)).child(span))
                    .child(div().flex_1().min_w(px(0.)).child(filename))
                    .child(
                        div()
                            .flex()
                            .flex_row()
                            .gap_1()
                            .child(Button::new(SharedString::from(format!("edit-track-{id}"))).xsmall().label("编辑").on_click(cx.listener(move |this, _, window, cx| this.edit_track(index, window, cx))))
                            .child(Button::new(SharedString::from(format!("track-up-{id}"))).xsmall().ghost().label("↑").disabled(index == 0).on_click(cx.listener(move |this, _, window, cx| this.move_track(index, -1, window, cx))))
                            .child(Button::new(SharedString::from(format!("track-down-{id}"))).xsmall().ghost().label("↓").disabled(last).on_click(cx.listener(move |this, _, window, cx| this.move_track(index, 1, window, cx)))),
                    ),
            );
        }
        info_card(chrome::detail_tabs()[1]).child(table)
    }

    fn project_history(&self, project: &Project) -> Div {
        let mut table = div().flex().flex_col().child(history_head());
        let mut any = false;
        for entry in self.workspace.logs.iter().rev().filter(|entry| entry.project_id == project.id).take(80) {
            any = true;
            table = table.child(history_line(&entry.time, &entry.project_name, &entry.operation, &entry.result, &entry.message));
        }
        if !any {
            table = table.child(div().p(px(16.)).text_color(rgb(0x66647a)).child("还没有处理记录。"));
        }
        info_card(chrome::detail_tabs()[2]).child(table)
    }

    fn listed_projects(&self, cx: &mut Context<Self>) -> Vec<usize> {
        let query = text(&self.filter_query, cx);
        let filter = chrome::ListingFilter {
            group: self.project_group,
            query: &query,
            status: &self.filter_status,
            origin: &self.filter_origin,
            date: &self.filter_date,
            performer: &self.filter_performer,
            conductor: &self.filter_conductor,
            composer: &self.filter_composer,
            city: &self.filter_city,
            venue: &self.filter_venue,
        };
        let mut rows = Vec::new();
        for (index, project) in self.workspace.projects.iter().enumerate() {
            let extra = listing_extra(project);
            let row = chrome::Listing {
                name: &project.name,
                performer: &project.performer,
                conductor: &project.conductor,
                composer: &project.composer,
                date: &project.date,
                city: &project.city,
                venue: &project.venue,
                origin: &project.origin,
                status: &project.status,
                extra: &extra,
            };
            if chrome::listing_visible(&row, &filter) {
                rows.push(index);
            }
        }
        match self.sort_key {
            "oldest" => rows.sort_by(|&left, &right| oldest_key(&self.workspace.projects[left].date).cmp(&oldest_key(&self.workspace.projects[right].date))),
            "name" => rows.sort_by(|&left, &right| self.workspace.projects[left].name.cmp(&self.workspace.projects[right].name)),
            "manual" => {}
            _ => rows.sort_by(|&left, &right| self.workspace.projects[right].date.cmp(&self.workspace.projects[left].date)),
        }
        rows
    }

    fn filter_line(&self, key: &'static str, title: &'static str, cx: &mut Context<Self>) -> Div {
        let current = self.filter_value(key);
        let values = self.values_of(key);
        let mut chips = div().flex().flex_row().gap_1().items_center();
        chips = if current.is_empty() {
            chips.child(Button::new(SharedString::from(format!("filter-{key}-all"))).xsmall().primary().label("全部").on_click(cx.listener(move |this, _, _, cx| this.set_filter(key, String::new(), cx))))
        } else {
            chips.child(Button::new(SharedString::from(format!("filter-{key}-all"))).xsmall().ghost().label("全部").on_click(cx.listener(move |this, _, _, cx| this.set_filter(key, String::new(), cx))))
        };
        for (index, value) in values.into_iter().enumerate() {
            let picked = current == value;
            let label = value.clone();
            chips = if picked {
                let chosen = value.clone();
                chips.child(Button::new(SharedString::from(format!("filter-{key}-{index}"))).xsmall().primary().label(label.clone()).on_click(cx.listener(move |this, _, _, cx| this.set_filter(key, chosen.clone(), cx))))
            } else {
                let chosen = value;
                chips.child(Button::new(SharedString::from(format!("filter-{key}-{index}"))).xsmall().ghost().label(label).on_click(cx.listener(move |this, _, _, cx| this.set_filter(key, chosen.clone(), cx))))
            };
        }
        div().flex().flex_col().gap_1().child(div().text_size(px(12.)).text_color(rgb(0x66647a)).child(title)).child(chips)
    }

    fn filter_value(&self, key: &str) -> String {
        match key {
            "status" => self.filter_status.clone(),
            "origin" => self.filter_origin.clone(),
            "date" => self.filter_date.clone(),
            "performer" => self.filter_performer.clone(),
            "conductor" => self.filter_conductor.clone(),
            "composer" => self.filter_composer.clone(),
            "city" => self.filter_city.clone(),
            "venue" => self.filter_venue.clone(),
            _ => String::new(),
        }
    }

    fn values_of(&self, key: &str) -> Vec<String> {
        let mut out = Vec::new();
        for project in &self.workspace.projects {
            match key {
                "status" => push_unique(&mut out, &project.status),
                "origin" => push_unique(&mut out, &project.origin),
                "date" => push_unique(&mut out, &project.date),
                "performer" => push_unique(&mut out, &project.performer),
                "conductor" => push_unique(&mut out, &project.conductor),
                "composer" => {
                    for part in project.composer.split(" / ") {
                        push_unique(&mut out, part);
                    }
                    for track in &project.tracks {
                        push_unique(&mut out, &track.tags.composer);
                    }
                }
                "city" => push_unique(&mut out, &project.city),
                "venue" => push_unique(&mut out, &project.venue),
                _ => {}
            }
        }
        out
    }

    fn set_filter(&mut self, key: &'static str, value: String, cx: &mut Context<Self>) {
        let slot = match key {
            "status" => &mut self.filter_status,
            "origin" => &mut self.filter_origin,
            "date" => &mut self.filter_date,
            "performer" => &mut self.filter_performer,
            "conductor" => &mut self.filter_conductor,
            "composer" => &mut self.filter_composer,
            "city" => &mut self.filter_city,
            "venue" => &mut self.filter_venue,
            _ => return,
        };
        if value.is_empty() || *slot == value {
            slot.clear();
        } else {
            *slot = value;
        }
        cx.notify();
    }

    fn clear_filters(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.filter_status.clear();
        self.filter_origin.clear();
        self.filter_date.clear();
        self.filter_performer.clear();
        self.filter_conductor.clear();
        self.filter_composer.clear();
        self.filter_city.clear();
        self.filter_venue.clear();
        set_text(&self.filter_query, "", window, cx);
        cx.notify();
    }

    fn edit_track(&mut self, index: usize, window: &mut Window, cx: &mut Context<Self>) {
        self.pull(cx);
        self.track_index = Some(index);
        if let Some(track) = self.project().and_then(|project| project.tracks.get(index)) {
            self.playhead = crate::names::time_seconds(&track.start).ok().flatten().unwrap_or(0.0);
        }
        self.playing = false;
        self.detail = false;
        self.page = Page::Split;
        self.overlay = None;
        self.push(window, cx);
        cx.notify();
    }

    fn move_track(&mut self, index: usize, delta: isize, window: &mut Window, cx: &mut Context<Self>) {
        self.pull(cx);
        let Some(project) = self.project_mut() else { return };
        let next = index as isize + delta;
        if next < 0 || next as usize >= project.tracks.len() {
            return;
        }
        project.tracks.swap(index, next as usize);
        self.track_index = Some(next as usize);
        self.push(window, cx);
        self.mark_dirty(cx);
    }


    fn create_project(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.pull(cx);
        self.workspace.projects.push(Project::blank());
        self.project_index = Some(self.workspace.projects.len() - 1);
        self.track_index = Some(0);
        self.track_picks.clear();
        self.detail = true;
        self.detail_tab = DetailTab::Overview;
        self.page = Page::Projects;
        self.overlay = None;
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
        self.detail = false;
        self.push(window, cx);
        self.mark_dirty(cx);
    }

    fn open_file(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.pull(cx);
        defer_dialog(window, cx, || rfd::FileDialog::new().set_title("打开项目文件").add_filter("项目", &["json"]).pick_file(), |this, path, window, cx| {
        let Some(path) = path else { return };
        let file = match store::read_project_file(&path) {
            Ok(file) => file,
            Err(error) => {
                this.show_error(error.to_string(), cx);
                return;
            }
        };
        let source_id = file.project.id.clone();
        match crate::model::import_project(&mut this.workspace, file) {
            crate::model::ProjectImport::Existing => {
                let index = this.workspace.projects.iter().position(|project| project.id == source_id).unwrap_or(0);
                this.open_detail(index, window, cx);
                this.status = "已打开现有项目.".into();
                return;
            }
            crate::model::ProjectImport::Copy => this.status = "已作为副本打开项目文件.".into(),
            crate::model::ProjectImport::New => this.status = "已打开项目文件.".into(),
        }
        let index = this.workspace.projects.len() - 1;
        this.open_detail(index, window, cx);
        this.mark_dirty(cx);
        });
    }

    fn save_file(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.pull(cx);
        let Some(project) = self.project().cloned() else {
            self.show_error("请先建立项目.", cx);
            return;
        };
        let leaf = crate::names::sanitize_leaf(&format!("CR Studio - {}", if project.name.trim().is_empty() { "recording-project" } else { project.name.trim() }), 110);
        let logs = self.workspace.logs.clone();
        defer_dialog(window, cx, move || rfd::FileDialog::new().set_title("保存项目文件").set_file_name(format!("{leaf}.crstudio.json")).save_file(), move |this, path, _, cx| {
            let Some(path) = path else { return };
            let file = crate::model::ProjectFile::from_project(&project, &logs);
            if let Err(error) = store::write_project_file(&path, &file) {
                this.show_error(error.to_string(), cx);
                return;
            }
            this.status = format!("已保存 {}", path.display()).into();
            cx.notify();
        });
    }

    fn header(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let title = chrome::heading(self.place()).0;
        let mut tools = div().flex().flex_row().items_center().gap_2();
        if self.overlay.is_some() {
            tools = tools.child(Button::new("tool-close").xsmall().ghost().label(chrome::label::CLOSE).on_click(cx.listener(|this, _, _, cx| {
                this.overlay = None;
                cx.notify();
            })));
        }
        let save = Button::new("tool-save").xsmall().label(if self.dirty { "保存" } else { "已保存" }).on_click(cx.listener(|this, _, _, cx| {
            this.save_now(cx);
        }));
        tools = tools
            .child(if self.dirty { save.primary() } else { save })
            .child(Button::new("tool-llm").xsmall().ghost().label(chrome::label::LLM).on_click(cx.listener(|this, _, _, cx| {
                this.overlay = Some(Overlay::Llm);
                cx.notify();
            })))
            .child(Button::new("tool-llm-files").xsmall().ghost().label(chrome::label::LLM_FILES).on_click(cx.listener(|this, _, _, cx| {
                this.overlay = Some(Overlay::Llm);
                cx.notify();
            })))
            .child(Button::new("tool-backup").xsmall().ghost().label(chrome::label::EXPORT_BACKUP).on_click(cx.listener(|this, _, window, cx| this.export_backup(window, cx))))
            .child(Button::new("tool-restore").xsmall().ghost().label(chrome::label::IMPORT_BACKUP).on_click(cx.listener(|this, _, window, cx| this.import_backup(window, cx))))
            .child(div().text_size(px(12.)).text_color(rgb(0x66647a)).child(self.status.clone()));
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
                    .child(title),
            )
            .child(tools)
    }

    fn layer_open(&self) -> bool {
        self.exit_ask || self.program_open || self.overlay.is_some() || self.ai_draft.is_some() || self.name_open || self.movement_open || self.tags_open || self.fx_open
    }

    fn dismiss_layer(&mut self, cx: &mut Context<Self>) {
        if self.program_open {
            self.program_open = false;
        } else if self.overlay.is_some() {
            self.overlay = None;
        } else if self.ai_draft.is_some() {
            self.close_ai(cx);
            return;
        } else if self.name_open {
            self.name_open = false;
        } else if self.movement_open {
            self.movement_open = false;
        } else if self.tags_open {
            self.tags_open = false;
        } else if self.fx_open {
            self.fx_open = false;
        }
        cx.notify();
    }

    fn app_popup(&mut self, window: &mut Window, cx: &mut Context<Self>) -> Option<gpui_kit::AnyElement> {
        self.sync_program(window, cx);
        let body = if self.program_open {
            self.program_dialog(window, cx)
        } else if let Some(overlay) = self.overlay {
            self.overlay_page(overlay, cx)
        } else if self.ai_draft.is_some() {
            self.ai_panel(cx)
        } else if self.name_open {
            self.name_panel(cx).into_any_element()
        } else if self.movement_open {
            self.movement_panel(cx).into_any_element()
        } else if self.tags_open {
            div().flex().flex_col().gap(px(8.)).child(div().text_size(px(16.)).child("编辑曲目标签")).child(work::tag_form(self)).into_any_element()
        } else if self.fx_open {
            self.fx_panel(cx).into_any_element()
        } else {
            return None;
        };
        Some(
            div()
                .absolute()
                .top(px(0.))
                .left(px(0.))
                .right(px(0.))
                .bottom(px(0.))
                .flex()
                .items_center()
                .justify_center()
                .bg(rgb(0x161226))
                .child(
                    div()
                        .w(px(760.))
                        .max_h(px(720.))
                        .overflow_y_scroll()
                        .bg(rgb(0xffffff))
                        .text_color(rgb(0x222037))
                        .border_1()
                        .border_color(rgb(0xe5e1ed))
                        .rounded(px(14.))
                        .p(px(18.))
                        .child(body),
                )
                .into_any_element(),
        )
    }

    fn overlay_page(&mut self, overlay: Overlay, cx: &mut Context<Self>) -> gpui_kit::AnyElement {
        let title = match overlay {
            Overlay::Cd => chrome::label::CD,
            Overlay::Video => chrome::label::COVER_VIDEO,
            Overlay::Llm => chrome::label::LLM,
            Overlay::Cover => "从场照制作专辑封面",
        };
        let body = match overlay {
            Overlay::Cd => self.cd_page(cx),
            Overlay::Video => self.video_page(cx),
            Overlay::Llm => self.llm_page(cx),
            Overlay::Cover => self.cover_page(cx),
        };
        div()
            .flex()
            .flex_col()
            .gap(px(12.))
            .child(
                div()
                    .flex()
                    .flex_row()
                    .items_center()
                    .justify_between()
                    .child(title)
                    .child(Button::new("close-overlay").small().label(chrome::label::CLOSE).on_click(cx.listener(|this, _, _, cx| {
                        this.overlay = None;
                        cx.notify();
                    }))),
            )
            .child(body)
            .into_any_element()
    }

    fn place(&self) -> chrome::Place {
        match self.page {
            Page::Projects => chrome::Place::Projects,
            Page::Split => chrome::Place::Split,
            Page::Scripts => chrome::Place::Scripts,
            Page::History => chrome::Place::History,
            Page::Templates => chrome::Place::Templates,
        }
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

    fn exit_prompt(&self, cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .absolute()
            .top(px(0.))
            .left(px(0.))
            .right(px(0.))
            .bottom(px(0.))
            .flex()
            .items_center()
            .justify_center()
            .bg(rgb(0x161226))
            .child(
                exit_card()
                    .child(div().text_size(px(18.)).child("退出前保存"))
                    .child(div().text_size(px(14.)).text_color(rgb(0x66647a)).child("当前修改还没有保存. 要写入本机工作区后再退出吗?"))
                    .child(div().text_size(px(12.)).text_color(rgb(0x66647a)).child("不保存会回到上次按下保存时的内容. Esc 取消."))
                    .child(
                        div()
                            .flex()
                            .flex_row()
                            .gap(px(8.))
                            .child(Button::new("exit-save").primary().label("保存并退出").on_click(cx.listener(|this, _, window, cx| this.exit_save(window, cx))))
                            .child(Button::new("exit-discard").label("不保存退出").on_click(cx.listener(|this, _, window, cx| this.exit_discard(window, cx))))
                            .child(Button::new("exit-cancel").ghost().label("取消").on_click(cx.listener(|this, _, _, cx| this.exit_cancel(cx)))),
                    ),
            )
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
                    .children(chrome::navigation().map(|item| self.nav_button(item.id, item.mark, item.label, page_from(item.place), cx))),
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
                    .child(chrome::label::SYNC),
            )
            .into_any_element();
        let header = self.header(cx).into_any_element();
        let footer = self.footer(cx).into_any_element();
        if let Some(design) = self.cover_pending.take() {
            set_text(&self.cover_title, &design.title, window, cx);
            set_text(&self.cover_performer, &design.performer, window, cx);
            set_text(&self.cover_detail, &design.detail, window, cx);
        }
        self.ensure_ai_fields(window, cx);
        let popup = self.app_popup(window, cx);
        let page = match self.page {
            Page::Projects => self.projects_page(window, cx).into_any_element(),
            Page::Split => self.split_page(cx),
            Page::Scripts => self.scripts_page(cx),
            Page::History => self.history_page(cx),
            Page::Templates => self.templates_page(cx),
        };
        div()
            .relative()
            .flex()
            .flex_row()
            .size_full()
            .bg(cx.theme().background)
            .text_color(cx.theme().foreground)
            .capture_key_down(cx.listener(|this, event: &gpui_kit::KeyDownEvent, window, cx| this.handle_key(event, window, cx)))
            .child(nav)
            .child(
                div()
                    .flex_1()
                    .h_full()
                    .min_w(px(0.))
                    .flex()
                    .flex_col()
                    .child(header)
                    .child(
                        div()
                            .id("page-body")
                            .relative()
                            .flex_1()
                            .min_h(px(0.))
                            .child(
                                div()
                                    .id("page-scroll")
                                    .absolute()
                                    .top(px(0.))
                                    .left(px(0.))
                                    .right(px(18.))
                                    .bottom(px(0.))
                                    .overflow_y_scroll()
                                    .track_scroll(&self.page_scroll)
                                    .px(px(30.))
                                    .py(px(24.))
                                    .child(div().w_full().max_w(px(1540.)).flex().flex_col().gap(px(16.)).child(page)),
                            )
                            .child(vertical_scrollbar(self.page_scroll.clone(), self.page_scroll_grab.clone())),
                    )
                    .child(footer),
            )
            .children(popup)
            .children(self.exit_ask.then(|| self.exit_prompt(cx)))
    }
}

fn exit_card() -> gpui_kit::Div {
    div().w(px(440.)).bg(rgb(0xffffff)).border_1().border_color(rgb(0xe5e1ed)).rounded(px(14.)).p(px(22.)).flex().flex_col().gap(px(12.))
}

fn vertical_scrollbar(scroll: gpui_kit::ScrollHandle, grab: std::rc::Rc<std::cell::Cell<Option<f32>>>) -> impl IntoElement {
    use gpui_kit::base::ScrollbarHandle as _;
    use gpui_kit::{MouseButton, MouseDownEvent, MouseMoveEvent, MouseUpEvent, canvas, fill};
    let scroll_move = scroll.clone();
    let scroll_up = scroll.clone();
    let grab_move = grab.clone();
    let grab_up = grab.clone();
    div().absolute().top(px(6.)).bottom(px(6.)).right(px(4.)).w(px(12.)).bg(rgb(0xe7e1f2)).rounded(px(6.)).child(canvas(
        |_, _, _| (),
        move |bounds, _, window, _| {
            if let Some(metrics) = bar_metrics(&scroll, px_f(bounds.size.height)) {
                if metrics.range > 1. {
                    let thumb = thumb_bounds(bounds, metrics);
                    let color = if grab.get().is_some() { rgb(0x5a3da8) } else { rgb(0xb7a6de) };
                    window.paint_quad(fill(thumb_paint(bounds, thumb), color));
                }
            }
            let scroll_down = scroll.clone();
            let grab_down = grab.clone();
            window.on_mouse_event(move |event: &MouseDownEvent, phase, window, cx| {
                if !phase.capture() || event.button != MouseButton::Left || !bounds.contains(&event.position) {
                    return;
                }
                let Some(metrics) = bar_metrics(&scroll_down, px_f(bounds.size.height)) else { return };
                if metrics.range <= 1. {
                    return;
                }
                cx.stop_propagation();
                let thumb = thumb_bounds(bounds, metrics);
                let mouse = px_f(event.position.y);
                let grab_at = if bounds_contains_y(thumb, mouse) { mouse - px_f(thumb.origin.y) } else { metrics.thumb / 2. };
                grab_down.set(Some(grab_at));
                scroll_down.start_drag();
                apply_bar_drag(&scroll_down, bounds, metrics, grab_at, mouse);
                window.refresh();
            });
            let scroll_move = scroll_move.clone();
            let grab_move = grab_move.clone();
            window.on_mouse_event(move |event: &MouseMoveEvent, phase, window, cx| {
                if !phase.capture() {
                    return;
                }
                if !event.dragging() {
                    if grab_move.take().is_some() {
                        scroll_move.end_drag();
                        window.refresh();
                    }
                    return;
                }
                let Some(grab_at) = grab_move.get() else { return };
                let Some(metrics) = bar_metrics(&scroll_move, px_f(bounds.size.height)) else { return };
                if metrics.range <= 1. {
                    return;
                }
                cx.stop_propagation();
                apply_bar_drag(&scroll_move, bounds, metrics, grab_at, px_f(event.position.y));
                window.refresh();
            });
            window.on_mouse_event(move |event: &MouseUpEvent, phase, window, cx| {
                if !phase.capture() || event.button != MouseButton::Left {
                    return;
                }
                if grab_up.take().is_some() {
                    cx.stop_propagation();
                    scroll_up.end_drag();
                    window.refresh();
                }
            });
        },
    ).size_full())
}

#[derive(Clone, Copy)]
struct BarMetrics {
    range: f32,
    thumb: f32,
    offset: f32,
}

fn bar_metrics(scroll: &gpui_kit::ScrollHandle, track: f32) -> Option<BarMetrics> {
    if track <= 1. {
        return None;
    }
    // max_offset 是还能往下滚的距离, 为正. 当前 offset 往下滚时变负.
    let range = scroll_extent(px_f(scroll.max_offset().y));
    let thumb = if range <= 1. { track } else { thumb_span(track, range).min(track) };
    Some(BarMetrics { range, thumb, offset: px_f(scroll.offset().y) })
}

fn thumb_bounds(track: gpui_kit::Bounds<gpui_kit::Pixels>, metrics: BarMetrics) -> gpui_kit::Bounds<gpui_kit::Pixels> {
    let track_h = px_f(track.size.height);
    let thumb = metrics.thumb.min(track_h);
    let travel = (track_h - thumb).max(0.);
    let progress = thumb_progress(metrics.offset, metrics.range);
    gpui_kit::Bounds {
        origin: gpui_kit::point(track.origin.x, track.origin.y + px(travel * progress)),
        size: gpui_kit::size(track.size.width, px(thumb)),
    }
}

fn thumb_paint(track: gpui_kit::Bounds<gpui_kit::Pixels>, thumb: gpui_kit::Bounds<gpui_kit::Pixels>) -> gpui_kit::Bounds<gpui_kit::Pixels> {
    gpui_kit::Bounds {
        origin: gpui_kit::point(track.origin.x + px(2.), thumb.origin.y),
        size: gpui_kit::size((track.size.width - px(4.)).max(px(4.)), thumb.size.height),
    }
}

fn bounds_contains_y(bounds: gpui_kit::Bounds<gpui_kit::Pixels>, y: f32) -> bool {
    let top = px_f(bounds.origin.y);
    y >= top && y <= top + px_f(bounds.size.height)
}

fn apply_bar_drag(scroll: &gpui_kit::ScrollHandle, track: gpui_kit::Bounds<gpui_kit::Pixels>, metrics: BarMetrics, grab: f32, mouse_y: f32) {
    let offset = offset_for_thumb_drag(px_f(track.size.height), metrics.range, metrics.thumb, grab, mouse_y, px_f(track.origin.y));
    let current = scroll.offset();
    scroll.set_offset(gpui_kit::point(current.x, px(clamp_scroll(offset, -metrics.range))));
}

#[cfg(test)]
mod scrollbar_tests {
    use super::{clamp_scroll, offset_for_thumb_drag, run_dialog, scroll_extent, thumb_progress, thumb_span};

    #[test]
    fn a_short_page_does_not_produce_a_non_finite_thumb() {
        assert_eq!(thumb_progress(0., 0.), 0.);
        assert!(thumb_progress(0., 0.).is_finite());
        assert_eq!(thumb_progress(-200., 400.), 0.5);
        assert_eq!(thumb_progress(-900., 400.), 1.);
    }

    #[test]
    fn positive_max_offset_still_leaves_a_thumb_range() {
        assert_eq!(scroll_extent(400.), 400.);
        assert_eq!(scroll_extent(-400.), 400.);
        assert_eq!(scroll_extent(0.), 0.);
    }

    #[test]
    fn dragging_past_the_bar_reaches_the_ends() {
        let view = 200.;
        let range = 400.;
        let thumb = thumb_span(view, range);
        assert!((thumb - (view * view / (view + range))).abs() < 0.01);
        assert!(offset_for_thumb_drag(view, range, thumb, 0., 0., 0.).abs() < 0.01);
        assert!((offset_for_thumb_drag(view, range, thumb, 12., 4000., 0.) + range).abs() < 0.01);
        assert!(offset_for_thumb_drag(view, range, thumb, 12., -800., 40.).abs() < 0.01);
        assert_eq!(clamp_scroll(-20., -400.), -20.);
        assert_eq!(clamp_scroll(-900., -400.), -400.);
        assert_eq!(clamp_scroll(30., -400.), 0.);
    }

    #[test]
    fn a_short_track_still_has_a_finite_thumb() {
        let thumb = thumb_span(12., 400.);
        assert!(thumb.is_finite());
        assert!(thumb <= 12.);
        assert!(thumb >= 1.);
        assert!(offset_for_thumb_drag(f32::NAN, 10., 4., 0., 0., 0.).is_finite());
    }

    #[test]
    fn a_file_dialog_returns_on_its_own_thread() {
        assert_eq!(run_dialog(|| Some(7u8)), Some(7));
        assert_eq!(run_dialog(|| None::<u8>), None);
    }
}

fn px_f(value: gpui_kit::Pixels) -> f32 {
    value.as_f32()
}

fn scroll_extent(max_offset_y: f32) -> f32 {
    let extent = max_offset_y.abs();
    if extent.is_finite() { extent } else { 0. }
}

fn thumb_progress(offset: f32, range: f32) -> f32 {
    if !offset.is_finite() || !range.is_finite() || range <= 1. {
        0.
    } else {
        (-offset / range).clamp(0., 1.)
    }
}

fn thumb_span(view: f32, range: f32) -> f32 {
    if !view.is_finite() || !range.is_finite() || view <= 1. {
        return 1.;
    }
    let span = view * view / (view + range);
    let floor = 28_f32.min(view);
    if !span.is_finite() { floor } else { span.clamp(floor, view) }
}

fn offset_for_thumb_drag(view: f32, range: f32, thumb: f32, grab: f32, mouse_y: f32, track_y: f32) -> f32 {
    if ![view, range, thumb, grab, mouse_y, track_y].iter().all(|value| value.is_finite()) {
        return 0.;
    }
    let travel = (view - thumb).max(1.);
    let ratio = ((mouse_y - grab - track_y) / travel).clamp(0., 1.);
    let offset = -range * ratio;
    if offset.is_finite() { offset } else { 0. }
}

fn clamp_scroll(offset: f32, limit: f32) -> f32 {
    let low = limit.min(0.);
    let high = limit.max(0.);
    offset.clamp(low, high)
}

pub(super) fn defer_dialog<T: Send + 'static>(window: &mut Window, cx: &mut Context<Studio>, dialog: impl FnOnce() -> Option<T> + Send + 'static, apply: impl FnOnce(&mut Studio, Option<T>, &mut Window, &mut Context<Studio>) + Send + 'static) {
    use gpui_kit::VisualContext as _;
    let view = cx.entity();
    window
        .spawn(cx, async move |cx| {
            let value = cx.background_spawn(async move { run_dialog(dialog) }).await;
            let _ = cx.update_window_entity(&view, |studio, window, cx| apply(studio, value, window, cx));
        })
        .detach();
}

fn run_dialog<T: Send + 'static>(dialog: impl FnOnce() -> Option<T> + Send + 'static) -> Option<T> {
    let (tx, rx) = std::sync::mpsc::channel();
    std::thread::spawn(move || {
        let _ = tx.send(dialog());
    });
    rx.recv().unwrap_or(None)
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
        theme.button = rgb(0xffffff).into();
        theme.button_hover = rgb(0xf0ebfa).into();
        theme.button_active = rgb(0xe4dcf3).into();
        theme.button_foreground = rgb(0x222037).into();
        theme.button_primary = rgb(0x5a3da8).into();
        theme.button_primary_hover = rgb(0x4b2e93).into();
        theme.button_primary_active = rgb(0x3d2778).into();
        theme.button_primary_foreground = rgb(0xffffff).into();
        theme.secondary_foreground = rgb(0x59536d).into();
        theme.danger = rgb(0xa33049).into();
        theme.input = rgb(0xdcd7e6).into();
        theme.ring = rgb(0x7851d7).into();
        theme.shadow = false;
    });
}

pub(super) fn surface() -> Div {
    div().bg(rgb(0xffffff)).border_1().border_color(rgb(0xe5e1ed)).rounded(px(14.)).p(px(19.)).flex().flex_col().gap_2()
}


fn info_card(title: &str) -> Div {
    div().bg(rgb(0xffffff)).border_1().border_color(rgb(0xe5e1ed)).rounded(px(14.)).p(px(19.)).flex().flex_col().gap_2().child(div().text_size(px(16.)).child(title.to_string()))
}

fn project_head() -> Div {
    let columns = chrome::project_columns();
    div()
        .px(px(19.))
        .py(px(8.))
        .bg(rgb(0xf6f4fa))
        .text_size(px(11.))
        .text_color(rgb(0x66647a))
        .flex()
        .flex_row()
        .items_center()
        .gap(px(8.))
        .child(div().flex_1().child(columns[0]))
        .child(div().w(px(110.)).child(columns[1]))
        .child(div().w(px(150.)).child(columns[2]))
        .child(div().w(px(90.)).child(columns[3]))
        .child(div().w(px(48.)).child(columns[4]))
        .child(div().w(px(88.)).child(columns[5]))
        .child(div().min_w(px(120.)).child(columns[6]))
}

pub(super) fn history_head() -> Div {
    let columns = chrome::history_columns();
    div()
        .flex()
        .flex_row()
        .items_center()
        .gap(px(8.))
        .px(px(12.))
        .py(px(8.))
        .bg(rgb(0xf6f4fa))
        .text_size(px(11.))
        .text_color(rgb(0x66647a))
        .child(div().w(px(140.)).child(columns[0]))
        .child(div().flex_1().child(columns[1]))
        .child(div().w(px(110.)).child(columns[2]))
        .child(div().w(px(80.)).child(columns[3]))
        .child(div().flex_1().child(columns[4]))
}

pub(super) fn history_line(time: &str, project: &str, operation: &str, result: &str, message: &str) -> Div {
    div()
        .flex()
        .flex_row()
        .items_center()
        .gap(px(8.))
        .px(px(12.))
        .py(px(8.))
        .border_t_1()
        .border_color(rgb(0xede9f3))
        .child(div().w(px(140.)).child(time.to_string()))
        .child(div().flex_1().min_w(px(0.)).child(project.to_string()))
        .child(div().w(px(110.)).child(operation.to_string()))
        .child(div().w(px(80.)).child(result.to_string()))
        .child(div().flex_1().min_w(px(0.)).child(message.to_string()))
}

fn listing_extra(project: &Project) -> String {
    let mut parts = Vec::new();
    for source in &project.sources {
        parts.push(crate::names::source_leaf(source));
    }
    for track in &project.tracks {
        if !track.tags.title.trim().is_empty() {
            parts.push(track.tags.title.clone());
        }
        if !track.tags.composer.trim().is_empty() {
            parts.push(track.tags.composer.clone());
        }
        if !track.filename.trim().is_empty() {
            parts.push(track.filename.clone());
        }
    }
    parts.join("\n")
}

fn oldest_key(date: &str) -> String {
    if date.trim().is_empty() { "9999".into() } else { date.to_string() }
}

fn push_unique(out: &mut Vec<String>, value: &str) {
    let value = value.trim();
    if value.is_empty() || out.iter().any(|item| item == value) {
        return;
    }
    out.push(value.to_string());
}

fn stat_card(value: usize, group: &'static str, label: &'static str, mark: &'static str, selected: bool, cx: &mut Context<Studio>) -> impl IntoElement {
    div()
        .id(SharedString::from(format!("group-{group}")))
        .flex_1()
        .flex()
        .flex_row()
        .items_center()
        .gap(px(12.))
        .p(px(14.))
        .bg(if selected { rgb(0xf4effb) } else { rgb(0xffffff) })
        .border_1()
        .border_color(if selected { rgb(0x5a3da8) } else { rgb(0xe5e1ed) })
        .rounded(px(14.))
        .on_click(cx.listener(move |this, _, _, cx| {
            this.project_group = group;
            cx.notify();
        }))
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

fn area_text(input: &Entity<TextareaState>, cx: &App) -> String {
    input.read(cx).value().to_string()
}

fn set_area(input: &Entity<TextareaState>, value: &str, window: &mut Window, cx: &mut App) {
    if input.read(cx).value().to_string() != value {
        input.update(cx, |state, cx| state.set_value(value, window, cx));
    }
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
    let skipped = reports.iter().filter(|item| item.skipped).count();
    let failed = reports.len() - ok;
    let mut lines = vec![format!("完成 {ok} 轨, 跳过 {skipped} 轨, 失败 {failed} 轨.")];
    for item in reports.iter().filter(|item| item.error.is_some()).take(8) {
        lines.push(format!("{}: {}", item.target, item.error.as_deref().unwrap_or("")));
    }
    lines.join("\n")
}

