use leptos::prelude::*;

#[derive(Clone)]
pub struct UseSelectiveSignalReturn<T>
where
    T: Clone + PartialEq + Sync + Send + 'static,
{
    pub value: Memo<T>,
    pub set_source: WriteSignal<T>,
    pub is_active: ReadSignal<bool>,
}

pub fn use_selective_signal<T>(
    initial_value: T,
    predicate: impl Fn(&T) -> bool + Clone + Send + Sync + 'static,
) -> UseSelectiveSignalReturn<T>
where
    T: Clone + PartialEq + Sync + Send + 'static,
{
    let (source, set_source) = signal(initial_value.clone());
    let (last_valid, set_last_valid) = signal(initial_value);
    let (is_active, set_is_active) = signal(false);

    let predicate_clone = predicate.clone();

    // Selector that only triggers when predicate result changes
    let selector = Selector::new(move || predicate_clone(&source.get()));

    // Effect that runs only when the predicate state changes
    Effect::new(move |_| {
        let current_value = source.get();
        let should_be_active = predicate(&current_value);

        log::trace!("selective signal condition is : {should_be_active}");

        if selector.selected(&true) && should_be_active {
            set_last_valid.set(current_value);
            set_is_active.set(true);
        } else if selector.selected(&false) {
            set_is_active.set(false);
        }
    });

    let value = Memo::new(move |_| last_valid.get());

    UseSelectiveSignalReturn {
        value,
        set_source,
        is_active,
    }
}
