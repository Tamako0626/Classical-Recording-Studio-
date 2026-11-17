//! 分轨, CD, 封面视频, 记录, 模板和模型页.

use gpui_kit::component::{
    Disableable as _, Selectable as _, Sizable as _,
    button::{Button, ButtonVariants as _},
    checkbox::Checkbox,
    form::{Field, Form},
    input::Input,
    label::Label,
    switch::Switch,
};
use gpui_kit::{AnyElement, Context, IntoElement, ParentElement as _, SharedString, Styled as _, Window, div, px, rgb};

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
        let mut page = div().flex().flex_col().gap(px(16.)).child(self.split_toolbar(cx));
        let Some(project) = self.project().cloned() else {
            return page.child(super::surface().child(Label::new(chrome::label::SCRIPT_EMPTY))).into_any_element();
        };
        page = page
            .child(
                super::surface().child(section_head(
                    chrome::section::IMPORT,
                    div().px(px(8.)).py(px(3.)).rounded(px(999.)).bg(rgb(0xf4f1f8)).text_size(px(11.)).text_color(rgb(0x66647a)).child("本机处理"),
                )).child(self.source_block(&project, cx)),
            )
            .child(
                super::surface()
                    .child(section_head(
                        chrome::section::SESSION,
                        Button::new("english-session").small().label(chrome::label::ENGLISH).disabled(self.busy).on_click(cx.listener(|this, _, _, cx| this.request_english(cx))),
                    ))
                    .child(session_form(self)),
            )
            .child(super::surface().child(section_head(chrome::section::COVER, div())).child(self.cover_block(cx)))
            .child(
                super::surface()
                    .child(section_head(
                        chrome::section::TRACKS,
                        div()
                            .flex()
                            .flex_row()
                            .gap_2()
                            .child(Button::new("fill-tags").small().label("按场次更新全部标签").on_click(cx.listener(|this, _, window, cx| this.fill_tags(window, cx))))
                            .child(Button::new("add-track").small().primary().label(chrome::label::ADD_TRACK).on_click(cx.listener(|this, _, window, cx| this.add_track(window, cx))))
                            .child(Button::new("add-at-playhead").small().label("＋ 从播放位置添加").on_click(cx.listener(|this, _, window, cx| this.add_at_playhead(window, cx)))),
                    ))
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
                    Button::new("open-video").primary().label(chrome::label::COVER_VIDEO).on_click(cx.listener(|this, _, _, cx| {
                        this.overlay = Some(super::Overlay::Video);
                        cx.notify();
                    })),
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
            .child(div().flex().flex_col().gap(px(6.)).child(div().text_size(px(28.)).child(title)).child(div().text_color(rgb(0x66647a)).child(detail)))
            .child(
                div()
                    .flex()
                    .flex_row()
                    .gap_2()
                    .child(Button::new("open-cd").primary().label(chrome::label::CD).on_click(cx.listener(|this, _, _, cx| {
                        this.overlay = Some(super::Overlay::Cd);
                        cx.notify();
                    })))
                    .child(Button::new("new-session").label(chrome::label::NEW_SESSION).on_click(cx.listener(|this, _, window, cx| this.create_project(window, cx)))),
            )
    }

    fn transport(&self, cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .flex()
            .flex_row()
            .gap_2()
            .child(Button::new("back-five").small().label(chrome::label::BACK_FIVE).disabled(self.busy).on_click(cx.listener(|this, _, _, cx| this.seek_preview(-chrome::SEEK_SECONDS, cx))))
            .child(Button::new("forward-five").small().label(chrome::label::FORWARD_FIVE).disabled(self.busy).on_click(cx.listener(|this, _, _, cx| this.seek_preview(chrome::SEEK_SECONDS, cx))))
            .child(Button::new("preview-raw").small().label(chrome::label::PREVIEW_FROM).disabled(self.busy).on_click(cx.listener(|this, _, _, cx| this.preview(false, cx))))
            .child(Button::new("preview-native").small().label(chrome::label::RETURN_PREVIEW).disabled(self.busy).on_click(cx.listener(|this, _, _, cx| this.return_preview(cx))))
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
                    .gap_2()
                    .child(Button::new("pick-cover").primary().label(chrome::label::UPLOAD_COVER).on_click(cx.listener(|this, _, window, cx| this.pick_cover(false, window, cx))))
                    .child(div().text_size(px(12.)).text_color(rgb(0x66647a)).child("或上传现成封面 · PNG / JPG"))
                    .child(div().px(px(10.)).py(px(6.)).border_1().border_color(rgb(0xe5e1ed)).rounded(px(8.)).child(cover_name))
                    .child(Button::new("clear-cover").small().ghost().label("移除封面").on_click(cx.listener(|this, _, _, cx| this.clear_cover(false, cx)))),
            )
            .child(
                div()
                    .flex()
                    .flex_row()
                    .items_center()
                    .gap_2()
                    .child(div().text_size(px(12.)).text_color(rgb(0x66647a)).child("封底（可选） · PNG / JPG"))
                    .child(Button::new("pick-back").small().label("选择文件").on_click(cx.listener(|this, _, window, cx| this.pick_cover(true, window, cx))))
                    .child(div().px(px(10.)).py(px(6.)).border_1().border_color(rgb(0xe5e1ed)).rounded(px(8.)).child(back_name))
                    .child(Button::new("clear-back").small().ghost().label("移除封底").on_click(cx.listener(|this, _, _, cx| this.clear_cover(true, cx)))),
            )
            .child(div().text_size(px(12.)).text_color(rgb(0x66647a)).child("可留空。"))
    }

    fn settings_block(&self, project: &Project, cx: &mut Context<Self>) -> impl IntoElement {
        let settings = &project.settings;
        let rate = settings.rate.to_string();
        div()
            .flex()
            .flex_col()
            .gap(px(12.))
            .child(quad(
                select_line("format-cycle", "输出格式", &settings.format, &[("FLAC", "FLAC"), ("WAV", "WAV"), ("MP3", "MP3"), ("MKV", "MKV")], cx, set_format),
                select_line("rate-cycle", "采样率", &rate, &[("44100", "44.1 kHz"), ("48000", "48 kHz"), ("96000", "96 kHz")], cx, set_rate),
                Form::new().child(super::field("增益（dB）", &self.gain)),
                Form::new().child(super::field("声场宽度（%）", &self.width)),
            ))
            .child(
                div().flex().flex_row().items_center().gap_2().child(
                    Switch::new("limiter")
                        .label("启用防爆音限幅")
                        .checked(self.limiter_on)
                        .on_change(cx.listener(|this, value, _, cx| {
                            this.limiter_on = *value;
                            this.pull(cx);
                            this.mark_dirty(cx);
                        })),
                ).child(Input::new(&self.limiter).small()),
            )
            .child(quad(
                Form::new().child(super::field("混响（%）", &self.reverb)),
                Form::new().child(super::field("英文命名规则", &self.naming)),
                div(),
                div(),
            ))
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
            Button::new("toggle-effects")
                .small()
                .ghost()
                .label(if self.effects_open { "▼ 高级后期处理" } else { "▶ 高级后期处理" })
                .on_click(cx.listener(|this, _, _, cx| {
                    this.effects_open = !this.effects_open;
                    cx.notify();
                })),
        );
        if !self.effects_open {
            return block;
        }
        block = block
            .child(div().text_size(px(14.)).child("降噪与修复"))
            .child(quad(
                select_line("denoise-cycle", "降噪方式", &effects.denoise, &[("off", "关闭"), ("fft", "FFT"), ("nlm", "非局部均值")], cx, set_denoise),
                Form::new().child(super::field("FFT 降噪量（dB）", &self.noise_reduction)),
                select_line("hum-cycle", "电源噪声", &effects.hum, &[("off", "关闭"), ("50", "50 Hz"), ("60", "60 Hz")], cx, set_hum),
                div().flex().flex_row().gap(px(8.)).items_center().child(check("declick", "去点击", effects.declick, cx, |project, value| project.settings.effects.declick = value)).child(check("declip", "修复削波", effects.declip, cx, |project, value| project.settings.effects.declip = value)),
            ))
            .child(div().text_size(px(14.)).child("频率与声道"))
            .child(quad(
                Form::new().child(super::field("高通（Hz，0 关闭）", &self.highpass)),
                Form::new().child(super::field("低通（Hz，0 关闭）", &self.lowpass)),
                Form::new().child(super::field("低频 100 Hz（dB）", &self.bass)),
                Form::new().child(super::field("中频 1 kHz（dB）", &self.mid)),
            ))
            .child(quad(Form::new().child(super::field("高频 6 kHz（dB）", &self.treble)), select_line("channels-cycle", "声道", &effects.channels, &[("source", "保持来源"), ("mono", "单声道"), ("swap", "交换左右"), ("invert", "反转右声道")], cx, set_channels), div(), div()))
            .child(div().text_size(px(14.)).child("动态与淡化"))
            .child(quad(
                Form::new().child(super::field("噪声门（dBFS，0 关闭）", &self.gate)),
                Form::new().child(super::field("压缩比（1 关闭）", &self.compress)),
                Form::new().child(super::field("压缩阈值（dBFS）", &self.threshold)),
                div().flex().flex_row().items_center().child(check("deess", "减弱齿音", effects.deess, cx, |project, value| project.settings.effects.deess = value)),
            ))
            .child(quad(
                select_line("normalize-cycle", "响度处理", &effects.normalize, &[("off", "关闭"), ("loud", "EBU R128"), ("dynamic", "动态")], cx, set_normalize),
                Form::new().child(super::field("目标响度（LUFS）", &self.lufs)),
                Form::new().child(super::field("淡入（秒）", &self.fade_in)),
                Form::new().child(super::field("淡出（秒）", &self.fade_out)),
            ))
            .child(div().text_size(px(12.)).text_color(rgb(0x66647a)).child("默认关闭。FFT 适用于持续底噪，非局部均值处理较慢。响度和动态处理建议试听后再批量导出。"));
        block
    }

    fn source_block(&self, project: &Project, cx: &mut Context<Self>) -> impl IntoElement {
        let mut list = div().flex().flex_col().gap_1();
        for (index, source) in project.sources.iter().enumerate() {
            list = list.child(format!("{}. {source}", index + 1));
        }
        let clock = names::format_seconds(self.playhead);
        let short = short_clock(self.playhead);
        let info = if project.sources.is_empty() { "请选择原始音频文件。" } else { "使用所选文件试听整场录音。" };
        let preview = if project.sources.is_empty() {
            div().px(px(12.)).py(px(8.)).border_1().border_color(rgb(0xe5e1ed)).rounded(px(8.)).flex().flex_row().justify_between().text_color(rgb(0x66647a)).child("请选择音频文件").child("▾").into_any_element()
        } else {
            self.source_picker(project, "preview", cx).into_any_element()
        };
        div()
            .flex()
            .flex_col()
            .gap(px(10.))
            .child(
                div()
                    .flex()
                    .flex_row()
                    .gap_2()
                    .child(Button::new("add-audio").primary().label(chrome::label::IMPORT_AUDIO).on_click(cx.listener(|this, _, window, cx| this.add_audio(window, cx))))
                    .child(Button::new("llm-audio").label(chrome::label::RECOGNIZE_AUDIO).on_click(cx.listener(|this, _, _, cx| {
                        this.overlay = Some(super::Overlay::Llm);
                        cx.notify();
                    }))),
            )
            .child(div().text_size(px(12.)).text_color(rgb(0x66647a)).child("支持 WAV、FLAC、APE、MP3、MKV 多选。"))
            .child(list)
            .child(
                div()
                    .flex()
                    .flex_row()
                    .gap_2()
                    .child(Button::new("concat-sources").small().primary().label(chrome::label::CONCAT).on_click(cx.listener(|this, _, window, cx| this.concat_sources(window, cx))))
                    .child(Button::new("relink-audio").small().ghost().label("重新关联当前曲目").on_click(cx.listener(|this, _, window, cx| this.relink(window, cx))))
                    .child(Button::new("tracks-from-sources").small().ghost().label("按录音各加一轨").on_click(cx.listener(|this, _, window, cx| this.tracks_from_sources(window, cx))))
                    .child(Button::new("relink-tree").small().ghost().label("按文件夹重新关联").on_click(cx.listener(|this, _, window, cx| this.relink_tree(window, cx))))
                    .child(Button::new("source-root").small().ghost().label("设置素材根目录").on_click(cx.listener(|this, _, window, cx| this.choose_root(window, cx)))),
            )
            .child(div().text_size(px(13.)).child("当前试听文件"))
            .child(preview)
            .child(div().text_size(px(12.)).text_color(rgb(0x66647a)).child(info))
            .child(
                div()
                    .border_1()
                    .border_color(rgb(0xe5e1ed))
                    .rounded(px(24.))
                    .px(px(14.))
                    .py(px(8.))
                    .flex()
                    .flex_row()
                    .items_center()
                    .gap(px(12.))
                    .child(Button::new("play-toggle").small().label(if self.playing { "暂停" } else { "▶" }).disabled(self.busy).on_click(cx.listener(|this, _, _, cx| this.toggle_preview(cx))))
                    .child(format!("{short} / 0:00"))
                    .child(div().flex_1().h(px(4.)).rounded(px(4.)).bg(rgb(0xe5e1ed))),
            )
            .child(
                div()
                    .flex()
                    .flex_row()
                    .items_center()
                    .gap_2()
                    .child(div().text_color(rgb(0x66647a)).child(clock))
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
                .gap_2()
                .px(px(8.))
                .py(px(6.))
                .bg(rgb(0xf6f4fa))
                .text_size(px(11.))
                .text_color(rgb(0x66647a))
                .child(div().w(px(28.)).child(""))
                .child(div().w(px(40.)).child(columns[0]))
                .child(div().flex_1().child(columns[1]))
                .child(div().w(px(120.)).child(columns[2]))
                .child(div().w(px(72.)).child(columns[3]))
                .child(div().w(px(72.)).child(columns[4]))
                .child(div().w(px(56.)).child(columns[5])),
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
                .flex()
                .flex_col()
                .gap_1()
                .child(Button::new(SharedString::from(format!("hear-{id}"))).xsmall().ghost().label("试听").disabled(self.busy).on_click(cx.listener(move |this, _, window, cx| {
                    this.select_track(index, window, cx);
                    this.preview(false, cx);
                })))
                .child(Button::new(SharedString::from(format!("tags-{id}"))).xsmall().ghost().label("编辑标签").on_click(cx.listener(move |this, _, window, cx| {
                    this.select_track(index, window, cx);
                    this.tags_open = !this.tags_open;
                    cx.notify();
                })));
            if selected == Some(index) {
                let prefix = format!("row{index}");
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
                            div()
                                .flex_1()
                                .min_w(px(0.))
                                .flex()
                                .flex_col()
                                .gap_1()
                                .child(Input::new(&self.tags[0]).small())
                                .child(Input::new(&self.filename).small())
                                .child(div().text_size(px(11.)).text_color(rgb(0x66647a)).child("留空时按命名规则生成")),
                        )
                        .child(div().w(px(160.)).child(self.source_picker(project, &prefix, cx)))
                        .child(
                            div().w(px(120.)).flex().flex_col().gap_1().child(Input::new(&self.start).small()).child(Button::new(SharedString::from(format!("take-start-{id}"))).xsmall().ghost().label("取播放点").on_click(cx.listener(|this, _, window, cx| this.stamp_time(false, window, cx)))),
                        )
                        .child(
                            div().w(px(120.)).flex().flex_col().gap_1().child(Input::new(&self.end).small()).child(Button::new(SharedString::from(format!("take-end-{id}"))).xsmall().ghost().label("取播放点").on_click(cx.listener(|this, _, window, cx| this.stamp_time(true, window, cx)))),
                        )
                        .child(actions),
                );
            } else {
                let title = if track.tags.title.trim().is_empty() { "未命名曲目".to_string() } else { track.tags.title.clone() };
                let leaf = if track.source.is_empty() { "请选择音频文件".to_string() } else { names::source_leaf(&track.source) };
                let end = if track.end.trim().is_empty() { "自动".to_string() } else { track.end.clone() };
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
                                Button::new(SharedString::from(format!("track-{id}")))
                                    .small()
                                    .ghost()
                                    .label(title)
                                    .on_click(cx.listener(move |this, _, window, cx| this.select_track(index, window, cx))),
                            ),
                        )
                        .child(div().w(px(160.)).child(leaf))
                        .child(div().w(px(120.)).child(track.start.clone()))
                        .child(div().w(px(120.)).child(end))
                        .child(actions),
                );
            }
        }
        let mut body = div()
            .flex()
            .flex_col()
            .gap_2()
            .child(check("auto-join", chrome::label::JOIN, project.auto_join, cx, |project, value| project.auto_join = value))
            .child(div().text_size(px(12.)).text_color(rgb(0x66647a)).child("每轨可选择不同的音频文件，起止点以所选文件为准。可输入 18:22 或 01:18:22.500。点击「取播放点」填写当前时间，点击「试听」从本轨起点播放。"))
            .child(list);
        if self.tags_open && selected.is_some() {
            body = body.child(tag_form(self));
        }
        body.child(
            div()
                .flex()
                .flex_row()
                .gap_2()
                .child(Button::new("remove-track").small().danger().label("删除当前曲目").on_click(cx.listener(|this, _, window, cx| this.remove_track(window, cx))))
                .child(Button::new("apply-names").small().ghost().label("按规则填写空文件名").on_click(cx.listener(|this, _, window, cx| this.apply_names(window, cx))))
                .child(Button::new("local-work").small().ghost().label("采用本地英文曲名").on_click(cx.listener(|this, _, window, cx| this.apply_catalog(window, cx))))
                .child(Button::new("merge-tracks").small().label(chrome::label::MERGE).disabled(self.track_picks.len() < 2).on_click(cx.listener(|this, _, window, cx| {
                    this.merge_selected(window, cx);
                })))
                .child(Button::new("split-merged").small().label(chrome::label::UNMERGE).on_click(cx.listener(|this, _, window, cx| this.split_merged(window, cx))))
                .child(Button::new("merge-groups").small().label(chrome::label::MERGE_GROUPS).on_click(cx.listener(|this, _, window, cx| this.merge_groups(window, cx))))
                .child(Button::new("cycle-status").small().label(format!("状态: {}", project.status)).on_click(cx.listener(|this, _, _, cx| this.cycle_status(cx)))),
        )
        .child(group_block(project, cx))
    }

    fn source_picker(&self, project: &Project, prefix: &str, cx: &mut Context<Self>) -> impl IntoElement {
        let current = self.track_index.and_then(|index| project.tracks.get(index)).map(|track| track.source.clone()).unwrap_or_default();
        if project.sources.is_empty() {
            return div().px(px(12.)).py(px(8.)).border_1().border_color(rgb(0xe5e1ed)).rounded(px(8.)).flex().flex_row().justify_between().text_color(rgb(0x66647a)).child("请选择音频文件").child("▾");
        }
        let mut row = div().flex().flex_row().gap_1();
        for source in &project.sources {
            let name = source.clone();
            let selected = name == current;
            let label = if name.contains('/') || name.contains('\\') { name.clone() } else { names::source_leaf(&name) };
            let id = SharedString::from(format!("{prefix}-use-{name}"));
            row = row.child(
                Button::new(id)
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
            .child(Button::new("preview-wet").label(chrome::label::PREVIEW_FX).disabled(self.busy).on_click(cx.listener(|this, _, _, cx| this.preview(true, cx))))
            .child(Button::new("preflight").label(chrome::label::PREFLIGHT).disabled(self.busy).on_click(cx.listener(|this, _, _, cx| this.preflight(cx))))
            .child(Button::new("export-album").primary().label(chrome::label::RUN).loading(self.busy).disabled(self.busy).on_click(cx.listener(|this, _, window, cx| this.export_album(window, cx))))
            .child(Button::new("local-script").label(chrome::label::LOCAL_SCRIPT).on_click(cx.listener(|this, _, _, cx| {
                this.overlay = None;
                this.page = super::Page::Scripts;
                cx.notify();
            })))
            .child(Button::new("stop-job").danger().label(chrome::label::STOP).disabled(!self.busy).on_click(cx.listener(|this, _, _, cx| this.stop_job(cx))))
            .child(Button::new("save-session").label(chrome::label::SAVE_PROJECT).on_click(cx.listener(|this, _, _, cx| {
                this.pull(cx);
                this.status = "已保存到录音项目.".into();
                this.mark_dirty(cx);
            })))
            .child(Button::new("open-saved").label(chrome::label::OPEN_SAVED).on_click(cx.listener(|this, _, _, cx| {
                this.overlay = None;
                this.page = super::Page::Projects;
                cx.notify();
            })))
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
        if let Some(track) = self.project().and_then(|project| project.tracks.get(index)) {
            self.playhead = names::time_seconds(&track.start).ok().flatten().unwrap_or(0.0);
        }
        self.playing = false;
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
        let start = match names::time_seconds(&track.start) {
            Ok(value) => value.unwrap_or(0.0),
            Err(error) => {
                self.show_error(error.to_string(), cx);
                return;
            }
        };
        self.playhead = start;
        self.preview_at(start, processed, cx);
    }

    pub(super) fn seek_preview(&mut self, delta: f64, cx: &mut Context<Self>) {
        self.playhead = chrome::nudge_playhead(self.playhead, delta);
        self.preview_at(self.playhead, false, cx);
    }

    fn return_preview(&mut self, cx: &mut Context<Self>) {
        self.playhead = 0.0;
        self.preview_at(0.0, false, cx);
    }

    pub(super) fn toggle_preview(&mut self, cx: &mut Context<Self>) {
        if self.playing {
            self.player.stop();
            self.playing = false;
            self.status = "已暂停试听.".into();
            cx.notify();
            return;
        }
        self.preview(false, cx);
    }

    fn preview_at(&mut self, start: f64, processed: bool, cx: &mut Context<Self>) {
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
        self.playing = false;
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
        self.overlay = None;
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
        .child(quad(
            Form::new().child(super::field("场次 / 专辑名", &studio.name)),
            Form::new().child(super::field("演出日期", &studio.date)),
            Form::new().child(super::field("演奏家", &studio.performer)),
            Form::new().child(super::field("指挥家", &studio.conductor)),
        ))
        .child(quad(
            Form::new().child(super::field("作曲家", &studio.composer)),
            Form::new().child(super::field("乐团", &studio.orchestra)),
            Form::new().child(super::field("城市", &studio.city)),
            Form::new().child(super::field("音乐厅", &studio.venue)),
        ))
        .child(quad(Form::new().child(super::field("录音来源", &studio.origin)), Form::new().child(super::field("专辑标签日期 / 年份 (CD)", &studio.album_date)), div(), div()))
}

fn output_form(studio: &Studio) -> impl IntoElement {
    div()
        .flex()
        .flex_col()
        .gap(px(8.))
        .child(Form::new().child(super::field("专辑文件夹名称", &studio.output)))
        .child(div().text_size(px(12.)).text_color(rgb(0x66647a)).child("留空时按照场次信息自动命名。输入自定义名称可覆盖。"))
        .child(Form::new().child(super::field("备注", &studio.notes)))
}

fn section_head(title: &'static str, actions: impl IntoElement) -> impl IntoElement {
    div().flex().flex_row().items_center().justify_between().gap_2().child(div().text_size(px(16.)).child(title)).child(actions)
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

fn select_line(id: &'static str, label: &'static str, current: &str, options: &[(&'static str, &'static str)], cx: &mut Context<Studio>, set: fn(&mut Project, &str)) -> impl IntoElement {
    let index = options.iter().position(|(value, _)| *value == current).unwrap_or(0);
    let shown = options.get(index).map(|(_, name)| *name).unwrap_or(current);
    let next = options[(index + 1) % options.len()].0;
    div().flex().flex_col().gap(px(4.)).child(div().text_size(px(12.)).text_color(rgb(0x66647a)).child(label)).child(
        Button::new(id).small().label(format!("{shown}  ▾")).on_click(cx.listener(move |this, _, _, cx| {
            this.pull(cx);
            if let Some(project) = this.project_mut() {
                set(project, next);
            }
            this.mark_dirty(cx);
        })),
    )
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
