//! 分轨, CD, 封面视频, 记录, 模板和模型页.

use gpui_kit::component::{
    Disableable as _, Selectable as _, Sizable as _,
    button::{Button, ButtonVariants as _},
    checkbox::Checkbox,
    form::{Field, Form},
    input::Input,
    label::Label,
};
use gpui_kit::{
    AnyElement, AppContext as _, Context, Entity, FontWeight, InteractiveElement as _, IntoElement, ParentElement as _,
    SharedString, StatefulInteractiveElement as _, Styled as _, Window, div, px, rgb,
};
use gpui_kit::component::input::InputState;

use crate::chrome;
use crate::catalog;
use crate::cue::{self, CueTrack};
use crate::effects;
use crate::export::{self, TrackReport};
use crate::ffmpeg;
use crate::llm::{self, LlmConfig};
use crate::mb;
use crate::model::{self, Project, Track, TAG_KEYS};
use crate::names;
use crate::program;
use crate::video::{self, VideoMode};

use super::{set_text, text, JobMsg, PlayedClip, Studio};

enum ClockTick {
    Continue,
    Stop,
    Advance(usize),
}

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
        self.ensure_preview_duration(cx);
        let mut page = div().flex().flex_col().gap(px(16.)).child(self.split_toolbar(cx));
        let Some(project) = self.project().cloned() else {
            return page.child(super::surface().child(Label::new(chrome::label::SCRIPT_EMPTY))).into_any_element();
        };
        page = page
            .child(
                super::surface().child(section_head(
                    chrome::section::IMPORT,
                    div().px(px(8.)).py(px(4.)).rounded(px(999.)).bg(rgb(0xeeeef2)).text_size(px(11.)).font_weight(FontWeight::SEMIBOLD).text_color(rgb(0x5c586e)).child("本机处理"),
                )).child(self.source_block(&project, cx)),
            )
            .child(
                super::surface()
                    .child(section_head(
                        chrome::section::SESSION,
                        div()
                            .flex()
                            .flex_row()
                            .gap(px(8.))
                            .child(primary_btn(Button::new("read-session").label("识别场次信息").on_click(cx.listener(|this, _, window, cx| this.import_program(true, window, cx)))))
                            .child(plain_btn(Button::new("import-program").label("导入节目单").on_click(cx.listener(|this, _, window, cx| this.import_program(false, window, cx)))))
                            .child(plain_btn(Button::new("english-session").label(chrome::label::ENGLISH).disabled(self.busy).on_click(cx.listener(|this, _, _, cx| this.request_english(cx))))),
                    ))
                    .child(session_form(self))
                    .child(self.model_strip(cx)),
            )
            .child(super::surface().child(section_head(chrome::section::COVER, div())).child(self.cover_block(cx)))
            .child(
                div()
                    .bg(rgb(0xffffff))
                    .border_1()
                    .border_color(rgb(0xe5e1ed))
                    .rounded(px(14.))
                    .flex()
                    .flex_col()
                    .overflow_hidden()
                    .child(
                        div().px(px(19.)).pt(px(19.)).child(section_head(
                            chrome::section::TRACKS,
                            div()
                                .flex()
                                .flex_row()
                                .gap(px(8.))
                                .child(plain_btn(Button::new("match-all-names").label("一键匹配全部文件名").on_click(cx.listener(|this, _, window, cx| this.match_filenames(false, window, cx)))))
                                .child(plain_btn(Button::new("fill-tags").label("按场次更新全部标签").on_click(cx.listener(|this, _, window, cx| this.fill_tags(window, cx)))))
                                .child(primary_btn(Button::new("add-track").label(chrome::label::ADD_TRACK).on_click(cx.listener(|this, _, window, cx| this.add_track(window, cx)))))
                                .child(plain_btn(Button::new("add-at-playhead").label("＋ 从播放位置添加").on_click(cx.listener(|this, _, window, cx| this.add_at_playhead(window, cx))))),
                        )),
                    )
                    .child(self.track_block(&project, cx)),
            )
            .child(
                super::surface()
                    .child(section_head(chrome::section::OUTPUT, div()))
                    .child(self.settings_block(&project, cx))
                    .child(self.effects_block(&project, cx))
                    .child(output_form(self))
                    .child(self.actions(cx)),
            )
            .child(
                super::surface().child(section_head(
                    chrome::section::RESULT,
                    primary_btn(Button::new("open-video").label(chrome::label::COVER_VIDEO).on_click(cx.listener(|this, _, _, cx| {
                        this.overlay = Some(super::Overlay::Video);
                        cx.notify();
                    }))),
                )).child(div().text_size(px(12.)).text_color(rgb(0x66647a)).child("生成的曲目将在这里显示。")),
            );
        if !self.preflight.is_empty() {
            page = page.child(super::surface().child(Label::new("导出预检")).child(preflight_lines(&self.preflight)));
        }
        if !self.mb_hits.is_empty() {
            page = page.child(super::surface().child(self.mb_block(cx)));
        }
        page.into_any_element()
    }

    fn split_toolbar(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let (title, detail) = chrome::heading(chrome::Place::Split);
        div()
            .flex()
            .flex_row()
            .items_center()
            .justify_between()
            .gap_2()
            .child(
                div()
                    .flex()
                    .flex_col()
                    .child(div().text_size(px(32.)).font_weight(FontWeight::BOLD).line_height(px(38.)).child(title))
                    .child(div().mt(px(10.)).text_color(rgb(0x5c5470)).line_height(px(22.)).child(detail)),
            )
            .child(
                div()
                    .flex()
                    .flex_row()
                    .gap(px(8.))
                    .child(primary_btn(Button::new("open-cd").label(chrome::label::CD).on_click(cx.listener(|this, _, _, cx| {
                        this.overlay = Some(super::Overlay::Cd);
                        cx.notify();
                    }))))
                    .child(plain_btn(Button::new("new-session").label(chrome::label::NEW_SESSION).on_click(cx.listener(|this, _, window, cx| this.create_project(window, cx))))),
            )
    }

    fn transport(&self, cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .flex()
            .flex_row()
            .gap_2()
            .child(plain_btn(Button::new("back-five").label(chrome::label::BACK_FIVE).disabled(self.busy).on_click(cx.listener(|this, _, _, cx| this.seek_preview(-chrome::SEEK_SECONDS, cx)))))
            .child(plain_btn(Button::new("forward-five").label(chrome::label::FORWARD_FIVE).disabled(self.busy).on_click(cx.listener(|this, _, _, cx| this.seek_preview(chrome::SEEK_SECONDS, cx)))))
            .child(quiet_btn(Button::new("preview-raw").label(chrome::label::PREVIEW_FROM).disabled(self.busy).on_click(cx.listener(|this, _, _, cx| this.preview_window(cx)))))
            .child(quiet_btn(Button::new("preview-native").label(chrome::label::RETURN_PREVIEW).disabled(self.busy).on_click(cx.listener(|this, _, _, cx| this.return_preview(cx)))))
    }

    fn cover_block(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let project = self.project();
        let cover = project.map(|item| item.cover_file.clone()).unwrap_or_default();
        let back = project.map(|item| item.back_cover_file.clone()).unwrap_or_default();
        let cover_name = if cover.trim().is_empty() { "未选择任何文件".to_string() } else { cover };
        let back_name = if back.trim().is_empty() { "未选择任何文件".to_string() } else { back };
        div()
            .flex()
            .flex_col()
            .gap(px(12.))
            .child(
                div()
                    .flex()
                    .flex_row()
                    .items_center()
                    .gap(px(8.))
                    .child(primary_btn(Button::new("pick-cover").label(chrome::label::UPLOAD_COVER).on_click(cx.listener(|this, _, window, cx| this.pick_cover(false, window, cx)))))
                    .child(
                        div()
                            .flex()
                            .flex_col()
                            .gap(px(5.))
                            .child(div().text_size(px(12.)).font_weight(FontWeight::SEMIBOLD).child("或上传现成封面 · PNG / JPG"))
                            .child(file_slot("pick-cover-file", cover_name, cx.listener(|this, _, window, cx| this.pick_cover(false, window, cx)))),
                    )
                    .child(quiet_btn(Button::new("clear-cover").label("移除封面").on_click(cx.listener(|this, _, _, cx| this.clear_cover(false, cx))))),
            )
            .child(
                div()
                    .flex()
                    .flex_row()
                    .items_center()
                    .gap(px(8.))
                    .child(
                        div()
                            .flex()
                            .flex_col()
                            .gap(px(5.))
                            .child(div().text_size(px(12.)).font_weight(FontWeight::SEMIBOLD).child("封底（可选） · PNG / JPG"))
                            .child(file_slot("pick-back-file", back_name, cx.listener(|this, _, window, cx| this.pick_cover(true, window, cx)))),
                    )
                    .child(quiet_btn(Button::new("clear-back").label("移除封底").on_click(cx.listener(|this, _, _, cx| this.clear_cover(true, cx))))),
            )
            .child(div().text_size(px(12.)).line_height(px(19.)).text_color(rgb(0x66647a)).child("可留空。"))
    }

    fn settings_block(&self, project: &Project, cx: &mut Context<Self>) -> impl IntoElement {
        let settings = &project.settings;
        let rate = settings.rate.to_string();
        div()
            .flex()
            .flex_col()
            .gap(px(12.))
            .child(quad(
                select_line(self, "format-cycle", "输出格式", &settings.format, &[("FLAC", "FLAC"), ("WAV", "WAV"), ("MP3", "MP3"), ("MKV", "MKV")], cx, set_format),
                select_line(self, "rate-cycle", "采样率", &rate, &[("44100", "44.1 kHz"), ("48000", "48 kHz"), ("96000", "96 kHz")], cx, set_rate),
                labeled("增益（dB）", &self.gain),
                labeled("声场宽度（%）", &self.width),
            ))
            .child({
                let mut row = div().flex().flex_row().items_center().gap(px(8.)).child(
                    Checkbox::new("limiter").label("启用防爆音限幅").checked(self.limiter_on).on_change(cx.listener(|this, value, _, cx| {
                        this.limiter_on = *value;
                        this.pull(cx);
                        this.mark_dirty(cx);
                    })),
                );
                if self.limiter_on {
                    row = row.child(flat_input(&self.limiter).w(px(88.)));
                }
                row
            })
            .child(quad(labeled("混响（%）", &self.reverb), labeled("英文命名规则", &self.naming), div(), div()))
            .child(
                div()
                    .flex()
                    .flex_row()
                    .gap(px(12.))
                    .child(check("embed-cover", "嵌入封面", settings.cover, cx, |project, value| project.settings.cover = value))
                    .child(check("split-tracks", "按时间点分轨", settings.split, cx, |project, value| project.settings.split = value))
                    .child(check("write-tags", "写入标签", settings.metadata, cx, |project, value| project.settings.metadata = value))
                    .child(check("embed-back", chrome::label::EMBED_BACK, settings.embed_back, cx, |project, value| project.settings.embed_back = value))
                    .child(check("disc-folders", chrome::label::DISC_FOLDERS, project.disc_folders, cx, |project, value| project.disc_folders = value)),
            )
            .child(div().text_size(px(12.)).text_color(rgb(0x66647a)).child("默认 48 kHz。WAV 选了封面时转为 FLAC。WAV 只支持部分标签。宽度调整适用于双声道。大幅提高增益时可勾选「启用防爆音限幅」，默认峰值上限为 −1 dBFS。"))
    }

    fn effects_block(&self, project: &Project, cx: &mut Context<Self>) -> impl IntoElement {
        let effects = &project.settings.effects;
        let mut block = div().flex().flex_col().gap(px(12.)).child(
            div()
                .id("toggle-effects")
                .cursor_pointer()
                .text_size(px(14.))
                .font_weight(FontWeight::SEMIBOLD)
                .child(if self.effects_open { "▼ 高级后期处理" } else { "▶ 高级后期处理" })
                .on_click(cx.listener(|this, _, _, cx| {
                    this.effects_open = !this.effects_open;
                    cx.notify();
                })),
        );
        if !self.effects_open {
            return block;
        }
        block = block
            .child(div().text_size(px(14.)).font_weight(FontWeight::SEMIBOLD).child("降噪与修复"))
            .child(quad(
                select_line(self, "denoise-cycle", "降噪方式", &effects.denoise, &[("off", "关闭"), ("fft", "FFT"), ("nlm", "非局部均值")], cx, set_denoise),
                labeled("FFT 降噪量（dB）", &self.noise_reduction),
                select_line(self, "hum-cycle", "电源噪声", &effects.hum, &[("off", "关闭"), ("50", "50 Hz"), ("60", "60 Hz")], cx, set_hum),
                div().flex().flex_row().gap(px(8.)).items_center().child(check("declick", "去点击", effects.declick, cx, |project, value| project.settings.effects.declick = value)).child(check("declip", "修复削波", effects.declip, cx, |project, value| project.settings.effects.declip = value)),
            ))
            .child(div().text_size(px(14.)).font_weight(FontWeight::SEMIBOLD).child("频率与声道"))
            .child(quad(
                labeled("高通（Hz，0 关闭）", &self.highpass),
                labeled("低通（Hz，0 关闭）", &self.lowpass),
                labeled("低频 100 Hz（dB）", &self.bass),
                labeled("中频 1 kHz（dB）", &self.mid),
            ))
            .child(quad(labeled("高频 6 kHz（dB）", &self.treble), select_line(self, "channels-cycle", "声道", &effects.channels, &[("source", "保持来源"), ("mono", "单声道"), ("swap", "交换左右"), ("invert", "反转右声道")], cx, set_channels), div(), div()))
            .child(div().text_size(px(14.)).font_weight(FontWeight::SEMIBOLD).child("动态与淡化"))
            .child(quad(
                labeled("噪声门（dBFS，0 关闭）", &self.gate),
                labeled("压缩比（1 关闭）", &self.compress),
                labeled("压缩阈值（dBFS）", &self.threshold),
                div().flex().flex_row().items_center().child(check("deess", "减弱齿音", effects.deess, cx, |project, value| project.settings.effects.deess = value)),
            ))
            .child(quad(
                select_line(self, "normalize-cycle", "响度处理", &effects.normalize, &[("off", "关闭"), ("loud", "EBU R128"), ("dynamic", "动态")], cx, set_normalize),
                labeled("目标响度（LUFS）", &self.lufs),
                labeled("淡入（秒）", &self.fade_in),
                labeled("淡出（秒）", &self.fade_out),
            ))
            .child(div().text_size(px(12.)).text_color(rgb(0x66647a)).child("默认关闭。FFT 适用于持续底噪，非局部均值处理较慢。响度和动态处理建议试听后再批量导出。"));
        block
    }

    fn source_block(&self, project: &Project, cx: &mut Context<Self>) -> impl IntoElement {
        let mut list = div().flex().flex_col().gap_1();
        for (index, source) in project.sources.iter().enumerate() {
            list = list.child(format!("{}. {source}", index + 1));
        }
        let file_total = self.preview_duration.unwrap_or(0.0);
        let clock = format!("{} / {}", names::format_seconds(self.playhead), names::format_seconds(file_total));
        let (bar_at, bar_total) = if self.stitch_on { (self.stitch_at, self.stitch_total) } else { (self.playhead, file_total) };
        let short = if self.stitch_on {
            format!("拼接 {} / {}", short_clock(bar_at), short_clock(bar_total))
        } else {
            format!("{} / {}", short_clock(bar_at), short_clock(bar_total))
        };
        let info = if self.stitch_on {
            "正在按拼接后的时间轴试听. 下面的时钟仍是录音文件里的位置, 可以取播放点."
        } else if project.sources.is_empty() {
            "请选择原始音频文件。"
        } else {
            "使用所选文件试听整场录音。"
        };
        div()
            .flex()
            .flex_col()
            .gap(px(10.))
            .child(
                div()
                    .flex()
                    .flex_row()
                    .gap(px(8.))
                    .child(primary_btn(Button::new("add-audio").label(chrome::label::IMPORT_AUDIO).on_click(cx.listener(|this, _, window, cx| this.add_audio(window, cx)))))
                    .child(quiet_btn(Button::new("llm-audio").label(chrome::label::RECOGNIZE_AUDIO).on_click(cx.listener(|this, _, _, cx| {
                        this.overlay = Some(super::Overlay::Llm);
                        cx.notify();
                    })))),
            )
            .child(div().text_size(px(12.)).line_height(px(19.)).text_color(rgb(0x66647a)).child("支持 WAV、FLAC、APE、MP3、MKV 多选。"))
            .child(list)
            .child(
                div()
                    .flex()
                    .flex_row()
                    .gap(px(8.))
                    .child(primary_btn(Button::new("concat-sources").label(chrome::label::CONCAT).on_click(cx.listener(|this, _, window, cx| this.concat_sources(window, cx)))))
                    .child(quiet_btn(Button::new("relink-audio").label("重新关联当前曲目").on_click(cx.listener(|this, _, window, cx| this.relink(window, cx)))))
                    .child(quiet_btn(Button::new("tracks-from-sources").label("按录音各加一轨").on_click(cx.listener(|this, _, window, cx| this.tracks_from_sources(window, cx)))))
                    .child(quiet_btn(Button::new("relink-tree").label("按文件夹重新关联").on_click(cx.listener(|this, _, window, cx| this.relink_tree(window, cx)))))
                    .child(quiet_btn(Button::new("source-root").label("设置素材根目录").on_click(cx.listener(|this, _, window, cx| this.choose_root(window, cx))))),
            )
            .child(
                div()
                    .mt(px(13.))
                    .flex()
                    .flex_col()
                    .gap(px(5.))
                    .child(div().text_size(px(12.)).font_weight(FontWeight::SEMIBOLD).child("当前试听文件"))
                    .child(self.source_picker(project, "preview", cx)),
            )
            .child(div().mt(px(9.)).text_size(px(12.)).line_height(px(19.)).text_color(rgb(0x66647a)).child(info))
            .child(
                div()
                    .mt(px(9.))
                    .h(px(40.))
                    .w_full()
                    .max_w(px(700.))
                    .rounded(px(20.))
                    .px(px(12.))
                    .bg(rgb(0xf1f3f4))
                    .flex()
                    .flex_row()
                    .items_center()
                    .gap(px(10.))
                    .child(
                        div()
                            .id("play-toggle")
                            .cursor_pointer()
                            .text_size(px(13.))
                            .child(if self.playing { "❚❚" } else { "▶" })
                            .on_click(cx.listener(|this, _, _, cx| this.toggle_preview(cx))),
                    )
                    .child(div().text_size(px(12.)).child(short))
                    .child(self.seek_bar(cx))
                    .child(div().text_size(px(12.)).text_color(rgb(0x66647a)).child("🔊"))
                    .child(div().text_size(px(14.)).text_color(rgb(0x66647a)).child("⋮")),
            )
            .child(
                div()
                    .mt(px(13.))
                    .flex()
                    .flex_row()
                    .items_center()
                    .gap(px(8.))
                    .child(div().font_family("Consolas").text_color(rgb(0x66647a)).child(clock))
                    .child(self.transport(cx)),
            )
            .child(div().text_size(px(12.)).text_color(rgb(0x66647a)).child("使用本机 FFmpeg 和 FFprobe，将分轨文件写入专辑子文件夹。"))
    }

    fn track_block(&mut self, project: &Project, cx: &mut Context<Self>) -> impl IntoElement {
        let selected = self.track_index;
        let columns = chrome::split_track_columns();
        let mut list = div().flex().flex_col().child(
            div()
                .flex()
                .flex_row()
                .items_center()
                .gap(px(8.))
                .px(px(13.))
                .py(px(11.))
                .bg(rgb(0xf6f4fa))
                .text_size(px(11.))
                .font_weight(FontWeight::SEMIBOLD)
                .text_color(rgb(0x615b73))
                .child(div().w(px(22.)).child(""))
                .child(div().w(px(36.)).child(columns[0]))
                .child(div().flex_1().child(columns[1]))
                .child(div().w(px(170.)).child(columns[2]))
                .child(div().w(px(168.)).child(columns[3]))
                .child(div().w(px(168.)).child(columns[4]))
                .child(div().w(px(120.)).child(columns[5])),
        );
        for (index, track) in project.tracks.iter().enumerate() {
            let id = track.id.clone();
            let picked = self.track_picks.contains(&index);
            let order = format!("{:02}", index + 1);
            let check = Checkbox::new(SharedString::from(format!("pick-{id}"))).checked(picked).on_change(cx.listener(move |this, value: &bool, _, cx| {
                this.pull(cx);
                if *value {
                    this.track_picks.insert(index);
                } else {
                    this.track_picks.remove(&index);
                }
                cx.notify();
            }));
            let actions = div()
                .w(px(120.))
                .flex()
                .flex_col()
                .child(text_btn(Button::new(SharedString::from(format!("hear-{id}"))).label("试听").disabled(self.busy).on_click(cx.listener(move |this, _, window, cx| {
                    this.select_track(index, window, cx);
                    this.preview(false, cx);
                }))))
                .child(text_btn(Button::new(SharedString::from(format!("match-{id}"))).label("匹配文件名").on_click(cx.listener(move |this, _, window, cx| {
                    this.select_track(index, window, cx);
                    this.match_filenames(true, window, cx);
                }))))
                .child(text_btn(Button::new(SharedString::from(format!("model-name-{id}"))).label("用模型匹配").disabled(self.busy).on_click(cx.listener(move |this, _, window, cx| {
                    this.select_track(index, window, cx);
                    this.model_filename(cx);
                }))))
                .child(text_btn(Button::new(SharedString::from(format!("expand-{id}"))).label("展开乐章／分曲").on_click(cx.listener(move |this, _, window, cx| {
                    this.select_track(index, window, cx);
                    this.movement_open = true;
                    cx.notify();
                }))))
                .child(text_btn(Button::new(SharedString::from(format!("tags-{id}"))).label("编辑标签").on_click(cx.listener(move |this, _, window, cx| {
                    this.select_track(index, window, cx);
                    this.tags_open = !this.tags_open;
                    cx.notify();
                }))));
            if selected == Some(index) {
                let prefix = format!("row{index}");
                let stitched = track.parts.len() >= 2;
                let mut title_col = div()
                    .flex_1()
                    .min_w(px(0.))
                    .flex()
                    .flex_col()
                    .gap(px(5.))
                    .child(flat_input(&self.tags[0]))
                    .child(div().text_size(px(12.)).text_color(rgb(0x66647a)).child("输出文件名（不含扩展名）"))
                    .child(flat_input(&self.filename));
                if stitched {
                    title_col = title_col.child(div().text_size(px(12.)).text_color(rgb(0x66647a)).child("导出和试听按这些段落拼接, 空隙不保留."));
                    for line in names::describe_stitch(&track.parts) {
                        title_col = title_col.child(div().text_size(px(12.)).child(line));
                    }
                }
                let start_cell = if stitched {
                    div().w(px(168.)).text_size(px(12.)).text_color(rgb(0x66647a)).child("按左侧段落")
                } else {
                    div().w(px(168.)).flex().flex_row().items_center().gap(px(4.)).child(div().flex_1().min_w(px(0.)).child(flat_input(&self.start))).child(text_btn(Button::new(SharedString::from(format!("take-start-{id}"))).label("取播放点").on_click(cx.listener(|this, _, window, cx| this.stamp_time(false, window, cx)))))
                };
                let end_cell = if stitched {
                    div().w(px(168.)).text_size(px(12.)).text_color(rgb(0x66647a)).child("取播放点见下方时钟")
                } else {
                    div().w(px(168.)).flex().flex_row().items_center().gap(px(4.)).child(div().flex_1().min_w(px(0.)).child(flat_input(&self.end))).child(text_btn(Button::new(SharedString::from(format!("take-end-{id}"))).label("取播放点").on_click(cx.listener(|this, _, window, cx| this.stamp_time(true, window, cx)))))
                };
                list = list.child(
                    div()
                        .flex()
                        .flex_row()
                        .items_center()
                        .gap_2()
                        .px(px(8.))
                        .py(px(8.))
                        .border_t_1()
                        .border_color(rgb(0xede9f3))
                        .child(check)
                        .child(div().w(px(36.)).text_size(px(12.)).child(order))
                        .child(title_col)
                        .child(div().w(px(170.)).child(self.source_picker(project, &prefix, cx)))
                        .child(start_cell)
                        .child(end_cell)
                        .child(actions),
                );
            } else {
                let title = if track.parts.len() >= 2 {
                    let name = if track.tags.title.trim().is_empty() { "未命名曲目".to_string() } else { track.tags.title.clone() };
                    format!("{name}  ({} 段拼接)", track.parts.len())
                } else if track.tags.title.trim().is_empty() {
                    "未命名曲目".to_string()
                } else {
                    track.tags.title.clone()
                };
                let leaf = if track.source.is_empty() { "请选择音频文件".to_string() } else { names::source_leaf(&track.source) };
                let end = if track.parts.len() >= 2 {
                    "按段落".to_string()
                } else if track.end.trim().is_empty() {
                    "自动".to_string()
                } else {
                    track.end.clone()
                };
                list = list.child(
                    div()
                        .flex()
                        .flex_row()
                        .items_center()
                        .gap_2()
                        .px(px(8.))
                        .py(px(8.))
                        .border_t_1()
                        .border_color(rgb(0xede9f3))
                        .child(check)
                        .child(div().w(px(36.)).child(order))
                        .child(
                            div().flex_1().min_w(px(0.)).child(
                                text_btn(Button::new(SharedString::from(format!("track-{id}"))).label(title).on_click(cx.listener(move |this, _, window, cx| this.select_track(index, window, cx)))),
                            ),
                        )
                        .child(div().w(px(170.)).text_size(px(12.)).child(leaf))
                        .child(div().w(px(168.)).text_size(px(12.)).child(if track.parts.len() >= 2 { "按段落".to_string() } else { track.start.clone() }))
                        .child(div().w(px(168.)).text_size(px(12.)).child(end))
                        .child(actions),
                );
            }
        }
        let mut body = div()
            .flex()
            .flex_col()
            .child(div().px(px(19.)).pt(px(8.)).pb(px(8.)).flex().flex_col().gap(px(6.)).child(check("auto-join", chrome::label::JOIN, project.auto_join, cx, |project, value| project.auto_join = value)).child(div().text_size(px(12.)).line_height(px(19.)).text_color(rgb(0x66647a)).child("每轨可选择不同的音频文件，起止点以所选文件为准。可输入 18:22 或 01:18:22.500。点击「取播放点」填写当前时间，点击「试听」从本轨起点播放。")))
            .child(list);
        if self.tags_open && selected.is_some() {
            body = body.child(div().px(px(19.)).py(px(8.)).child(tag_form(self)));
        }
        body = body.child(
            div()
                .px(px(19.))
                .py(px(12.))
                .flex()
                .flex_row()
                .gap(px(8.))
                .child(quiet_btn(Button::new("remove-track").label("删除当前曲目").on_click(cx.listener(|this, _, window, cx| this.remove_track(window, cx)))))
                .child(quiet_btn(Button::new("apply-names").label("按规则填写空文件名").on_click(cx.listener(|this, _, window, cx| this.apply_names(window, cx)))))
                .child(quiet_btn(Button::new("local-work").label("采用本地英文曲名").on_click(cx.listener(|this, _, window, cx| this.apply_catalog(window, cx)))))
                .child(plain_btn(Button::new("merge-tracks").label(chrome::label::MERGE).disabled(self.track_picks.len() < 2).on_click(cx.listener(|this, _, window, cx| {
                    this.merge_selected(window, cx);
                }))))
                .child(plain_btn(Button::new("split-merged").label(chrome::label::UNMERGE).on_click(cx.listener(|this, _, window, cx| this.split_merged(window, cx)))))
                .child(plain_btn(Button::new("merge-groups").label(chrome::label::MERGE_GROUPS).on_click(cx.listener(|this, _, window, cx| this.merge_groups(window, cx)))))
                .child(plain_btn(Button::new("cycle-status").label(format!("状态: {}", project.status)).on_click(cx.listener(|this, _, _, cx| this.cycle_status(cx))))),
        );
        if self.movement_open {
            body = body.child(
                div()
                    .px(px(19.))
                    .pb(px(8.))
                    .flex()
                    .flex_col()
                    .gap(px(8.))
                    .child(div().text_size(px(12.)).text_color(rgb(0x66647a)).child("每段用 | 分开. 第一段保留起点, 最后一段保留终点."))
                    .child(flat_input(&self.movements))
                    .child(if self.pending_parts.is_empty() { String::new() } else { format!("模型建议: {}", self.pending_parts) })
                    .child(
                        div()
                            .flex()
                            .flex_row()
                            .gap(px(8.))
                            .child(primary_btn(Button::new("apply-movements").label("填入乐章").on_click(cx.listener(|this, _, window, cx| this.apply_movements(window, cx)))))
                            .child(plain_btn(Button::new("model-movements").label("用模型匹配乐章").disabled(self.busy).on_click(cx.listener(|this, _, _, cx| this.model_movements(cx)))))
                            .child(plain_btn(Button::new("use-model-movements").label("采用模型乐章").disabled(self.pending_parts.is_empty()).on_click(cx.listener(|this, _, window, cx| {
                                let text = this.pending_parts.clone();
                                set_text(&this.movements, &text, window, cx);
                                this.status = "已填入模型乐章, 可再改后点填入乐章.".into();
                                cx.notify();
                            }))))
                            .child(quiet_btn(Button::new("close-movements").label("取消").on_click(cx.listener(|this, _, _, cx| {
                                this.movement_open = false;
                                cx.notify();
                            })))),
                    ),
            );
        }
        body.child(div().px(px(19.)).pb(px(16.)).child(group_block(project, cx)))
    }

    fn source_picker(&self, project: &Project, prefix: &str, cx: &mut Context<Self>) -> impl IntoElement {
        let current = self.track_index.and_then(|index| project.tracks.get(index)).map(|track| track.source.clone()).unwrap_or_default();
        let shown = if current.trim().is_empty() { "请选择音频文件".to_string() } else { names::source_leaf(&current) };
        let menu_id = format!("{prefix}-source");
        let open = self.open_menu.as_deref() == Some(menu_id.as_str());
        let sources = project.sources.clone();
        let head_id = menu_id.clone();
        let mut column = div().flex().flex_col().gap(px(4.)).w_full().min_w(px(0.)).child(menu_box(SharedString::from(menu_id.clone()), shown).on_click(cx.listener(move |this, _, _, cx| {
            if this.project().map(|item| item.sources.is_empty()).unwrap_or(true) {
                this.status = "请先导入录音文件.".into();
                cx.notify();
                return;
            }
            if this.open_menu.as_deref() == Some(head_id.as_str()) {
                this.open_menu = None;
            } else {
                this.open_menu = Some(head_id.clone());
            }
            cx.notify();
        })));
        if open {
            let mut list = div().flex().flex_col().rounded(px(8.)).border_1().border_color(rgb(0xdcd7e6)).bg(rgb(0xffffff)).overflow_hidden();
            for (index, source) in sources.iter().enumerate() {
                let source = source.clone();
                let leaf = names::source_leaf(&source);
                let picked = source == current;
                list = list.child(
                    div()
                        .id(SharedString::from(format!("{menu_id}-{index}")))
                        .px(px(9.))
                        .py(px(7.))
                        .cursor_pointer()
                        .bg(if picked { rgb(0xf0ebfa) } else { rgb(0xffffff) })
                        .text_size(px(14.))
                        .child(leaf)
                        .on_click(cx.listener(move |this, _, _, cx| {
                            this.pull(cx);
                            if let Some(track) = this.track_mut() {
                                track.source = source.clone();
                            }
                            this.open_menu = None;
                            this.preview_key.clear();
                            this.preview_miss.clear();
                            this.preview_duration = None;
                            this.duration_task = None;
                            this.mark_dirty(cx);
                        })),
                );
            }
            column = column.child(list);
        }
        column
    }

    fn actions(&self, cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .flex()
            .flex_row()
            .gap_2()
            .child(plain_btn(Button::new("preview-wet").label(chrome::label::PREVIEW_FX).disabled(self.busy).on_click(cx.listener(|this, _, _, cx| this.preview(true, cx)))))
            .child(plain_btn(Button::new("preflight").label(chrome::label::PREFLIGHT).disabled(self.busy).on_click(cx.listener(|this, _, _, cx| this.preflight(cx)))))
            .child(primary_btn(Button::new("export-album").label(chrome::label::RUN).loading(self.busy).disabled(self.busy).on_click(cx.listener(|this, _, window, cx| this.export_album(window, cx)))))
            .child(plain_btn(Button::new("local-script").label(chrome::label::LOCAL_SCRIPT).on_click(cx.listener(|this, _, _, cx| {
                this.overlay = None;
                this.page = super::Page::Scripts;
                cx.notify();
            }))))
            .child(plain_btn(Button::new("stop-job").label(chrome::label::STOP).disabled(!self.busy).on_click(cx.listener(|this, _, _, cx| this.stop_job(cx)))))
            .child(plain_btn(Button::new("save-session").label(chrome::label::SAVE_PROJECT).on_click(cx.listener(|this, _, _, cx| {
                this.pull(cx);
                this.status = "已保存到录音项目.".into();
                this.mark_dirty(cx);
            }))))
            .child(quiet_btn(Button::new("open-saved").label(chrome::label::OPEN_SAVED).on_click(cx.listener(|this, _, _, cx| {
                this.overlay = None;
                this.page = super::Page::Projects;
                cx.notify();
            }))))
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
            let cue_name = if disc.cue_path.is_empty() { "未配对 CUE".to_string() } else { names::source_leaf(&disc.cue_path) };
            let audio_name = disc_audio_names(disc);
            let label = format!("碟 {:02}  {cue_name} → {audio_name}  {} 轨{issue}", disc.number, disc.rows.len());
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
            .child(Label::new(format!("已配对 {} / {} 张. 封面和封底全套共用.", self.cd_discs.iter().filter(|disc| disc.issue.is_empty() && !disc.rows.is_empty()).count(), self.cd_discs.len())))
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
                    .child(Button::new("cd-audio").small().label("为当前碟添加音频").on_click(cx.listener(|this, _, window, cx| this.add_disc_audio(window, cx))))
                    .child(Button::new("cd-model").small().label("用模型识别封面与封底").disabled(self.busy).on_click(cx.listener(|this, _, _, cx| this.model_cd(cx))))
                    .child(Button::new("cd-apply-model").small().label("填入模型识别").disabled(self.pending_import.is_none() || !self.pending_cd).on_click(cx.listener(|this, _, window, cx| this.apply_pending(window, cx)))),
            )
            .child(
                div()
                    .flex()
                    .flex_row()
                    .gap_2()
                    .child(Button::new("cd-import").primary().label("导入到当前项目").on_click(cx.listener(|this, _, window, cx| this.import_cd(CdImport::Append, window, cx))))
                    .child(Button::new("cd-next").label(chrome::label::APPEND_DISC).on_click(cx.listener(|this, _, window, cx| this.import_cd(CdImport::NextDisc, window, cx))))
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

    pub(super) fn scripts_page(&mut self, _cx: &mut Context<Self>) -> AnyElement {
        let (title, detail) = chrome::heading(chrome::Place::Scripts);
        let Some(project) = self.project() else {
            return div()
                .flex()
                .flex_col()
                .gap(px(8.))
                .child(div().text_size(px(22.)).child(title))
                .child(div().text_color(rgb(0x66647a)).child(detail))
                .child(div().text_color(rgb(0x66647a)).child(chrome::label::SCRIPT_EMPTY))
                .into_any_element();
        };
        let body = match export::prepare(project) {
            Ok(plan) => chrome::script_preview(&export::describe_plan(&plan)),
            Err(error) => error.to_string(),
        };
        let mut preview = div().p(px(16.)).rounded(px(12.)).bg(rgb(0x211b35)).text_color(rgb(0xf4edff)).flex().flex_col().gap_1();
        for line in body.lines() {
            preview = preview.child(if line.is_empty() { " ".to_string() } else { line.to_string() });
        }
        div()
            .flex()
            .flex_col()
            .gap(px(12.))
            .child(div().text_size(px(22.)).child(title))
            .child(div().text_color(rgb(0x66647a)).child(detail))
            .child(super::surface().child(div().text_size(px(16.)).child(chrome::label::SCRIPT_CARD)).child(preview))
            .into_any_element()
    }

    pub(super) fn history_page(&self, cx: &mut Context<Self>) -> AnyElement {
        let (title, detail) = chrome::heading(chrome::Place::History);
        let mut table = div().flex().flex_col().child(super::history_head());
        if self.workspace.logs.is_empty() {
            table = table.child(div().p(px(16.)).text_color(rgb(0x66647a)).child("还没有处理记录。"));
        }
        for entry in self.workspace.logs.iter().rev().take(80) {
            table = table.child(super::history_line(&entry.time, &entry.project_name, &entry.operation, &entry.result, &entry.message));
        }
        div()
            .flex()
            .flex_col()
            .gap(px(12.))
            .child(div().text_size(px(22.)).child(title))
            .child(div().text_color(rgb(0x66647a)).child(detail))
            .child(Button::new("clear-logs").small().danger().label("清除当前项目的记录").on_click(cx.listener(|this, _, _, cx| {
                if let Some(id) = this.project().map(|project| project.id.clone()) {
                    this.workspace.logs.retain(|entry| entry.project_id != id);
                } else {
                    this.workspace.logs.clear();
                }
                this.mark_dirty(cx);
            })))
            .child(super::surface().child(table))
            .into_any_element()
    }

    pub(super) fn templates_page(&mut self, cx: &mut Context<Self>) -> AnyElement {
        let (title, detail) = chrome::heading(chrome::Place::Templates);
        let hint = ["Source", "Order"].into_iter().chain(TAG_KEYS).map(|key| format!("{{{key}}}")).collect::<Vec<_>>().join(" · ");
        let mut list = div().flex().flex_col().gap(px(12.));
        for (index, template) in self.workspace.templates.iter().enumerate() {
            let name = template.name.clone();
            let rule = template.rule.clone();
            let output = template.output.clone();
            let summary = format!("{} · {} Hz · {}", template.settings.format, template.settings.rate, output);
            let id = template.id.clone();
            list = list.child(
                div()
                    .bg(rgb(0xffffff))
                    .border_1()
                    .border_color(rgb(0xe5e1ed))
                    .rounded(px(14.))
                    .p(px(19.))
                    .flex()
                    .flex_col()
                    .gap_2()
                    .child(div().text_size(px(16.)).child(name))
                    .child(div().text_color(rgb(0x66647a)).child(summary))
                    .child(div().child(rule))
                    .child(
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
            .gap(px(16.))
            .child(
                div()
                    .flex()
                    .flex_row()
                    .items_center()
                    .justify_between()
                    .child(div().flex().flex_col().gap(px(6.)).child(div().text_size(px(22.)).child(title)).child(div().text_color(rgb(0x66647a)).child(detail)))
                    .child(Button::new("save-template").primary().label(chrome::label::NEW_TEMPLATE).on_click(cx.listener(|this, _, _, cx| this.save_template(cx)))),
            )
            .child(
                div()
                    .bg(rgb(0xffffff))
                    .border_1()
                    .border_color(rgb(0xe5e1ed))
                    .rounded(px(14.))
                    .p(px(19.))
                    .flex()
                    .flex_col()
                    .gap_2()
                    .child(div().text_size(px(16.)).child(chrome::label::NAMING_CARD))
                    .child(div().child(self.workspace.naming.clone()))
                    .child(div().text_size(px(12.)).text_color(rgb(0x66647a)).child(format!("可用变量 {hint}"))),
            )
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
            .child(Label::new("API Key 只保存在本机, 不写入项目文件. 图片识别要勾选图像输入. 联网核对使用 DeepSeek 官方接口和 deepseek-flash."))
            .child(Form::new().child(super::field("接口", &self.llm_base)).child(super::field("模型", &self.llm_model)).child(Field::new().label("API Key").child(Input::new(&self.llm_key).small().content_type(gpui_kit::component::input::InputContentType::Password).mask_toggle())))
            .child(
                div()
                    .flex()
                    .flex_row()
                    .gap_2()
                    .child(Button::new("provider-deepseek").small().selected(self.llm.provider != "compatible").label("DeepSeek").on_click(cx.listener(|this, _, window, cx| this.choose_provider(false, window, cx))))
                    .child(Button::new("provider-compatible").small().selected(self.llm.provider == "compatible").label("兼容接口").on_click(cx.listener(|this, _, window, cx| this.choose_provider(true, window, cx))))
                    .child(Checkbox::new("llm-vision").label("模型支持图像输入").checked(self.llm.vision).on_change(cx.listener(|this, value, _, cx| {
                        this.llm.vision = *value;
                        cx.notify();
                    }))),
            )
            .child(model_choices(self, cx))
            .child(
                div()
                    .flex()
                    .flex_row()
                    .gap_2()
                    .child(Button::new("save-llm").label("保存配置").on_click(cx.listener(|this, _, window, cx| this.save_llm(window, cx))))
                    .child(Button::new("list-models").label("读取模型").disabled(self.busy).on_click(cx.listener(|this, _, _, cx| this.list_models(cx))))
                    .child(Button::new("test-llm").label("检查连接").disabled(self.busy).on_click(cx.listener(|this, _, _, cx| this.test_llm(cx))))
                    .child(Button::new("english").primary().label("生成英文场次").loading(self.busy).disabled(self.busy).on_click(cx.listener(|this, _, _, cx| this.request_english(cx))))
                    .child(Button::new("apply-english").label("应用选中项").on_click(cx.listener(|this, _, window, cx| this.apply_english(window, cx)))),
            )
            .child(review)
            .into_any_element()
    }

    fn select_track(&mut self, index: usize, window: &mut Window, cx: &mut Context<Self>) {
        self.pull(cx);
        self.track_index = Some(index);
        if let Some(track) = self.project().and_then(|project| project.tracks.get(index)) {
            self.playhead = names::time_seconds(&track.start).ok().flatten().unwrap_or(0.0);
        }
        self.player.stop();
        self.playing = false;
        self.play_generation = self.play_generation.wrapping_add(1);
        self.clear_stitch();
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

    fn add_at_playhead(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let stamp = names::format_seconds(self.playhead);
        self.add_track(window, cx);
        super::set_text(&self.start, &stamp, window, cx);
        self.pull(cx);
        self.mark_dirty(cx);
    }

    fn stamp_time(&mut self, end: bool, window: &mut Window, cx: &mut Context<Self>) {
        let stamp = names::format_seconds(self.playhead);
        if end {
            super::set_text(&self.end, &stamp, window, cx);
        } else {
            super::set_text(&self.start, &stamp, window, cx);
        }
        self.pull(cx);
        self.mark_dirty(cx);
    }

    fn clear_cover(&mut self, back: bool, cx: &mut Context<Self>) {
        self.pull(cx);
        let Some(project) = self.project_mut() else {
            return;
        };
        if back {
            project.back_cover_file.clear();
            project.back_cover_path.clear();
        } else {
            project.cover_file.clear();
            project.cover_path.clear();
        }
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

    pub(super) fn cycle_status(&mut self, cx: &mut Context<Self>) {
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
        let Some(track) = self.track_index.and_then(|index| self.project().and_then(|project| project.tracks.get(index)).cloned()) else {
            self.show_error("请先选择曲目.", cx);
            return;
        };
        if track.parts.len() >= 2 {
            self.stitch_note = processed;
            self.play_stitch_from(0.0, if processed { Some(20.0) } else { None }, cx);
            return;
        }
        let start = match names::time_seconds(&track.start) {
            Ok(value) => value.unwrap_or(0.0),
            Err(error) => {
                self.show_error(error.to_string(), cx);
                return;
            }
        };
        let limit = match names::time_seconds(&track.end) {
            Ok(Some(end)) if end > start => Some(end - start),
            Ok(_) => None,
            Err(error) => {
                self.show_error(error.to_string(), cx);
                return;
            }
        };
        self.playhead = start;
        self.clear_stitch();
        if processed {
            self.extract_preview(start, 20.0, true, false, cx);
        } else {
            self.start_listen(start, limit, cx);
        }
    }

    fn preview_window(&mut self, cx: &mut Context<Self>) {
        if self.stitch_on {
            self.play_stitch_from(self.stitch_at, Some(20.0), cx);
        } else {
            self.start_listen(self.playhead, Some(20.0), cx);
        }
    }

    pub(super) fn seek_preview(&mut self, delta: f64, cx: &mut Context<Self>) {
        if self.stitch_on {
            let next = chrome::nudge_playhead(self.stitch_at, delta).min(self.stitch_total);
            self.play_stitch_from(next, None, cx);
            return;
        }
        let mut next = chrome::nudge_playhead(self.playhead, delta);
        if let Some(duration) = self.preview_duration.filter(|value| value.is_finite() && *value > 0.0) {
            next = next.min(duration);
        }
        self.playhead = next;
        self.start_listen(next, None, cx);
    }

    fn seek_to(&mut self, at: f64, cx: &mut Context<Self>) {
        let at = if at.is_finite() { at.max(0.0) } else { 0.0 };
        self.playhead = at;
        self.start_listen(at, None, cx);
    }

    fn return_preview(&mut self, cx: &mut Context<Self>) {
        self.clear_stitch();
        self.playhead = 0.0;
        self.start_listen(0.0, None, cx);
    }

    pub(super) fn toggle_preview(&mut self, cx: &mut Context<Self>) {
        if self.playing {
            self.player.stop();
            self.playing = false;
            self.play_generation = self.play_generation.wrapping_add(1);
            self.status = "已暂停试听.".into();
            cx.notify();
            return;
        }
        if self.stitch_on {
            self.play_stitch_from(self.stitch_at, None, cx);
        } else {
            self.start_listen(self.playhead, None, cx);
        }
    }

    fn clear_stitch(&mut self) {
        self.stitch_on = false;
        self.stitch_paths.clear();
        self.stitch_slices.clear();
        self.stitch_total = 0.0;
        self.stitch_at = 0.0;
        self.stitch_index = 0;
        self.stitch_virtual_origin = 0.0;
        self.stitch_note = false;
    }

    fn start_listen(&mut self, start: f64, limit: Option<f64>, cx: &mut Context<Self>) {
        if self.busy {
            return;
        }
        if self.preview_duration.filter(|value| value.is_finite() && *value > 0.0).is_none() {
            self.pending_listen = Some((start, limit));
            if !self.preview_key.is_empty() && self.preview_miss == self.preview_key {
                self.preview_miss.clear();
                self.preview_key.clear();
                self.duration_task = None;
            }
            self.ensure_preview_duration(cx);
            return;
        }
        self.pending_listen = None;
        self.begin_playback(start, limit, cx);
    }

    fn begin_playback(&mut self, start: f64, limit: Option<f64>, cx: &mut Context<Self>) {
        if self.busy {
            return;
        }
        let start = if start.is_finite() { start.max(0.0) } else { 0.0 };
        let duration = self.preview_duration.unwrap_or(0.0);
        let span = names::listen_span(start, duration, limit);
        if span <= 0.0 {
            self.playing = false;
            self.show_error("已经到录音末尾.", cx);
            return;
        }
        let Some(path) = self.preview_path() else {
            self.show_error("请先选择曲目和录音.", cx);
            return;
        };
        self.playhead = start;
        match self.player.play_at(path, std::time::Duration::from_secs_f64(start)) {
            Ok(()) => self.begin_clock(start, span, cx),
            Err(_) => self.extract_preview(start, span.min(20.0), false, span > 20.0, cx),
        }
    }

    fn extract_preview(&mut self, start: f64, seconds: f64, processed: bool, limited: bool, cx: &mut Context<Self>) {
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
        self.player.stop();
        self.playing = false;
        self.cancel.reset();
        let cancel = self.cancel.clone();
        let settings = project.settings.clone();
        self.spawn_job(cx, move || {
            let played = (|| {
                let info = ffmpeg::probe(&binaries, &source, &cancel)?;
                let filters = if processed { effects::filter_chain(&settings, info.duration, info.channels)? } else { Vec::new() };
                export::preview_span(&binaries, &source, start, seconds, &filters, settings.rate, &cancel)
            })();
            JobMsg::Play(played.map(|path| PlayedClip { path, origin: start, span: seconds, limited }).map_err(|error| error.to_string()))
        });
    }

    fn preview_path(&self) -> Option<std::path::PathBuf> {
        let project = self.project()?;
        let track = self.track_index.and_then(|index| project.tracks.get(index))?;
        export::resolve_source(project, &track.source).ok()
    }

    fn ensure_preview_duration(&mut self, cx: &mut Context<Self>) {
        let Some(key) = self.preview_path().map(|path| path.display().to_string()) else {
            if !self.preview_key.is_empty() || self.preview_duration.is_some() {
                self.preview_key.clear();
                self.preview_miss.clear();
                self.preview_duration = None;
            }
            return;
        };
        if self.preview_key == key && (self.preview_duration.is_some() || self.duration_task.is_some() || self.preview_miss == key) {
            return;
        }
        self.preview_key = key.clone();
        self.preview_duration = None;
        self.preview_miss.clear();
        let Some(binaries) = self.ffmpeg.clone() else {
            self.preview_miss = key;
            return;
        };
        let cancel = self.cancel.clone();
        let probe_key = key.clone();
        self.duration_task = Some(cx.spawn(async move |this, cx| {
            let seconds = cx
                .background_spawn(async move { ffmpeg::probe(&binaries, std::path::Path::new(&probe_key), &cancel).ok().and_then(|info| info.duration.filter(|value| value.is_finite() && *value > 0.0)) })
                .await;
            let key_done = key;
            this.update(cx, |this, cx| {
                if this.preview_key != key_done {
                    return;
                }
                if let Some(seconds) = seconds {
                    this.preview_duration = Some(seconds);
                    if let Some((start, limit)) = this.pending_listen.take() {
                        this.begin_playback(start, limit, cx);
                    }
                } else {
                    this.preview_miss = key_done;
                    if this.pending_listen.take().is_some() {
                        this.show_error("读不到录音时长.", cx);
                    }
                }
                cx.notify();
            })
            .ok();
        }));
    }

    pub(super) fn begin_clock(&mut self, origin: f64, span: f64, cx: &mut Context<Self>) {
        self.play_origin = origin;
        self.play_span = span.max(0.0);
        self.play_mark = std::time::Instant::now();
        self.play_generation = self.play_generation.wrapping_add(1);
        let generation = self.play_generation;
        self.playing = true;
        self.status = if self.stitch_on && self.stitch_note { "拼接试听先不套用效果.".into() } else { "正在试听.".into() };
        self.clock_task = Some(cx.spawn(async move |this, cx| {
            loop {
                cx.background_executor().timer(std::time::Duration::from_millis(200)).await;
                let done = this
                    .update(cx, |this, cx| {
                        if this.play_generation != generation || !this.playing {
                            return ClockTick::Stop;
                        }
                        let elapsed = this.play_mark.elapsed().as_secs_f64();
                        if this.stitch_on {
                            this.stitch_at = this.stitch_virtual_origin + elapsed.min(this.play_span);
                        }
                        if elapsed >= this.play_span {
                            this.playhead = this.play_origin + this.play_span;
                            if this.stitch_on {
                                this.stitch_at = this.stitch_virtual_origin + this.play_span;
                                let next = this.stitch_index + 1;
                                if next < this.stitch_slices.len() {
                                    cx.notify();
                                    return ClockTick::Advance(next);
                                }
                            }
                            this.playing = false;
                            this.player.stop();
                            cx.notify();
                            return ClockTick::Stop;
                        }
                        this.playhead = this.play_origin + elapsed;
                        cx.notify();
                        ClockTick::Continue
                    })
                    .unwrap_or(ClockTick::Stop);
                match done {
                    ClockTick::Continue => {}
                    ClockTick::Stop => break,
                    ClockTick::Advance(index) => {
                        this.update(cx, |this, cx| this.play_stitch_index(index, cx)).ok();
                        break;
                    }
                }
            }
        }));
        cx.notify();
    }

    fn play_stitch_from(&mut self, from: f64, limit: Option<f64>, cx: &mut Context<Self>) {
        self.pull(cx);
        if self.busy {
            return;
        }
        let Some(project) = self.project().cloned() else {
            return;
        };
        let Some(track) = self.track_index.and_then(|index| project.tracks.get(index)).cloned() else {
            self.show_error("请先选择曲目.", cx);
            return;
        };
        let cuts = match names::stitch_cuts(&track.parts) {
            Ok(cuts) => cuts,
            Err(error) => {
                self.show_error(error.to_string(), cx);
                return;
            }
        };
        let mut paths = Vec::new();
        let mut spans = Vec::new();
        let mut missing = Vec::new();
        for cut in &cuts {
            let path = match export::resolve_source(&project, &cut.source) {
                Ok(path) => path,
                Err(error) => {
                    self.show_error(error.to_string(), cx);
                    return;
                }
            };
            if cut.end.is_none() {
                missing.push((cut.source.clone(), path.clone()));
            }
            paths.push(path);
            spans.push((cut.source.clone(), cut.start, cut.end));
        }
        if missing.is_empty() {
            let durations = vec![None; spans.len()];
            self.arm_stitch(paths, &spans, &durations, from, limit, cx);
            return;
        }
        let Some(binaries) = self.ffmpeg.clone() else {
            self.show_error("未找到裁剪版 FFmpeg.", cx);
            return;
        };
        let cancel = self.cancel.clone();
        self.stitch_task = Some(cx.spawn(async move |this, cx| {
            let found = cx
                .background_spawn(async move {
                    let mut map = std::collections::BTreeMap::new();
                    for (id, path) in missing {
                        let seconds = ffmpeg::probe(&binaries, &path, &cancel).ok().and_then(|info| info.duration);
                        map.insert(id, seconds);
                    }
                    map
                })
                .await;
            this.update(cx, |this, cx| {
                let durations: Vec<Option<f64>> = spans.iter().map(|(id, _, _)| found.get(id).copied().flatten()).collect();
                this.arm_stitch(paths, &spans, &durations, from, limit, cx);
            })
            .ok();
        }));
    }

    fn arm_stitch(&mut self, paths: Vec<std::path::PathBuf>, spans: &[(String, f64, Option<f64>)], file_durations: &[Option<f64>], from: f64, limit: Option<f64>, cx: &mut Context<Self>) {
        let times: Vec<(f64, Option<f64>)> = spans.iter().map(|(_, start, end)| (*start, *end)).collect();
        match names::stitch_queue(&times, file_durations, from, limit) {
            Ok((slices, total)) => {
                if slices.is_empty() {
                    self.show_error("已经到拼接末尾.", cx);
                    return;
                }
                self.stitch_on = true;
                self.stitch_paths = paths;
                self.stitch_slices = slices;
                self.stitch_total = total;
                self.stitch_at = from.max(0.0);
                self.play_stitch_index(0, cx);
            }
            Err(error) => self.show_error(error.to_string(), cx),
        }
    }

    fn play_stitch_index(&mut self, index: usize, cx: &mut Context<Self>) {
        let Some(slice) = self.stitch_slices.get(index).cloned() else {
            self.playing = false;
            self.player.stop();
            return;
        };
        let Some(path) = self.stitch_paths.get(slice.index).cloned() else {
            self.show_error("找不到拼接用的录音.", cx);
            return;
        };
        self.stitch_index = index;
        self.stitch_at = slice.virtual_at;
        self.stitch_virtual_origin = slice.virtual_at;
        self.playhead = slice.source_at;
        match self.player.play_at(path, std::time::Duration::from_secs_f64(slice.source_at)) {
            Ok(()) => self.begin_clock(slice.source_at, slice.seconds, cx),
            Err(error) => {
                self.playing = false;
                self.show_error(error.to_string(), cx);
            }
        }
    }

    fn seek_bar(&self, cx: &mut Context<Self>) -> impl IntoElement {
        const PARTS: usize = 40;
        let duration = if self.stitch_on { self.stitch_total } else { self.preview_duration.unwrap_or(0.0) };
        let at = if self.stitch_on { self.stitch_at } else { self.playhead };
        let played = if duration > 0.0 { (at / duration).clamp(0.0, 1.0) } else { 0.0 };
        let filled = (played * PARTS as f64).round() as usize;
        let mut bar = div().flex_1().h(px(40.)).flex().flex_row().items_center();
        for index in 0..PARTS {
            let color = if duration > 0.0 && index < filled { rgb(0x5a3da8) } else { rgb(0xc5c7cb) };
            bar = bar.child(
                div()
                    .id(SharedString::from(format!("seek-{index}")))
                    .flex_1()
                    .h(px(40.))
                    .flex()
                    .items_center()
                    .cursor_pointer()
                    .on_click(cx.listener(move |this, _, _, cx| {
                        if this.stitch_on {
                            this.play_stitch_from(names::seek_at(index, PARTS, this.stitch_total), None, cx);
                        } else {
                            let duration = this.preview_duration.unwrap_or(0.0);
                            this.seek_to(names::seek_at(index, PARTS, duration), cx);
                        }
                    }))
                    .child(div().w_full().h(px(4.)).bg(color)),
            );
        }
        bar
    }

    fn model_strip(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let mut column = div().flex().flex_col().gap(px(8.)).pt(px(8.));
        let mut row = div()
            .flex()
            .flex_row()
            .gap(px(8.))
            .child(plain_btn(Button::new("search-session").label("联网核对").disabled(self.busy).on_click(cx.listener(|this, _, _, cx| this.search_session(cx)))));
        if !self.intake_files.is_empty() {
            let count = self.intake_files.len();
            row = row.child(primary_btn(Button::new("recognize-images").label("用模型识别").disabled(self.busy).on_click(cx.listener(|this, _, _, cx| this.recognize_intake(cx))))).child(div().text_size(px(12.)).text_color(rgb(0x66647a)).child(format!("已选择 {count} 张图片")));
        }
        if self.pending_import.is_some() && !self.pending_cd {
            row = row.child(plain_btn(Button::new("apply-import").label("填入识别结果").on_click(cx.listener(|this, _, window, cx| this.apply_pending(window, cx)))));
        }
        column = column.child(row);
        for (index, hit) in self.session_hits.iter().enumerate() {
            let title = [hit.name.as_str(), hit.performer.as_str(), hit.date.as_str(), hit.venue.as_str()].into_iter().filter(|item| !item.is_empty()).collect::<Vec<_>>().join(" · ");
            let sources = hit.sources.iter().map(|source| format!("{} {}", source.title, source.url)).collect::<Vec<_>>().join(" ; ");
            column = column.child(
                div()
                    .flex()
                    .flex_row()
                    .gap(px(8.))
                    .items_center()
                    .child(plain_btn(Button::new(SharedString::from(format!("use-session-{index}"))).label("使用此场次").on_click(cx.listener(move |this, _, window, cx| this.apply_session(index, window, cx)))))
                    .child(div().text_size(px(12.)).child(format!("{title}  {sources}"))),
            );
        }
        for (index, hit) in self.name_hits.iter().enumerate() {
            let label = if hit.composer.is_empty() { hit.title.clone() } else { format!("{} - {}", hit.composer, hit.title) };
            column = column.child(plain_btn(Button::new(SharedString::from(format!("use-name-{index}"))).label(format!("采用 {label}")).on_click(cx.listener(move |this, _, window, cx| this.apply_name(index, window, cx)))));
        }
        column
    }

    fn import_program(&mut self, session_only: bool, window: &mut Window, cx: &mut Context<Self>) {
        self.pull(cx);
        let Some(files) = rfd::FileDialog::new().set_title(if session_only { "识别场次信息" } else { "导入节目单" }).add_filter("图片或文本", &["png", "jpg", "jpeg", "webp", "bmp", "gif", "txt", "md"]).pick_files() else {
            return;
        };
        let text = files.iter().filter(|path| text_file(path)).cloned().collect::<Vec<_>>();
        let images = files.iter().filter(|path| !text_file(path)).cloned().collect::<Vec<_>>();
        if !text.is_empty() && !images.is_empty() || text.len() > 1 {
            self.show_error("请单独选择一个文本, 或一次选择多张图片.", cx);
            return;
        }
        if let Some(file) = text.first() {
            let bytes = match std::fs::read(file) {
                Ok(bytes) => bytes,
                Err(error) => {
                    self.show_error(error.to_string(), cx);
                    return;
                }
            };
            self.intake_files.clear();
            let parsed = program::parse_program(&program::decode_text(&bytes));
            let (fields, tracks) = {
                let Some(project) = self.project_mut() else {
                    return;
                };
                program::apply_program(project, &parsed, !session_only)
            };
            if fields == 0 && tracks == 0 {
                self.show_error("没有识别到场次或曲目. 用「演奏家: 姓名」这样的行, 曲目用「1. 曲名」.", cx);
                return;
            }
            if tracks > 0 {
                self.track_index = Some(0);
                self.track_picks.clear();
            }
            self.push(window, cx);
            self.mark_dirty(cx);
            self.status = if tracks > 0 { format!("已填写 {fields} 项场次信息, 导入 {tracks} 条曲目.").into() } else { format!("已填写 {fields} 项场次信息.").into() };
            return;
        }
        if images.len() > 8 {
            self.show_error("每次最多向视觉模型发送 8 张图片.", cx);
            return;
        }
        self.intake_files = images;
        self.intake_session = session_only;
        self.status = format!("已选择 {} 张图片. 可点「用模型识别」, 场次也可以「联网核对」.", self.intake_files.len()).into();
        cx.notify();
    }

    fn recognize_intake(&mut self, cx: &mut Context<Self>) {
        let files = self.intake_files.clone();
        let session = self.intake_session;
        if files.is_empty() {
            self.show_error("请先选择图片.", cx);
            return;
        }
        let config = llm::config_after_endpoint_change(&self.llm, read_llm(self, cx));
        self.spawn_job(cx, move || match llm::recognize(&config, if session { "session" } else { "program" }, "", &files) {
            Ok(text) => JobMsg::Import { cd: false, include_tracks: !session, text },
            Err(error) => JobMsg::Done { operation: "模型识别", result: Err(error.to_string()) },
        });
    }

    fn search_session(&mut self, cx: &mut Context<Self>) {
        self.pull(cx);
        let Some(project) = self.project().cloned() else {
            self.show_error("请先打开项目.", cx);
            return;
        };
        let clues = serde_json::json!({
            "name": project.name,
            "performer": project.performer,
            "conductor": project.conductor,
            "orchestra": project.orchestra,
            "composer": project.composer,
            "date": project.date,
            "city": project.city,
            "venue": project.venue
        });
        let config = llm::config_after_endpoint_change(&self.llm, read_llm(self, cx));
        self.spawn_job(cx, move || match llm::search_session(&config, &clues) {
            Ok(items) => JobMsg::Sessions(Ok(items)),
            Err(error) => JobMsg::Sessions(Err(error.to_string())),
        });
    }

    fn apply_pending(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(imported) = self.pending_import.clone() else {
            return;
        };
        if self.pending_cd {
            self.fill_cd_import(&imported, window, cx);
        } else {
            self.pull(cx);
            let include = self.pending_tracks;
            let (fields, tracks) = {
                let Some(project) = self.project_mut() else { return };
                program::apply_program(project, &imported, include)
            };
            if tracks > 0 {
                self.track_index = Some(0);
                self.track_picks.clear();
            }
            self.push(window, cx);
            self.mark_dirty(cx);
            self.status = format!("已填入 {fields} 项信息和 {tracks} 条曲目.").into();
        }
        self.pending_import = None;
        self.pending_cd = false;
        cx.notify();
    }

    fn fill_cd_import(&mut self, imported: &program::ImportedProgram, window: &mut Window, cx: &mut Context<Self>) {
        if text(&self.cd_title, cx).trim().is_empty() {
            if let Some(value) = imported.name.as_deref() {
                set_text(&self.cd_title, value, window, cx);
            }
        }
        if text(&self.cd_artist, cx).trim().is_empty() {
            if let Some(value) = imported.performer.as_deref() {
                set_text(&self.cd_artist, value, window, cx);
            }
        }
        if text(&self.cd_composer, cx).trim().is_empty() {
            if let Some(value) = imported.composer.as_deref() {
                set_text(&self.cd_composer, value, window, cx);
            }
        }
        if text(&self.cd_date, cx).trim().is_empty() {
            if let Some(value) = imported.date.as_deref() {
                set_text(&self.cd_date, value, window, cx);
            }
        }
        if !imported.tracks.is_empty() {
            let album_artist = imported.performer.clone().unwrap_or_default();
            let album_composer = imported.composer.clone().unwrap_or_default();
            let rows = imported
                .tracks
                .iter()
                .enumerate()
                .map(|(index, track)| cue::CueTrack {
                    number: (index + 1) as u32,
                    source: String::new(),
                    path: String::new(),
                    start: if track.start.is_empty() { "00:00".into() } else { track.start.clone() },
                    end: track.end.clone(),
                    title: track.title.clone(),
                    artist: album_artist.clone(),
                    composer: if track.composer.is_empty() { album_composer.clone() } else { track.composer.clone() },
                    filename: String::new(),
                })
                .collect::<Vec<_>>();
            if let Some(disc) = self.cd_disc.and_then(|index| self.cd_discs.get_mut(index)) {
                disc.rows = rows;
                disc.issue.clear();
            } else {
                self.cd_discs.push(cue::DiscDraft {
                    number: 1,
                    label: "模型识别".into(),
                    cue_path: String::new(),
                    directory: String::new(),
                    files: Vec::new(),
                    album: cue::CueAlbum { title: imported.name.clone().unwrap_or_default(), artist: album_artist, composer: album_composer, date: imported.date.clone().unwrap_or_default(), disc: 1 },
                    rows,
                    issue: String::new(),
                });
                self.cd_disc = Some(self.cd_discs.len() - 1);
            }
            self.load_disc(window, cx);
        }
        self.status = "已把模型识别填入 CD 页. 请再核对轨号.".into();
    }

    fn apply_session(&mut self, index: usize, window: &mut Window, cx: &mut Context<Self>) {
        let Some(hit) = self.session_hits.get(index).cloned() else { return };
        self.pull(cx);
        let Some(project) = self.project_mut() else { return };
        let write = |slot: &mut String, value: &str| {
            if !value.trim().is_empty() {
                *slot = value.trim().to_string();
            }
        };
        write(&mut project.name, &hit.name);
        write(&mut project.performer, &hit.performer);
        write(&mut project.conductor, &hit.conductor);
        write(&mut project.orchestra, &hit.orchestra);
        write(&mut project.composer, &hit.composer);
        write(&mut project.date, &hit.date);
        write(&mut project.city, &hit.city);
        write(&mut project.venue, &hit.venue);
        self.push(window, cx);
        self.mark_dirty(cx);
        self.status = "已填入核对过的场次. 来源仍显示在下方.".into();
    }

    fn model_filename(&mut self, cx: &mut Context<Self>) {
        self.pull(cx);
        let Some(track) = self.track_index.and_then(|index| self.project().and_then(|project| project.tracks.get(index))).cloned() else {
            self.show_error("请先选择曲目.", cx);
            return;
        };
        if track.tags.title.trim().is_empty() {
            self.show_error("请先填写曲目名.", cx);
            return;
        }
        let request = serde_json::json!({"title": track.tags.title, "composer": track.tags.composer}).to_string();
        let config = llm::config_after_endpoint_change(&self.llm, read_llm(self, cx));
        self.spawn_job(cx, move || match llm::recognize(&config, "filenames", &request, &[]) {
            Ok(text) => match llm::parse_name_hits(&text) {
                Ok(items) => JobMsg::Names(Ok(items)),
                Err(error) => JobMsg::Names(Err(error.to_string())),
            },
            Err(error) => JobMsg::Names(Err(error.to_string())),
        });
    }

    fn apply_name(&mut self, index: usize, window: &mut Window, cx: &mut Context<Self>) {
        let Some(hit) = self.name_hits.get(index).cloned() else { return };
        self.pull(cx);
        let Some(track_index) = self.track_index else { return };
        {
            let Some(project) = self.project_mut() else { return };
            let Some(track) = project.tracks.get_mut(track_index) else { return };
            track.tags.title = hit.title;
            if track.tags.composer.trim().is_empty() && !hit.composer.is_empty() {
                track.tags.composer = hit.composer;
            }
        }
        let filename = {
            let Some(project) = self.project() else { return };
            let rule = names::effective_naming(project);
            let tags = names::track_tags(project, &project.tracks[track_index], track_index);
            let mut scratch = project.tracks[track_index].clone();
            scratch.filename.clear();
            names::filename_for(&rule, &scratch, &tags, track_index)
        };
        if let Some(track) = self.project_mut().and_then(|project| project.tracks.get_mut(track_index)) {
            if !filename.is_empty() {
                track.filename = filename;
            }
        }
        self.name_hits.clear();
        self.push_track(window, cx);
        self.mark_dirty(cx);
        self.status = "已采用模型曲名.".into();
    }

    fn model_movements(&mut self, cx: &mut Context<Self>) {
        self.pull(cx);
        let Some(track) = self.track_index.and_then(|index| self.project().and_then(|project| project.tracks.get(index))).cloned() else {
            self.show_error("请先选择曲目.", cx);
            return;
        };
        let composer = self.project().map(|project| if track.tags.composer.trim().is_empty() { project.composer.clone() } else { track.tags.composer.clone() }).unwrap_or_default();
        let request = serde_json::json!({"title": track.tags.title, "composer": composer}).to_string();
        let config = llm::config_after_endpoint_change(&self.llm, read_llm(self, cx));
        self.spawn_job(cx, move || match llm::recognize(&config, "movements", &request, &[]) {
            Ok(text) => match llm::parse_movements(&text) {
                Ok((_, _, parts)) => JobMsg::Parts(Ok(parts.join(" | "))),
                Err(error) => JobMsg::Parts(Err(error.to_string())),
            },
            Err(error) => JobMsg::Parts(Err(error.to_string())),
        });
    }

    fn model_cd(&mut self, cx: &mut Context<Self>) {
        let Some(project) = self.project().cloned() else {
            self.show_error("请先打开项目, 并选择套装封面或封底.", cx);
            return;
        };
        let files = [project.cover_path, project.back_cover_path].into_iter().filter(|path| !path.trim().is_empty()).map(std::path::PathBuf::from).collect::<Vec<_>>();
        if files.is_empty() {
            self.show_error("请先选择套装封面或封底.", cx);
            return;
        }
        let config = llm::config_after_endpoint_change(&self.llm, read_llm(self, cx));
        self.spawn_job(cx, move || match llm::recognize(&config, "cd", "", &files) {
            Ok(text) => JobMsg::Import { cd: true, include_tracks: true, text },
            Err(error) => JobMsg::Done { operation: "模型识别封面", result: Err(error.to_string()) },
        });
    }

    fn choose_provider(&mut self, compatible: bool, window: &mut Window, cx: &mut Context<Self>) {
        self.llm.provider = if compatible { "compatible".into() } else { "deepseek".into() };
        if !compatible {
            set_text(&self.llm_base, "https://api.deepseek.com", window, cx);
            if text(&self.llm_model, cx).trim().is_empty() {
                set_text(&self.llm_model, "deepseek-flash", window, cx);
            }
        }
        cx.notify();
    }

    fn test_llm(&mut self, cx: &mut Context<Self>) {
        let config = llm::config_after_endpoint_change(&self.llm, read_llm(self, cx));
        self.spawn_job(cx, move || match llm::test_connection(&config) {
            Ok(text) => JobMsg::Done { operation: "检查连接", result: Ok(text) },
            Err(error) => JobMsg::Done { operation: "检查连接", result: Err(error.to_string()) },
        });
    }

    fn match_filenames(&mut self, only_current: bool, window: &mut Window, cx: &mut Context<Self>) {
        self.pull(cx);
        let track_count = self.project().map(|project| project.tracks.len()).unwrap_or(0);
        let indexes: Vec<usize> = if only_current {
            match self.track_index {
                Some(index) if index < track_count => vec![index],
                _ => {
                    self.show_error("请先选择曲目.", cx);
                    return;
                }
            }
        } else {
            (0..track_count).collect()
        };
        let Some(project) = self.project_mut() else {
            return;
        };
        let rule = names::effective_naming(project);
        let mut matched = 0;
        for index in indexes {
            let title = project.tracks[index].tags.title.clone();
            let Some(hit) = catalog::suggest_work(&title) else {
                continue;
            };
            project.tracks[index].tags.title = hit.title.to_string();
            if project.tracks[index].tags.composer.trim().is_empty() {
                project.tracks[index].tags.composer = hit.composer.to_string();
            }
            let filename = {
                let tags = names::track_tags(project, &project.tracks[index], index);
                let mut scratch = project.tracks[index].clone();
                scratch.filename.clear();
                names::filename_for(&rule, &scratch, &tags, index)
            };
            project.tracks[index].filename = filename;
            matched += 1;
        }
        drop(project);
        if matched == 0 {
            self.show_error("本地曲目表没有匹配.", cx);
            return;
        }
        self.push_track(window, cx);
        self.mark_dirty(cx);
        self.status = format!("已按曲目表填写 {matched} 个文件名.").into();
    }

    fn apply_movements(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.pull(cx);
        let titles = program::movement_titles(&text(&self.movements, cx));
        if titles.is_empty() {
            self.show_error("请先写下乐章, 用 | 分开.", cx);
            return;
        }
        let Some(index) = self.track_index else {
            self.show_error("请先选择曲目.", cx);
            return;
        };
        let expanded = {
            let Some(project) = self.project_mut() else {
                return;
            };
            if index >= project.tracks.len() {
                None
            } else {
                let current = project.tracks[index].clone();
                let expanded = program::expand_movements(&current, &titles);
                let _ = project.tracks.splice(index..=index, expanded);
                Some(())
            }
        };
        if expanded.is_none() {
            self.show_error("请先选择曲目.", cx);
            return;
        }
        self.track_index = Some(index);
        self.track_picks.clear();
        self.movement_open = false;
        self.push_track(window, cx);
        self.mark_dirty(cx);
        self.status = format!("已展开 {} 个乐章.", titles.len()).into();
    }

    fn stop_job(&mut self, cx: &mut Context<Self>) {
        self.cancel.request();
        self.player.stop();
        self.playing = false;
        self.play_generation = self.play_generation.wrapping_add(1);
        self.status = "已请求停止.".into();
        cx.notify();
    }

    pub(super) fn search_work(&mut self, cx: &mut Context<Self>) {
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
        let mut files = Vec::new();
        for disc in &self.cd_discs {
            for item in &disc.files {
                if !files.iter().any(|(_, path)| path == &item.1) {
                    files.push(item.clone());
                }
            }
        }
        let prefer = file.parent().map(|parent| parent.display().to_string()).unwrap_or_default();
        match cue::parse_disc_cue_near(&file, &files, &prefer) {
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
        let mut skipped = Vec::new();
        for disc in discs {
            if disc.rows.is_empty() {
                skipped.push(format!("碟 {:02}", disc.number));
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
        self.overlay = None;
        self.push(window, cx);
        self.status = if skipped.is_empty() {
            format!("已导入 {imported} 条曲目.").into()
        } else {
            format!("已导入 {imported} 条曲目. 未导入: {}. 这些碟仍留在 CD 页.", skipped.join(", ")).into()
        };
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
            Ok(names) => JobMsg::Models(Ok(names)),
            Err(error) => JobMsg::Models(Err(error.to_string())),
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
        self.overlay = Some(super::Overlay::Llm);
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
    div()
        .flex()
        .flex_col()
        .gap(px(12.))
        .child(quad(labeled("场次 / 专辑名", &studio.name), labeled("演出日期", &studio.date), labeled("演奏家", &studio.performer), labeled("指挥家", &studio.conductor)))
        .child(quad(labeled("作曲家", &studio.composer), labeled("乐团", &studio.orchestra), labeled("城市", &studio.city), labeled("音乐厅", &studio.venue)))
        .child(quad(labeled("录音来源", &studio.origin), labeled("专辑标签日期 / 年份 (CD)", &studio.album_date), div(), div()))
}

fn output_form(studio: &Studio) -> impl IntoElement {
    div()
        .flex()
        .flex_col()
        .gap(px(8.))
        .child(labeled("专辑文件夹名称", &studio.output))
        .child(div().text_size(px(12.)).line_height(px(19.)).text_color(rgb(0x66647a)).child("留空时按照场次信息自动命名。输入自定义名称可覆盖。"))
        .child(labeled("备注", &studio.notes))
}

fn section_head(title: &'static str, actions: impl IntoElement) -> impl IntoElement {
    div().flex().flex_row().items_center().justify_between().gap(px(10.)).child(div().text_size(px(16.)).font_weight(FontWeight::SEMIBOLD).child(title)).child(actions)
}

fn quad(first: impl IntoElement, second: impl IntoElement, third: impl IntoElement, fourth: impl IntoElement) -> gpui_kit::Div {
    div()
        .flex()
        .flex_row()
        .gap(px(12.))
        .child(div().flex_1().min_w(px(0.)).child(first))
        .child(div().flex_1().min_w(px(0.)).child(second))
        .child(div().flex_1().min_w(px(0.)).child(third))
        .child(div().flex_1().min_w(px(0.)).child(fourth))
}

fn select_line(studio: &Studio, id: &'static str, label: &'static str, current: &str, options: &[(&'static str, &'static str)], cx: &mut Context<Studio>, set: fn(&mut Project, &str)) -> impl IntoElement {
    let index = options.iter().position(|(value, _)| *value == current).unwrap_or(0);
    let shown = options.get(index).map(|(_, name)| *name).unwrap_or(current).to_string();
    let open = studio.open_menu.as_deref() == Some(id);
    let mut column = div().flex().flex_col().gap(px(5.)).min_w(px(0.)).w_full().child(div().text_size(px(12.)).font_weight(FontWeight::SEMIBOLD).child(label)).child(
        menu_box(id, shown).on_click(cx.listener(move |this, _, _, cx| {
            if this.open_menu.as_deref() == Some(id) {
                this.open_menu = None;
            } else {
                this.open_menu = Some(id.to_string());
            }
            cx.notify();
        })),
    );
    if open {
        let mut list = div().flex().flex_col().rounded(px(8.)).border_1().border_color(rgb(0xdcd7e6)).bg(rgb(0xffffff)).overflow_hidden();
        for (value, name) in options {
            let value = *value;
            let picked = value == current;
            list = list.child(
                div()
                    .id(SharedString::from(format!("{id}-{value}")))
                    .px(px(9.))
                    .py(px(7.))
                    .cursor_pointer()
                    .bg(if picked { rgb(0xf0ebfa) } else { rgb(0xffffff) })
                    .text_size(px(14.))
                    .child(*name)
                    .on_click(cx.listener(move |this, _, _, cx| {
                        this.pull(cx);
                        if let Some(project) = this.project_mut() {
                            set(project, value);
                        }
                        this.open_menu = None;
                        this.mark_dirty(cx);
                    })),
            );
        }
        column = column.child(list);
    }
    column
}

fn menu_box(id: impl Into<gpui_kit::ElementId>, shown: String) -> gpui_kit::Stateful<gpui_kit::Div> {
    div()
        .id(id)
        .h(px(36.))
        .w_full()
        .px(px(9.))
        .rounded(px(8.))
        .border_1()
        .border_color(rgb(0xdcd7e6))
        .bg(rgb(0xffffff))
        .flex()
        .flex_row()
        .items_center()
        .justify_between()
        .cursor_pointer()
        .child(div().text_size(px(14.)).child(shown))
        .child(div().text_size(px(12.)).text_color(rgb(0x66647a)).child("▾"))
}

fn file_slot(id: &'static str, name: String, click: impl Fn(&gpui_kit::ClickEvent, &mut Window, &mut gpui_kit::App) + 'static) -> impl IntoElement {
    div()
        .id(id)
        .h(px(36.))
        .rounded(px(8.))
        .border_1()
        .border_color(rgb(0xdcd7e6))
        .bg(rgb(0xffffff))
        .flex()
        .flex_row()
        .items_center()
        .overflow_hidden()
        .cursor_pointer()
        .on_click(click)
        .child(div().h(px(34.)).px(px(10.)).bg(rgb(0xf6f4fa)).border_r_1().border_color(rgb(0xdcd7e6)).flex().items_center().text_size(px(13.)).child("选择文件"))
        .child(div().px(px(10.)).text_size(px(13.)).text_color(if name == "未选择任何文件" { rgb(0x66647a) } else { rgb(0x222037) }).child(name))
}

fn disc_audio_names(disc: &cue::DiscDraft) -> String {
    let mut names = Vec::new();
    for row in &disc.rows {
        let name = if row.source.is_empty() { names::source_leaf(&row.path) } else { row.source.clone() };
        if !name.is_empty() && !names.iter().any(|item| item == &name) {
            names.push(name);
        }
    }
    if names.is_empty() {
        for (name, _) in &disc.files {
            if !names.iter().any(|item| item == name) {
                names.push(name.clone());
            }
        }
    }
    if names.is_empty() { "无音频".into() } else { names.join(", ") }
}

fn labeled(label: &'static str, input: &Entity<InputState>) -> gpui_kit::Div {
    div().min_w(px(0.)).w_full().flex().flex_col().gap(px(5.)).child(div().text_size(px(12.)).font_weight(FontWeight::SEMIBOLD).child(label)).child(flat_input(input))
}

fn flat_input(input: &Entity<InputState>) -> Input {
    Input::new(input).h(px(36.)).rounded(px(8.)).bg(rgb(0xffffff)).border_1().border_color(rgb(0xdcd7e6)).text_size(px(14.))
}

fn primary_btn(button: Button) -> Button {
    button.primary().h(px(36.)).px(px(12.)).rounded(px(8.)).text_size(px(14.)).bg(rgb(0x5a3da8)).border_1().border_color(rgb(0x5a3da8)).text_color(rgb(0xffffff))
}

fn plain_btn(button: Button) -> Button {
    button.h(px(36.)).px(px(12.)).rounded(px(8.)).text_size(px(14.)).bg(rgb(0xffffff)).border_1().border_color(rgb(0xe5e1ed)).text_color(rgb(0x222037))
}

fn quiet_btn(button: Button) -> Button {
    button.ghost().h(px(36.)).px(px(8.)).rounded(px(8.)).text_size(px(14.)).text_color(rgb(0x59536d))
}

fn text_btn(button: Button) -> Button {
    button.ghost().h(px(26.)).px(px(4.)).text_size(px(12.)).text_color(rgb(0x59536d))
}

fn short_clock(seconds: f64) -> String {
    let total = seconds.max(0.0) as u64;
    format!("{}:{:02}", total / 60, total % 60)
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

fn mode_button(id: &'static str, label: &'static str, on: bool, cx: &mut Context<Studio>, mode: VideoMode) -> impl IntoElement {
    Button::new(id).small().selected(on).label(label).on_click(cx.listener(move |this, _, _, cx| {
        this.video_mode = mode;
        cx.notify();
    }))
}

fn set_format(project: &mut Project, value: &str) {
    project.settings.format = value.into();
}

fn set_rate(project: &mut Project, value: &str) {
    if let Ok(rate) = value.parse() {
        project.settings.rate = rate;
    }
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
        provider: if studio.llm.provider == "compatible" { "compatible".into() } else { "deepseek".into() },
        vision: studio.llm.vision,
    }
}

fn text_file(path: &std::path::Path) -> bool {
    matches!(path.extension().and_then(|item| item.to_str()).unwrap_or("").to_ascii_lowercase().as_str(), "txt" | "md")
}

fn model_choices(studio: &Studio, cx: &mut Context<Studio>) -> gpui_kit::Div {
    let mut row = div().flex().flex_col().gap(px(8.));
    let mut shown = llm::PRESET_MODELS.iter().map(|item| (*item).to_string()).collect::<Vec<_>>();
    for name in studio.model_choices.iter().take(40) {
        if !shown.iter().any(|item| item == name) {
            shown.push(name.clone());
        }
    }
    for name in shown {
        let label = name.clone();
        row = row.child(Button::new(SharedString::from(format!("preset-{name}"))).small().label(label).on_click(cx.listener(move |this, _, window, cx| {
            set_text(&this.llm_model, &name, window, cx);
            this.status = format!("已选择模型 {name}.").into();
            cx.notify();
        })));
    }
    row
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
