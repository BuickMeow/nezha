use std::collections::HashMap;

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
    pub curr_time: String,
    pub cmil_time: String,
    pub cfr_time: String,
    pub total_sec: f64,
    pub total_time: String,
    pub tmil_time: String,
    pub tfr_time: String,
    pub rem_sec: f64,
    pub rem_time: String,
    pub rmil_time: String,
    pub rfr_time: String,
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
}

/// 渲染模板字符串，将 `{key}` 替换为对应值。
pub fn render(template: &str, vars: &TemplateVars, cfg: &FormatConfig) -> String {
    let mut result = template.to_string();

    // 预计算格式化字符串
    let sep = match cfg.separator {
        Separator::Comma => ",",
        Separator::Dot => ".",
        Separator::Nothing => "",
    };

    let _bpm_fmt = if cfg.zero_padding {
        format!("{:0>width_int$}.{:0>width_dec$}", "", "",
            width_int = cfg.bpm_int_pad,
            width_dec = cfg.bpm_dec_pad)
    } else {
        "0.00".to_string()
    };

    // 辅助：带千位分隔符 + 零填充的整数格式化
    let fmt_int = |v: u64, pad: usize| -> String {
        let s = if cfg.zero_padding {
            format!("{:0>pad$}", v)
        } else {
            v.to_string()
        };
        if sep.is_empty() || v < 1000 {
            return s;
        }
        // 插入千位分隔符
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
            format!("{}{}.{}" , sign, int_str, dec_str)
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

    // 构建替换映射
    let mut map: HashMap<&str, String> = HashMap::new();
    map.insert("{bpm}", fmt_f(vars.bpm, cfg.bpm_int_pad, cfg.bpm_dec_pad));
    map.insert("{nc}", fmt_int(vars.note_count, cfg.note_count_pad));
    map.insert("{nr}", fmt_int(vars.notes_remaining, cfg.note_count_pad));
    map.insert("{tn}", fmt_int(vars.total_notes, cfg.note_count_pad));
    map.insert("{nps}", fmt_int(vars.nps, cfg.nps_pad));
    map.insert("{mnps}", fmt_int(vars.max_nps, cfg.nps_pad));
    map.insert("{plph}", fmt_int(vars.polyphony, cfg.polyphony_pad));
    map.insert("{mplph}", fmt_int(vars.max_polyphony, cfg.polyphony_pad));
    map.insert("{tin}", fmt_int(vars.total_instances, cfg.note_count_pad));

    map.insert("{currsec}", fmt_f(vars.curr_sec, 1, 1));
    map.insert("{currtime}", vars.curr_time.clone());
    map.insert("{cmiltime}", vars.cmil_time.clone());
    map.insert("{cfrtime}", vars.cfr_time.clone());

    map.insert("{totalsec}", fmt_f(vars.total_sec, 1, 1));
    map.insert("{totaltime}", vars.total_time.clone());
    map.insert("{tmiltime}", vars.tmil_time.clone());
    map.insert("{tfrtime}", vars.tfr_time.clone());

    map.insert("{remsec}", fmt_f(vars.rem_sec, 1, 1));
    map.insert("{remtime}", vars.rem_time.clone());
    map.insert("{rmiltime}", vars.rmil_time.clone());
    map.insert("{rfrtime}", vars.rfr_time.clone());

    map.insert("{currticks}", fmt_int(vars.curr_ticks, cfg.ticks_pad));
    map.insert("{totalticks}", fmt_int(vars.total_ticks, cfg.ticks_pad));
    map.insert("{remticks}", fmt_int(vars.rem_ticks, cfg.ticks_pad));

    map.insert("{currbars}", fmt_int(vars.curr_bars, cfg.bars_pad));
    map.insert("{totalbars}", fmt_int(vars.total_bars, cfg.bars_pad));
    map.insert("{rembars}", fmt_int(vars.rem_bars, cfg.bars_pad));

    map.insert("{ppq}", vars.ppq.to_string());
    map.insert("{tsn}", vars.time_sig_num.to_string());
    map.insert("{tsd}", vars.time_sig_den.to_string());
    map.insert("{avgnps}", format!("{:.2}", vars.avg_nps));

    map.insert("{currframes}", fmt_int(vars.curr_frames, cfg.frames_pad));
    map.insert("{totalframes}", fmt_int(vars.total_frames, cfg.frames_pad));
    map.insert("{remframes}", fmt_int(vars.rem_frames, cfg.frames_pad));

    map.insert("{notep}", format!("{:.4}", vars.note_percent));
    map.insert("{tickp}", format!("{:.4}", vars.tick_percent));
    map.insert("{timep}", format!("{:.4}", vars.time_percent));

    // 按占位符长度降序替换，避免短占位符干扰长的
    let mut keys: Vec<&&str> = map.keys().collect();
    keys.sort_by_key(|k| std::cmp::Reverse(k.len()));

    for key in keys {
        result = result.replace(key, &map[key]);
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
    format!("{:02}:{:02};{:0>width$}", mins, secs, frame, width = fps_digits)
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
