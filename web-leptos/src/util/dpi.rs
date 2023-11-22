use leptos::{create_effect, create_signal, Signal, SignalGet};
use leptos_use::use_media_query;

pub fn use_dpi(mut values: Vec<u16>) -> Signal<u16> {
    values.sort();

    let mut queries = vec![];
    let (dpi, set_dpi) = create_signal(values[0]);

    let mut it = values.iter().peekable();

    while let Some(value) = it.next() {
        let and_max = if let Some(next) = it.peek() {
            format!("and (max-resolution: {}dpi)", **next - 1)
        } else {
            String::default()
        };

        queries.push(use_media_query(format!(
            "(min-resolution: {}dpi){}",
            value, and_max
        )))
    }

    create_effect(move |_| {
        if let Some((dpi, _)) = values.iter().zip(queries.iter()).find(|(_, q)| q.get()) {
            set_dpi(*dpi)
        }
    });

    dpi.into()
}
