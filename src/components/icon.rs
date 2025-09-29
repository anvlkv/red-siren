use leptos::prelude::*;

use crate::components::UiSize;

/*
    Icon Component (MAYA DRY KISS)

    Goals:
    - Centralize inline SVG icon rendering.
    - Normalize sizing via UiSize (Sm | Md | Lg) to align with the design system.
    - Strip any width/height/fill/stroke coming from raw SVG assets and replace with scalable, currentColor-driven styling.
    - Remove the previously exposed stroke_width prop (requirement).
    - Keep SVGs scalable via 1em so surrounding font-size (Tailwind text-* utilities) controls visual size.

    Why <i> tag?
    - Semantically neutral and traditionally used for icons.
    - Keeps layout inline by default.

    Size Mapping Rationale:
    - We keep the underlying <svg> at width/height = 1em.
    - We apply a Tailwind text-* utility to the wrapper to control actual pixel size.
    - Mapping chosen to mirror the visual rhythm used in buttons and other components:
        Sm => text-base
        Md => text-2xl
        Lg => text-4xl  (default)

    Future:
    - If we later want pixel-exact sizing independent of font context, we could add a `pixel` mode.
*/

const INFO_ICON: &str = include_str!("./icon/info.svg");
const PLAY_ICON: &str = include_str!("./icon/play.svg");
const TUNE_ICON: &str = include_str!("./icon/tune.svg");
const BACK_ICON: &str = include_str!("./icon/back.svg");
const RESUME_ICON: &str = include_str!("./icon/resume.svg");
const PAUSE_ICON: &str = include_str!("./icon/pause.svg");
const DONATE_ICON: &str = include_str!("./icon/donate.svg");
const MIC_ICON: &str = include_str!("./icon/mic.svg");
const ENTROPY_ICON: &str = include_str!("./icon/entropy.svg");

#[component]
pub fn Icon(
    #[prop(into)] name: Signal<String>,
    #[prop(optional, into)] size: Signal<UiSize>, // Defaults to UiSize::Lg via its Default impl
    #[prop(optional, into)] class: Signal<String>, // Allow caller to append classes
) -> impl IntoView {
    // Tailwind size class derived from UiSize
    let size_class = move || match size() {
        UiSize::Sm => "text-base",
        UiSize::Md => "text-2xl",
        UiSize::Lg => "text-4xl",
    };

    view! {
        <i
            class=move || {
                format!(
                    "inline-block align-middle leading-none {}",
                    {
                        let sc = size_class();
                        let extra = class();
                        if extra.is_empty() { sc.to_string() } else { format!("{sc} {extra}") }
                    },
                )
            }
            style="line-height:1;"
            inner_html=move || {
                let raw = match name().as_str() {
                    "info" => INFO_ICON,
                    "play" => PLAY_ICON,
                    "tune" => TUNE_ICON,
                    "back" => BACK_ICON,
                    "resume" => RESUME_ICON,
                    "pause" => PAUSE_ICON,
                    "donate" => DONATE_ICON,
                    "mic" => MIC_ICON,
                    "entropy" => ENTROPY_ICON,
                    _ => "No such icon",
                };
                decorate_svg(raw)
            }
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

fn decorate_svg(svg: &str) -> String {
    debug_assert!(
        svg.contains(r#"viewBox="0 0 1024 1024""#),
        "incorrect svg viewBox"
    );

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

            let insertion = r#" width="1em" height="1em" fill="currentColor" stroke="currentColor" style="display:inline-block;vertical-align:-0.125em;line-height:1;color:currentColor""#;
            if let Some(open_pos) = tag.find("<svg") {
                let insert_pos = open_pos + 4;
                tag.insert_str(insert_pos, insertion);
                return format!("{}{}{}", &svg[..start], tag, &svg[end + 1..]);
            }
        }
    }
    svg.to_string()
}
