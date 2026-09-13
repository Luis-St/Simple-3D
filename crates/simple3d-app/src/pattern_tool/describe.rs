//! What a stage and its variations do, in a line of English.

use simple3d_core::pattern::{self, StageMode, Variation, Vary};
use simple3d_core::unit::Unit;

/// A line of English saying what one stage does, so the numbers under it can be
/// read without working them out.
pub(crate) fn describe(stage: &pattern::Stage, unit: Unit) -> String {
    use simple3d_core::unit::format_number;
    let axis = pattern::VARY_AXES[stage.axis.min(2)];
    if stage.mode == StageMode::Mirror {
        return format!("mirrored across {axis}");
    }
    let length = |mm: f64| length(mm, unit);
    let mut what: Vec<String> = Vec::new();
    if stage.mode == StageMode::Turn {
        if stage.turn.abs() > 1e-9 {
            what.push(format!("turning {} deg about {axis}", format_number(stage.turn, 1)));
        }
        if stage.radius.abs() > 1e-9 || stage.growth.abs() > 1e-9 {
            what.push(format!("at radius {}", length(stage.radius)));
        }
        if stage.rise.abs() > 1e-9 {
            what.push(format!("rising {}", length(stage.rise)));
        }
    } else if stage.step.length() > 1e-9 {
        what.push(format!("{} apart", length(stage.step.length())));
    }
    // What varies the copies, in the same sentence: a rule that staggers its
    // rows has to say so where the rule is read, even with its cards folded.
    for variation in stage.variations().iter().filter(|v| v.acts(stage.mode)) {
        what.push(describe_variation(variation, stage.mode, unit));
    }
    if what.is_empty() {
        what.push("in place".to_string());
    }
    // A blank stage makes exactly one copy, and a line of English reading
    // "1 copies" draws attention to itself rather than to the stage.
    let copies = stage.copies();
    format!("{copies} cop{}, {}", if copies == 1 { "y" } else { "ies" }, what.join(", "))
}

/// A few words saying what one variation does to a stage doing `mode`.
pub(crate) fn describe_variation(variation: &Variation, mode: StageMode, unit: Unit) -> String {
    use simple3d_core::unit::format_number;
    if !variation.what.fits(mode) {
        return "does nothing on a turn".to_string();
    }
    if !variation.acts(mode) {
        return "nothing yet".to_string();
    }
    let length = |mm: f64| length(mm, unit);
    let axis = ["X", "Y", "Z", "all axes"][variation.axis.min(pattern::ALL_AXES)];
    let copies = reach(variation);
    let more = if variation.repeats { "" } else { ", more each time" };
    match variation.what {
        Vary::Shift => format!("shifting {} along {axis} {copies}{more}", length(variation.amount)),
        Vary::Spin => format!("spinning {} deg about {axis} {copies}{more}", format_number(variation.amount, 1)),
        Vary::Size => {
            let along = if variation.axis == pattern::ALL_AXES { String::new() } else { format!(" along {axis}") };
            format!("sized {} %{along} {copies}{more}", format_number(variation.amount * 100.0, 0))
        }
        Vary::Gap => {
            let way = if variation.amount > 0.0 { "widening" } else { "narrowing" };
            format!("gaps {way} {} after {copies}{more}", length(variation.amount.abs()))
        }
    }
}

/// A length with its unit, the way a sentence says it.
pub(crate) fn length(mm: f64, unit: Unit) -> String {
    format!("{} {}", simple3d_core::unit::format_length(mm, unit), unit.suffix())
}

/// Which copies a variation reaches, in words.
fn reach(variation: &Variation) -> String {
    let from = match variation.start {
        1 => " from the original".to_string(),
        2 => String::new(),
        start => format!(" from copy {start}"),
    };
    match variation.every {
        1 if variation.start == 1 => "every copy".to_string(),
        1 if variation.start == 2 => "every copy after the original".to_string(),
        1 => format!("every copy{from}"),
        2 => format!("every other copy{from}"),
        every => format!("every {} copy{from}", ordinal(every)),
    }
}

/// 3rd, 4th, 11th, 22nd.
fn ordinal(n: u32) -> String {
    let suffix = match (n % 10, n % 100) {
        (_, 11..=13) => "th",
        (1, _) => "st",
        (2, _) => "nd",
        (3, _) => "rd",
        _ => "th",
    };
    format!("{n}{suffix}")
}
