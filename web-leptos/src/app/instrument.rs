use leptos::*;
use shared::{
    geometry::{Line, Rect},
    instrument,
};

#[component]
pub fn StringComponent(#[prop(into)] layout_line: Signal<Line>) -> impl IntoView {
    let start = move || {
        let p0 = layout_line().p0();
        format!("M {},{}", p0.x, p0.y)
    };

    let end = move || {
        let p1 = layout_line().p1();
        format!("L {},{}", p1.x, p1.y)
    };
    let d = move || format!("{} {}", start(), end());

    view! {
      <path d={d} />
    }
}

#[component]
pub fn ButtonComponent(#[prop(into)] layout_rect: Signal<Rect>) -> impl IntoView {
    let r = move || layout_rect().width() / 2.0;
    let cx = move || layout_rect().center().x;
    let cy = move || layout_rect().center().y;

    view! {
      <circle r=r cx=cx cy=cy />
    }
}

#[component]
pub fn TrackComponent(#[prop(into)] layout_rect: Signal<Rect>) -> impl IntoView {
    let r = move || layout_rect().width().min(layout_rect().height()) / 2.0;
    let p0 = move || layout_rect().top_left();
    let width = move || layout_rect().width();
    let height = move || layout_rect().height();

    view! {
      <rect x={move||p0().x} y={move||p0().y} width=width height=height rx=r ry=r/>
    }
}

#[component]
pub fn InstrumentComponent(
    vm: Signal<instrument::InstrumentVM>,
    ev: SignalSetter<instrument::InstrumentEV>,
) -> impl IntoView {
    let view_box = move || {
        let vb = vm().view_box;
        format!(
            "{} {} {} {}",
            vb.top_left().x,
            vb.top_left().y,
            vb.bottom_right().x,
            vb.bottom_right().y
        )
    };

    let inbound_layout_line = Signal::derive(move || vm().layout.inbound);
    let outbound_layout_line = Signal::derive(move || vm().layout.outbound);

    view! {
      <div class="h-full w-full bg-red dark:bg-black splash">
        <svg fill="none" class="flute stroke-black dark:stroke-red" viewBox={view_box} xmlns="http://www.w3.org/2000/svg">
          <StringComponent layout_line={inbound_layout_line} />
          <StringComponent layout_line={outbound_layout_line} />
        </svg>
        <svg class="tracks fill-red dark:fill-black stroke-black dark:stroke-red" viewBox={view_box} xmlns="http://www.w3.org/2000/svg">
          {move || vm().layout.tracks.into_iter().map(|rect|
            view!{
              <TrackComponent layout_rect={Signal::derive(move || rect)}/>
            }
          ).collect_view()}
        </svg>
        <svg class="sun fill-black dark:fill-red" viewBox={view_box} xmlns="http://www.w3.org/2000/svg">
          {move || vm().layout.buttons.into_iter().map(|rect|
            view!{
              <ButtonComponent layout_rect={Signal::derive(move || rect)}/>
            }
          ).collect_view()}
        </svg>
      </div>
    }
}

// #[component]
// fn TracksComponent(
//   vm: Signal<intro::ViewModel>,
// ) {
//   let whole_groups = vm().groups.floor() as u64;
//   let whole_buttons = vm().buttons_group.floor() as u64;
//   let track_size = vm().track_size;
//   let track_radius = vm().track_radius;
//   let mut tracks_offset = vm().tracks_offset;
//   let button_gap = move || vm().button_gap;
//   let button_group_gap = move || vm().button_group_gap;
//   let button_size = move || vm().button_size;

//   let mut groups = Vec::new();

//   for i in 1..=whole_groups {
//     let mut tracks = Vec::new();
//     let groups_before = (i - 1) as f64;
//     let previous_groups_size =
//         groups_before * (whole_buttons as f64) * (button_size() + button_gap())
//             + (groups_before) * button_group_gap();

//     for j in 1..=whole_buttons {
//         let transform = move || {
//             format!(
//                 "translate({} {})",
//                 tracks_offset - 0.25 * button_gap(),
//                 previous_groups_size + (j - 1) as f64 * (button_size() + button_gap()) - 0.25 * button_gap(),
//             )
//         };
//         tracks.push(view! {
//           <rect
//              width={move || track_size.x + track_radius}
//              height={track_size.y}
//              transform={transform}
//              rx={track_radius}
//              ry={track_radius}
//           />
//         });
//         tracks_offset = (tracks_offset - track_size.x).max(0.0);
//     }

//     let transform = if i % 2 ==0 {
//       format!("scale(-1, 1) translate({} 0)", -1.0 * button_size())
//     } else {
//       format!("scale(1, 1)")
//     };

//     groups.push(view! {
//       <g transform={transform}>
//         {tracks}
//       </g>
//     })
// }

//   let g_transform = move || {
//       format!(
//           "rotate({} {} {}) translate({} {})",
//           vm().buttons_rotation.z,
//           vm().buttons_rotation.x,
//           vm().buttons_rotation.y,
//           vm().buttons_position.x,
//           vm().buttons_position.y,
//       )
//   };

//   view! {
//     <svg id="tracks"
//       viewBox={move || view_box()}
//       class="fill-red dark:fill-black stroke-black dark:stroke-red"
//       xmlns="http://www.w3.org/2000/svg">
//       <g transform={g_transform}>
//         {groups}
//       </g>
//     </svg>
//   }
// }
