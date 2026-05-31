/// 千位分隔符。
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum Separator {
    #[default]
    Comma,
    Dot,
    Nothing,
}

/// 格式化配置（对齐 zenith-midi 的 Padding 设置）。
#[derive(Clone, Debug)]
pub struct FormatConfig {
    pub separator: Separator,
    pub zero_padding: bool,
    pub bpm_int_pad: usize,
    pub bpm_dec_pad: usize,
    pub note_count_pad: usize,
    pub polyphony_pad: usize,
    pub nps_pad: usize,
    pub ticks_pad: usize,
    pub bars_pad: usize,
    pub frames_pad: usize,
}

impl Default for FormatConfig {
    fn default() -> Self {
        Self {
            separator: Separator::Comma,
            zero_padding: false,
            bpm_int_pad: 3,
            bpm_dec_pad: 2,
            note_count_pad: 5,
            polyphony_pad: 3,
            nps_pad: 3,
            ticks_pad: 5,
            bars_pad: 3,
            frames_pad: 5,
        }
    }
}

/// 模板变量 —— 所有计数器可能用到的值。
#[derive(Clone, Debug, Default)]
pub struct TemplateVars {
    pub bpm: f64,
    pub note_count: u64,
    pub notes_remaining: u64,
    pub total_notes: u64,
    pub nps: u64,
    pub max_nps: u64,
    pub polyphony: u64,
    pub max_polyphony: u64,
    pub total_instances: u64,
    pub curr_sec: f64,
    pub curr_time: Option<String>,
    pub cmil_time: Option<String>,
    pub cfr_time: Option<String>,
    pub total_sec: f64,
    pub total_time: Option<String>,
    pub tmil_time: Option<String>,
    pub tfr_time: Option<String>,
    pub rem_sec: f64,
    pub rem_time: Option<String>,
    pub rmil_time: Option<String>,
    pub rfr_time: Option<String>,
    pub curr_ticks: u64,
    pub total_ticks: u64,
    pub rem_ticks: u64,
    pub curr_bars: u64,
    pub total_bars: u64,
    pub rem_bars: u64,
    pub ppq: u32,
    pub time_sig_num: u8,
    pub time_sig_den: u8,
    pub avg_nps: f64,
    pub curr_frames: u64,
    pub total_frames: u64,
    pub rem_frames: u64,
    pub note_percent: f64,
    pub tick_percent: f64,
    pub time_percent: f64,
    /// 帧率，用于惰性计算时间格式化字符串。
    pub fps: u32,
}

// ── 模板 key 索引常量 ──
const K_BPM: usize = 0;
const K_NC: usize = 1;
const K_NR: usize = 2;
const K_TN: usize = 3;
const K_NPS: usize = 4;
const K_MNPS: usize = 5;
const K_PLPH: usize = 6;
const K_MPLPH: usize = 7;
const K_TIN: usize = 8;
const K_CURRSEC: usize = 9;
const K_CURRTIME: usize = 10;
const K_CMILTIME: usize = 11;
const K_CFRTIME: usize = 12;
const K_TOTALSEC: usize = 13;
const K_TOTALTIME: usize = 14;
const K_TMILTIME: usize = 15;
const K_TFRTIME: usize = 16;
const K_REMSEC: usize = 17;
const K_REMTIME: usize = 18;
const K_RMILTIME: usize = 19;
const K_RFRTIME: usize = 20;
const K_CURRTICKS: usize = 21;
const K_TOTALTICKS: usize = 22;
const K_REMTICKS: usize = 23;
const K_CURRBARS: usize = 24;
const K_TOTALBARS: usize = 25;
const K_REMBARS: usize = 26;
const K_PPQ: usize = 27;
const K_AVGNPS: usize = 28;
const K_CURRFRAMES: usize = 29;
const K_TOTALFRAMES: usize = 30;
const K_REMFRAMES: usize = 31;
const K_TSN: usize = 32;
const K_TSD: usize = 33;
const K_NOTEP: usize = 34;
const K_TICKP: usize = 35;
const K_TIMEP: usize = 36;

/// 模板 key 列表，按长度降序排列。
/// 确保最长匹配优先（如 `{currtime}` 优先于 `{curr}`）。
const SORTED_KEYS: &[(usize, &str)] = &[
    (K_CURRTICKS, "{currticks}"),
    (K_TOTALTICKS, "{totalticks}"),
    (K_CURRFRAMES, "{currframes}"),
    (K_TOTALFRAMES, "{totalframes}"),
    (K_CURRTIME, "{currtime}"),
    (K_CMILTIME, "{cmiltime}"),
    (K_CFRTIME, "{cfrtime}"),
    (K_TOTALTIME, "{totaltime}"),
    (K_TMILTIME, "{tmiltime}"),
    (K_TFRTIME, "{tfrtime}"),
    (K_REMTIME, "{remtime}"),
    (K_RMILTIME, "{rmiltime}"),
    (K_RFRTIME, "{rfrtime}"),
    (K_REMFRAMES, "{remframes}"),
    (K_TOTALBARS, "{totalbars}"),
    (K_CURRBARS, "{currbars}"),
    (K_TOTALSEC, "{totalsec}"),
    (K_CURRSEC, "{currsec}"),
    (K_REMSEC, "{remsec}"),
    (K_REMTICKS, "{remticks}"),
    (K_REMBARS, "{rembars}"),
    (K_MPLPH, "{mplph}"),
    (K_AVGNPS, "{avgnps}"),
    (K_PLPH, "{plph}"),
    (K_MNPS, "{mnps}"),
    (K_NOTEP, "{notep}"),
    (K_TICKP, "{tickp}"),
    (K_TIMEP, "{timep}"),
    (K_BPM, "{bpm}"),
    (K_TIN, "{tin}"),
    (K_TSD, "{tsd}"),
    (K_TSN, "{tsn}"),
    (K_NPS, "{nps}"),
    (K_PPQ, "{ppq}"),
    (K_TN, "{tn}"),
    (K_NR, "{nr}"),
    (K_NC, "{nc}"),
];

/// 渲染模板字符串，将 `{key}` 替换为对应值。
///
/// 使用单遍扫描，只对模板中实际出现的变量执行格式化。
/// 替换键按长度降序匹配，避免短键干扰长键（如 `{currtime}` 优先于 `{curr}`）。
pub fn render(template: &str, vars: &TemplateVars, cfg: &FormatConfig) -> String {
    let sep = match cfg.separator {
        Separator::Comma => ",",
        Separator::Dot => ".",
        Separator::Nothing => "",
    };

    let fmt_int = |v: u64, pad: usize| -> String {
        let s = if cfg.zero_padding {
            format!("{:0>pad$}", v)
        } else {
            v.to_string()
        };
        if sep.is_empty() || v < 1000 {
            return s;
        }
        let mut out = String::new();
        let digits: Vec<char> = s.chars().collect();
        let mut count = 0;
        for (i, ch) in digits.iter().enumerate().rev() {
            if count == 3 && i != digits.len() - 1 {
                out.push(sep.chars().next().unwrap());
                count = 0;
            }
            out.push(*ch);
            if ch.is_ascii_digit() {
                count += 1;
            }
        }
        out.chars().rev().collect()
    };

    let fmt_f = |v: f64, pad_int: usize, pad_dec: usize| -> String {
        let int_part = v.trunc() as i64;
        let dec_part = ((v.fract().abs()) * 10f64.powi(pad_dec as i32)).round() as u64;
        let int_str = if cfg.zero_padding {
            format!("{:0>pad_int$}", int_part.abs())
        } else {
            int_part.abs().to_string()
        };
        let dec_str = format!("{:0>pad_dec$}", dec_part);
        let sign = if int_part < 0 { "-" } else { "" };
        if sep.is_empty() || int_part.abs() < 1000 {
            format!("{}{}.{}", sign, int_str, dec_str)
        } else {
            let mut out = String::new();
            let digits: Vec<char> = int_str.chars().collect();
            let mut count = 0;
            for (i, ch) in digits.iter().enumerate().rev() {
                if count == 3 && i != digits.len() - 1 {
                    out.push(sep.chars().next().unwrap());
                    count = 0;
                }
                out.push(*ch);
                count += 1;
            }
            let int_with_sep: String = out.chars().rev().collect();
            format!("{}{}.{}", sign, int_with_sep, dec_str)
        }
    };

    // 单遍扫描：逐字符查找 `{`，尝试按长度降序匹配已知 key
    let mut result = String::with_capacity(template.len());
    let mut scan = 0;
    let template_bytes = template.as_bytes();

    while scan < template.len() {
        if template_bytes[scan] == b'{' {
            let mut matched = false;
            for &(idx, key) in SORTED_KEYS.iter() {
                let key_len = key.len();
                if scan + key_len <= template.len()
                    && template[scan..scan + key_len].eq(key)
                {
                    let value = match idx {
                        0 => fmt_f(vars.bpm, cfg.bpm_int_pad, cfg.bpm_dec_pad),
                        1 => fmt_int(vars.note_count, cfg.note_count_pad),
                        2 => fmt_int(vars.notes_remaining, cfg.note_count_pad),
                        3 => fmt_int(vars.total_notes, cfg.note_count_pad),
                        4 => fmt_int(vars.nps, cfg.nps_pad),
                        5 => fmt_int(vars.max_nps, cfg.nps_pad),
                        6 => fmt_int(vars.polyphony, cfg.polyphony_pad),
                        7 => fmt_int(vars.max_polyphony, cfg.polyphony_pad),
                        8 => fmt_int(vars.total_instances, cfg.note_count_pad),
                        9 => fmt_f(vars.curr_sec, 1, 1),
                        10 => vars.curr_time.clone().unwrap_or_else(|| format_time_mmss(vars.curr_sec)),
                        11 => vars.cmil_time.clone().unwrap_or_else(|| format_time_mmss_millis(vars.curr_sec)),
                        12 => vars.cfr_time.clone().unwrap_or_else(|| {
                            let frame = if vars.fps > 0 { vars.curr_frames % vars.fps as u64 } else { 0 };
                            format_time_mmss_frame(vars.curr_sec, frame, vars.fps)
                        }),
                        13 => fmt_f(vars.total_sec, 1, 1),
                        14 => vars.total_time.clone().unwrap_or_else(|| format_time_mmss(vars.total_sec)),
                        15 => vars.tmil_time.clone().unwrap_or_else(|| format_time_mmss_millis(vars.total_sec)),
                        16 => vars.tfr_time.clone().unwrap_or_else(|| {
                            let frame = if vars.fps > 0 { vars.total_frames % vars.fps as u64 } else { 0 };
                            format_time_mmss_frame(vars.total_sec, frame, vars.fps)
                        }),
                        17 => fmt_f(vars.rem_sec, 1, 1),
                        18 => vars.rem_time.clone().unwrap_or_else(|| format_time_mmss(vars.rem_sec)),
                        19 => vars.rmil_time.clone().unwrap_or_else(|| format_time_mmss_millis(vars.rem_sec)),
                        20 => vars.rfr_time.clone().unwrap_or_else(|| {
                            let fps = vars.fps as u64;
                            let frame = if fps > 0 { (vars.total_frames - vars.curr_frames + fps) % fps } else { 0 };
                            format_time_mmss_frame(vars.rem_sec, frame, vars.fps)
                        }),
                        21 => fmt_int(vars.curr_ticks, cfg.ticks_pad),
                        22 => fmt_int(vars.total_ticks, cfg.ticks_pad),
                        23 => fmt_int(vars.rem_ticks, cfg.ticks_pad),
                        24 => fmt_int(vars.curr_bars, cfg.bars_pad),
                        25 => fmt_int(vars.total_bars, cfg.bars_pad),
                        26 => fmt_int(vars.rem_bars, cfg.bars_pad),
                        27 => vars.ppq.to_string(),
                        28 => format!("{:.2}", vars.avg_nps),
                        29 => fmt_int(vars.curr_frames, cfg.frames_pad),
                        30 => fmt_int(vars.total_frames, cfg.frames_pad),
                        31 => fmt_int(vars.rem_frames, cfg.frames_pad),
                        32 => vars.time_sig_num.to_string(),
                        33 => vars.time_sig_den.to_string(),
                        34 => format!("{:.4}", vars.note_percent),
                        35 => format!("{:.4}", vars.tick_percent),
                        36 => format!("{:.4}", vars.time_percent),
                        _ => String::new(),
                    };
                    result.push_str(&value);
                    scan += key_len;
                    matched = true;
                    break;
                }
            }
            if !matched {
                result.push('{');
                scan += 1;
            }
        } else {
            result.push(template[scan..].chars().next().unwrap());
            scan += 1;
        }
    }
    result
}

/// 将秒数格式化为 `mm:ss`。
pub fn format_time_mmss(seconds: f64) -> String {
    let total_secs = seconds.max(0.0) as u64;
    let mins = total_secs / 60;
    let secs = total_secs % 60;
    format!("{:02}:{:02}", mins, secs)
}

/// 将秒数格式化为 `mm:ss.fff`。
pub fn format_time_mmss_millis(seconds: f64) -> String {
    let total_secs = seconds.max(0.0);
    let mins = (total_secs as u64) / 60;
    let secs = (total_secs as u64) % 60;
    let millis = ((total_secs.fract()) * 1000.0).round() as u64;
    format!("{:02}:{:02}.{:03}", mins, secs, millis)
}

/// 将秒数格式化为 `mm:ss;frame`。
pub fn format_time_mmss_frame(seconds: f64, frame: u64, fps: u32) -> String {
    let total_secs = seconds.max(0.0) as u64;
    let mins = total_secs / 60;
    let secs = total_secs % 60;
    let fps_digits = fps.to_string().len();
    format!(
        "{:02}:{:02};{:0>width$}",
        mins,
        secs,
        frame,
        width = fps_digits
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_render_basic() {
        let mut vars = TemplateVars::default();
        vars.note_count = 1234;
        vars.total_notes = 5000;
        vars.bpm = 120.5;
        let cfg = FormatConfig::default();
        let out = render("Notes: {nc} / {tn}\nBPM: {bpm}", &vars, &cfg);
        assert!(out.contains("Notes: 1,234 / 5,000"));
        assert!(out.contains("BPM: 120.50"));
    }

    #[test]
    fn test_render_dot_separator() {
        let mut vars = TemplateVars::default();
        vars.note_count = 1234;
        let mut cfg = FormatConfig::default();
        cfg.separator = Separator::Dot;
        let out = render("{nc}", &vars, &cfg);
        assert_eq!(out, "1.234");
    }

    #[test]
    fn test_render_zero_padding() {
        let mut vars = TemplateVars::default();
        vars.note_count = 42;
        let mut cfg = FormatConfig::default();
        cfg.zero_padding = true;
        let out = render("{nc}", &vars, &cfg);
        assert_eq!(out, "00042");
    }

    #[test]
    fn test_format_time() {
        assert_eq!(format_time_mmss(65.5), "01:05");
        assert_eq!(format_time_mmss_millis(65.123), "01:05.123");
        assert_eq!(format_time_mmss_frame(65.0, 12, 60), "01:05;12");
    }
}
