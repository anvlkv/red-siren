# Best Practices for Creating One Signal from Multiple Signals in Leptos

When you need to combine multiple signals to create a derived reactive value for use in effects, Leptos provides several well-established patterns. The key is choosing the right approach based on your specific use case and performance requirements.

## **Recommended Approaches**

### **1. Derived Signals (Most Common)**

The simplest and most efficient approach is to create a **derived signal** - a closure that accesses multiple signals:[1]

```rust
// Multiple source signals
let (first_name, set_first_name) = signal("Bridget".to_string());
let (last_name, set_last_name) = signal("Jones".to_string());

// Derived signal combining both
let full_name = move || format!("{} {}",
    &*first_name.read(),
    &*last_name.read()
);

// Use in an effect
Effect::new(move |_| {
    println!("Full name: {}", full_name());
});
```

**When to use derived signals:**
- For simple, lightweight computations[2]
- When you don't mind the calculation running once per access
- For most common use cases where performance isn't critical

### **2. Memos for Expensive Computations**

When combining multiple signals involves expensive calculations, use a `Memo` instead:[2]

```rust
let (value1, set_value1) = signal(10);
let (value2, set_value2) = signal(20);

// Memo ensures expensive computation runs only once per change
let expensive_combination = Memo::new(move |_| {
    expensive_computation(value1.get(), value2.get())
});

Effect::new(move |_| {
    println!("Result: {}", expensive_combination.get());
});
```

**Key advantages of memos:**
- **Runs only once per change**, regardless of how many times accessed[2]
- **Only notifies dependents if the computed value actually changes**[2]
- **Perfect for expensive computations**[2]

**When to use memos:**
- For computationally expensive derivations
- When the derived value is accessed multiple times
- When you want to avoid redundant calculations

### **3. Signal::derive() for Type-Erased Closures**

You can also use `Signal::derive()` to wrap a derived signal computation:[3]

```rust
let (count, set_count) = signal(2);
let (multiplier, set_multiplier) = signal(3);

let combined = Signal::derive(move || count.get() * multiplier.get());

Effect::new(move |_| {
    println!("Combined value: {}", combined.get());
});
```

## **What NOT to Do**

### **Avoid Using Effects to Synchronize Signals**

Creating an effect to write from one signal to another is officially discouraged:[4][5][1]

```rust
// ❌ DON'T DO THIS
let (a, set_a) = signal(1);
let (b, set_b) = signal(0);

Effect::new(move |_| {
    set_b.set(a.get() * 2); // Creates inefficient reactive chains
});
```

**Why this is problematic:**
- **Less efficient** - triggers two full reactive cycles[1]
- **Risk of infinite loops** and reactive spaghetti code[1]
- **Harder to reason about** data flow[1]

## **Advanced Patterns**

### **Multiple Independent Updates**

When signals are independent but sometimes updated together, update them separately in event handlers:[1]

```rust
let (age, set_age) = signal(32);
let (favorite_number, set_favorite_number) = signal(42);

let clear_handler = move |_| {
    // Update both signals together
    set_age.set(0);
    set_favorite_number.set(0);
};
```

### **Using leptos-use for Complex Synchronization**

For more complex scenarios, consider using `leptos-use` utilities like `sync_signal`, though the Leptos book recommends trying the "Good Options" first:[6]

```rust
use leptos_use::sync_signal;

let (a, set_a) = signal(1);
let (b, set_b) = signal(2);

// Two-way synchronization (use sparingly)
let stop = sync_signal((a, set_a), (b, set_b));
```

## **Performance Considerations**

- **Derived signals** run every time they're accessed and every time dependencies change[7]
- **Memos** cache results and only recalculate when dependencies actually change[2]
- **Memos have overhead** compared to derived signals, so use them only when needed[2]
- **Memos are lazy** - they don't run until first accessed[2]

## **Best Practice Summary**

1. **Start with derived signals** for simple combinations
2. **Use memos** when computations are expensive or accessed frequently
3. **Avoid writing to signals from effects** - prefer top-down data flow
4. **Update multiple independent signals** directly in event handlers
5. **Consider the reactive graph** - clear, predictable data flow is better than clever synchronization

The reactive system in Leptos is designed to minimize effect reruns, so following these patterns ensures your application remains performant and maintainable while leveraging the full power of fine-grained reactivity.[8]

[1](https://book.leptos.dev/reactivity/working_with_signals.html)
[2](https://docs.rs/leptos/latest/leptos/prelude/struct.Memo.html)
[3](https://docs.rs/leptos/latest/leptos/reactive/wrappers/read/struct.Signal.html)
[4](https://book.leptos.dev/reactivity/14_create_effect.html)
[5](https://docs.rs/leptos_reactive/latest/leptos_reactive/fn.create_effect.html)
[6](https://leptos-use.rs/reactivity/sync_signal.html)
[7](https://book.leptos.dev/view/02_dynamic_attributes.html)
[8](https://book.leptos.dev/appendix_reactive_graph.html)
[9](https://github.com/leptos-rs/leptos/discussions/2356)
[10](https://github.com/leptos-rs/leptos/discussions/2552)
[11](https://github.com/leptos-rs/leptos/issues/2041)
[12](https://book.leptos.dev/view/04b_iteration.html)
[13](https://docs.rs/leptos/latest/leptos/reactive/signal/struct.RwSignal.html)
[14](https://docs.rs/leptos/latest/i686-unknown-linux-gnu/leptos/struct.RwSignal.html)
[15](https://book.leptos.dev/reactivity/interlude_functions.html)
[16](https://www.hamzak.xyz/blog-posts/how-to-create-and-usehooks-in-leptos)
[17](https://stackoverflow.com/questions/77034191/why-does-cloning-a-leptos-signal-result-in-updating-the-same-value-multiple-time)
[18](https://book.leptos.dev/appendix_life_cycle.html?highlight=owner)
[19](https://book.leptos.dev/view/08_parent_child.html)
[20](https://www.snoyman.com/blog/manual-leptos/)
[21](https://dev.to/davidedelpapa/leptos-tauri-tutorial-3k60)
[22](https://docs.rs/leptos/latest/leptos/reactive/computed/index.html)
[23](https://github.com/leptos-rs/leptos/discussions/2807)
[24](https://crates.io/crates/reactive-signals)
[25](https://users.rust-lang.org/t/reactive-programming/95503)
[26](https://crates.io/crates/leptos/0.0.15)
[27](https://github.com/leptos-rs/leptos/issues/2357)
[28](https://book.leptos.dev/appendix_life_cycle.html)
[29](https://crates.io/crates/leptos/0.6.0-rc1)
[30](https://github.com/leptos-rs/leptos/issues/1827)
[31](https://book.leptos.dev/view/06_control_flow.html)
[32](https://www.reddit.com/r/rust/comments/zxrkrg/question_of_leptos_web_framework_vs_sycamore/)
[33](https://crates.io/crates/leptos/0.0.13)
