use leptos::{html, prelude::*};

use crate::components::UiSize;

const INFO_ICON: &str = include_str!("./icon/info.svg");
const PLAY_ICON: &str = include_str!("./icon/play.svg");
const TUNE_ICON: &str = include_str!("./icon/tune.svg");
const BACK_ICON: &str = include_str!("./icon/back.svg");
const RESUME_ICON: &str = include_str!("./icon/resume.svg");
const PAUSE_ICON: &str = include_str!("./icon/pause.svg");
const DONATE_ICON: &str = include_str!("./icon/donate.svg");
const MIC_ICON: &str = include_str!("./icon/mic.svg");
const ENTROPY_ICON: &str = include_str!("./icon/entropy.svg");
const RESET_ICON: &str = include_str!("./icon/reset.svg");
const DARK_ICON: &str = include_str!("./icon/dark.svg");
const BRIGHT_ICON: &str = include_str!("./icon/bright.svg");
const SYSTEM_ICON: &str = include_str!("./icon/system.svg");
const PROBE_ICON: &str = include_str!("./icon/probe.svg");
const CANCEL_ICON: &str = include_str!("./icon/cancel.svg");
const CHEVRON_LEFT_ICON: &str = include_str!("./icon/chevron-left.svg");
const CHEVRON_RIGHT_ICON: &str = include_str!("./icon/chevron-right.svg");
const SKULL_ICON: &str = include_str!("./icon/skull.svg");
const OK_ICON: &str = include_str!("./icon/ok.svg");
const WARNING_ICON: &str = include_str!("./icon/warning.svg");
const CUBE_ICON: &str = include_str!("./icon/cube.svg");
const BATCH_ICON: &str = include_str!("./icon/batch.svg");

#[component]
pub fn Icon(
    #[prop(into)] name: Signal<String>,
    #[prop(optional, into)] size: Signal<UiSize>, // Defaults to UiSize::Lg via its Default impl
    #[prop(optional, into)] class: Signal<String>, // Allow caller to append classes
    #[prop(optional, into)] node_ref: NodeRef<html::I>,
) -> impl IntoView {
    // Tailwind size class derived from UiSize
    let size_class = move || match size() {
        UiSize::Sm => "md:text-3xl text-xl",
        UiSize::Md => "md:text-4xl text-2xl",
        UiSize::Lg => "md:text-5xl text-3xl",
    };

    let decorated_svg = Signal::derive(move || {
        let name = name();
        let name = name.as_str();
        let raw = match name {
            "info" => INFO_ICON,
            "play" => PLAY_ICON,
            "tune" => TUNE_ICON,
            "back" => BACK_ICON,
            "resume" => RESUME_ICON,
            "pause" => PAUSE_ICON,
            "donate" => DONATE_ICON,
            "mic" => MIC_ICON,
            "entropy" => ENTROPY_ICON,
            "reset" => RESET_ICON,
            "dark" => DARK_ICON,
            "bright" => BRIGHT_ICON,
            "system" => SYSTEM_ICON,
            "probe" => PROBE_ICON,
            "cancel" => CANCEL_ICON,
            "chevron-left" => CHEVRON_LEFT_ICON,
            "chevron-right" => CHEVRON_RIGHT_ICON,
            "skull" => SKULL_ICON,
            "ok" => OK_ICON,
            "warning" => WARNING_ICON,
            "batch" => BATCH_ICON,
            "cube" => CUBE_ICON,
            i => panic!("No such icon: [{i}]"),
        };
        decorate_svg(raw, size(), name)
    });

    view! {
        <i
            class=move || {
                format!(
                    "inline-flex items-center leading-none {}",
                    {
                        let sc = size_class();
                        let extra = class();
                        if extra.is_empty() { sc.to_string() } else { format!("{sc} {extra}") }
                    },
                )
            }
            style="line-height:1;"
            inner_html=decorated_svg
            node_ref=node_ref
        />
    }
}

/*
    Minimal attribute scrubber.

    We strip conflicting sizing/color attributes from the incoming <svg> tag so
    we can enforce:
        width="1em" height="1em" fill="currentColor" stroke="currentColor"
    This ensures consistency and theming adaptability (e.g. dark mode via text color).
*/
fn remove_attr(tag: &str, name: &str) -> String {
    let mut s = tag.to_string();
    loop {
        let search = format!("{name}=");
        if let Some(pos) = s.find(&search) {
            let after = pos + search.len();
            let bytes = s.as_bytes();
            if after < s.len() && (bytes[after] as char == '"' || bytes[after] as char == '\'') {
                let quote = bytes[after] as char;
                if let Some(rel_end) = s[after + 1..].find(quote) {
                    let end_pos = after + 1 + rel_end;
                    // Remove possible leading space
                    let pre_space = if pos > 0 && s.as_bytes()[pos - 1].is_ascii_whitespace() {
                        1
                    } else {
                        0
                    };
                    s.replace_range(pos - pre_space..=end_pos, "");
                    continue;
                }
            } else {
                // Unquoted value fallback
                let mut i = after;
                while i < s.len() {
                    let c = s.as_bytes()[i] as char;
                    if c.is_ascii_whitespace() || c == '>' {
                        break;
                    }
                    i += 1;
                }
                let pre_space = if pos > 0 && s.as_bytes()[pos - 1].is_ascii_whitespace() {
                    1
                } else {
                    0
                };
                s.replace_range(pos - pre_space..i, "");
                continue;
            }
        }
        break;
    }
    s
}

fn decorate_svg(svg: &str, size: UiSize, name: &str) -> String {
    debug_assert!(
        svg.contains("viewBox=\"0 0 1024 1024\""),
        "incorrect svg viewBox: [{name}]"
    );

    // Determine stroke width based on size - thicker for smaller icons
    let stroke_width = match size {
        UiSize::Sm => "11",
        UiSize::Md => "8",
        UiSize::Lg => "5",
    };

    if let Some(start) = svg.find("<svg") {
        if let Some(rel_end) = svg[start..].find('>') {
            let end = start + rel_end;
            let mut tag = svg[start..=end].to_string();

            for attr in [
                "width",
                "height",
                "fill",
                "stroke",
                "stroke-width",
                "style",
                "color",
            ] {
                tag = remove_attr(&tag, attr);
            }

            let insertion = format!(
                r#" width="1em" height="1em" fill="currentColor" stroke="currentColor" stroke-width="{}" style="display:block;color:currentColor""#,
                stroke_width
            );
            if let Some(open_pos) = tag.find("<svg") {
                let insert_pos = open_pos + 4;
                tag.insert_str(insert_pos, &insertion);
                return format!("{}{}{}", &svg[..start], tag, &svg[end + 1..]);
            }
        }
    }
    svg.to_string()
}
