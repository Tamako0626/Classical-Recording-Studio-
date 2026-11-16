//! 与原先网页版相同的 ffmpeg 滤镜链.

use crate::error::{StudioError, StudioResult};
use crate::model::{Effects, Settings};

fn db(value: f64) -> String {
    format!("{:.7}", 10_f64.powf(value / 20.0))
}

fn seconds(value: f64) -> String {
    format!("{value:.6}")
}

pub fn effects_valid(effects: &Effects) -> bool {
    let denoise = matches!(effects.denoise.as_str(), "off" | "fft" | "nlm");
    let hum = matches!(effects.hum.as_str(), "off" | "50" | "60");
    let channels = matches!(effects.channels.as_str(), "source" | "mono" | "swap" | "invert");
    let normalize = matches!(effects.normalize.as_str(), "off" | "loud" | "dynamic");
    let ranged = |value: f64, min: f64, max: f64| value.is_finite() && value >= min && value <= max;
    denoise
        && hum
        && channels
        && normalize
        && ranged(effects.noise_reduction, 1.0, 30.0)
        && ranged(effects.highpass, 0.0, 250.0)
        && ranged(effects.lowpass, 0.0, 20000.0)
        && ranged(effects.bass, -15.0, 15.0)
        && ranged(effects.mid, -15.0, 15.0)
        && ranged(effects.treble, -15.0, 15.0)
        && ranged(effects.gate, -60.0, 0.0)
        && ranged(effects.compress, 1.0, 8.0)
        && ranged(effects.threshold, -60.0, 0.0)
        && ranged(effects.lufs, -30.0, -10.0)
        && ranged(effects.fade_in, 0.0, 30.0)
        && ranged(effects.fade_out, 0.0, 30.0)
        && ranged(effects.limiter, -18.0, 0.0)
        && (effects.lowpass == 0.0 || (effects.lowpass >= 100.0 && effects.lowpass > effects.highpass))
        && (effects.gate == 0.0 || effects.gate <= -1.0)
}

pub fn filter_chain(settings: &Settings, duration: Option<f64>, channels: u32) -> StudioResult<Vec<String>> {
    if !effects_valid(&settings.effects) {
        return Err(StudioError::new("后期处理参数无效. 低通至少为 100 Hz 且高于高通, 噪声门阈值须为负数或 0."));
    }
    if !settings.gain.is_finite() || !(-60.0..=30.0).contains(&settings.gain) {
        return Err(StudioError::new("增益超出 -60 到 30 dB."));
    }
    if !settings.width.is_finite() || !(0.0..=250.0).contains(&settings.width) {
        return Err(StudioError::new("声场宽度超出 0 到 250."));
    }
    if !settings.reverb.is_finite() || !(0.0..=100.0).contains(&settings.reverb) {
        return Err(StudioError::new("混响超出 0 到 100."));
    }
    let effects = &settings.effects;
    let stereo = matches!(effects.channels.as_str(), "swap" | "invert");
    if stereo && channels != 2 {
        return Err(StudioError::new("交换声道或反转右声道极性需要双声道录音."));
    }
    if settings.width != 100.0 && channels != 2 {
        return Err(StudioError::new("声场宽度调整需要双声道录音."));
    }
    if effects.fade_out > 0.0 && duration.is_none_or(|value| value <= 0.0) {
        return Err(StudioError::new("淡出需要有效的音轨时长, 请填写终点."));
    }

    let mut before = Vec::new();
    if effects.highpass > 0.0 {
        before.push(format!("highpass=f={}", effects.highpass));
    }
    if effects.lowpass > 0.0 {
        before.push(format!("lowpass=f={}", effects.lowpass));
    }
    if effects.hum != "off" {
        let base: f64 = effects.hum.parse().unwrap_or(50.0);
        for harmonic in 1..=3 {
            before.push(format!("equalizer=f={}:t=q:w=20:g=-24", base * harmonic as f64));
        }
    }
    if effects.declip {
        before.push("adeclip".into());
    }
    if effects.declick {
        before.push("adeclick".into());
    }
    if effects.denoise == "fft" {
        before.push(format!("afftdn=nr={}:tn=1", effects.noise_reduction));
    }
    if effects.denoise == "nlm" {
        before.push("anlmdn=s=0.0001".into());
    }
    if effects.bass != 0.0 {
        before.push(format!("bass=f=100:g={}", effects.bass));
    }
    if effects.mid != 0.0 {
        before.push(format!("equalizer=f=1000:t=q:w=1:g={}", effects.mid));
    }
    if effects.treble != 0.0 {
        before.push(format!("treble=f=6000:g={}", effects.treble));
    }
    if effects.gate != 0.0 {
        before.push(format!("agate=threshold={}:ratio=3:attack=10:release=250", db(effects.gate)));
    }
    if effects.channels == "swap" {
        before.push("pan=stereo|c0=c1|c1=c0".into());
    }
    if effects.channels == "invert" {
        before.push("pan=stereo|c0=c0|c1=-1*c1".into());
    }

    let mut filters = before;
    if settings.gain != 0.0 {
        filters.push(format!("volume={}dB", settings.gain));
    }
    if settings.width != 100.0 {
        filters.push(format!("extrastereo=c=0:m={}", settings.width / 100.0));
    }
    if settings.reverb > 0.0 {
        let mix = settings.reverb / 100.0;
        let delays = [0.22, 0.14, 0.08]
            .into_iter()
            .map(|gain| format!("{:.5}", mix * gain))
            .collect::<Vec<_>>()
            .join("|");
        filters.push(format!("aecho=1:1:35|61|89:{delays}"));
    }
    if effects.channels == "mono" {
        filters.push("aformat=channel_layouts=mono".into());
    }
    if effects.deess {
        filters.push("deesser".into());
    }
    if effects.compress > 1.0 {
        filters.push(format!(
            "acompressor=threshold={}:ratio={}:attack=20:release=250",
            db(effects.threshold),
            effects.compress
        ));
    }
    if effects.normalize == "loud" {
        filters.push(format!("loudnorm=I={}:TP=-1.5:LRA=11", effects.lufs));
    }
    if effects.normalize == "dynamic" {
        filters.push("dynaudnorm".into());
    }
    if effects.fade_in > 0.0 {
        let length = duration.unwrap_or(f64::INFINITY);
        filters.push(format!("afade=t=in:d={}", seconds(effects.fade_in.min(length))));
    }
    if let Some(duration) = duration.filter(|_| effects.fade_out > 0.0) {
        let fade = effects.fade_out.min(duration);
        filters.push(format!("afade=t=out:st={}:d={}", seconds((duration - fade).max(0.0)), seconds(fade)));
    }
    if effects.limiter != 0.0 {
        filters.push(format!("alimiter=limit={}:level=false:latency=true", db(effects.limiter)));
    }
    Ok(filters)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn builds_the_same_chain_as_the_previous_app() {
        let mut settings = Settings::default();
        settings.effects.highpass = 80.0;
        settings.effects.hum = "50".into();
        settings.effects.denoise = "fft".into();
        settings.effects.noise_reduction = 12.0;
        settings.effects.channels = "swap".into();
        settings.effects.fade_in = 1.0;
        settings.effects.fade_out = 2.0;
        let chain = filter_chain(&settings, Some(10.0), 2).unwrap();
        assert_eq!(
            chain,
            vec![
                "highpass=f=80",
                "equalizer=f=50:t=q:w=20:g=-24",
                "equalizer=f=100:t=q:w=20:g=-24",
                "equalizer=f=150:t=q:w=20:g=-24",
                "afftdn=nr=12:tn=1",
                "pan=stereo|c0=c1|c1=c0",
                "afade=t=in:d=1.000000",
                "afade=t=out:st=8.000000:d=2.000000",
            ]
        );
    }
}
