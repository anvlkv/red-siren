use leptos::prelude::*;

const INFO_ICON: &str = include_str!("./icon/info.svg");
const PLAY_ICON: &str = include_str!("./icon/play.svg");
const TUNE_ICON: &str = include_str!("./icon/tune.svg");
const BACK_ICON: &str = include_str!("./icon/back.svg");

#[component]
pub fn Icon(
    #[prop(into)] name: Signal<String>,
    #[prop(optional)] stroke_width: Option<f32>,
) -> impl IntoView {
    view! {
        <i
            class="inline-block"
            style="line-height: 1;"
            inner_html=move || {
                let raw = match name().as_str() {
                    "info" => INFO_ICON,
                    "play" => PLAY_ICON,
                    "tune" => TUNE_ICON,
                    "back" => BACK_ICON,
                    _ => "No such icon",
                };
                decorate_svg(raw, stroke_width)
            }
        />
    }
}

fn remove_attr(tag: &str, name: &str) -> String {
    let mut s = tag.to_string();
    loop {
        let search = format!("{}=", name);
        if let Some(pos) = s.find(&search) {
            let after = pos + search.len();
            let bytes = s.as_bytes();
            if after < s.len() && (bytes[after] as char == '"' || bytes[after] as char == '\'') {
                let quote = bytes[after] as char;
                if let Some(rel_end) = s[after + 1..].find(quote) {
                    let end_pos = after + 1 + rel_end;
                    let pre_space = if pos > 0 && s.as_bytes()[pos - 1].is_ascii_whitespace() {
                        1
                    } else {
                        0
                    };
                    s.replace_range(pos - pre_space..=end_pos, "");
                    continue;
                } else {
                    break;
                }
            } else {
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
        } else {
            break;
        }
    }
    s
}

fn decorate_svg(svg: &str, stroke_width: Option<f32>) -> String {
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

            let sw = stroke_width
                .map(|w| format!(r#" stroke-width="{}""#, w))
                .unwrap_or_default();
            let insertion = format!(
                r#" width="1em" height="1em" fill="currentColor" stroke="currentColor"{} style="display:inline-block;vertical-align:-0.125em;line-height:1;color:currentColor""#,
                sw
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
