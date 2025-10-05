<img src="https://r2cdn.perplexity.ai/pplx-full-logo-primary-dark%402x.png" style="height:64px;margin-right:32px"/>

# Animated Router Navigation in Leptos

Leptos provides several powerful approaches to create smooth, animated transitions between routes. Here's a comprehensive guide covering all available methods, from the modern View Transitions API to manual CSS animations.

## Native View Transitions API (Recommended)

The most elegant solution is using Leptos' built-in support for the **View Transitions API**. This modern browser feature provides smooth, automatic transitions between routes with minimal code.[^1]

Simply add the `transition=true` prop to your `<Routes>` component:[^1]

```rust
use leptos::prelude::*;
use leptos_router::components::*;
use leptos_router::path;

#[component]
pub fn App() -> impl IntoView {
    view! {
        <Router>
            <nav class="nav">
                <A href="/" exact=true>"Home"</A>
                <A href="/about">"About"</A>
                <A href="/contact">"Contact"</A>
            </nav>
            <main>
                // Enable View Transitions API for smooth route transitions
                <Routes fallback=|| "Not found." transition=true>
                    <Route path=path!("/") view=Home/>
                    <Route path=path!("/about") view=About/>
                    <Route path=path!("/contact") view=Contact/>
                </Routes>
            </main>
        </Router>
    }
}
```

You can customize the animations using CSS:

```css
::view-transition-old(root) {
    animation: slide-out-left 300ms cubic-bezier(0.4, 0, 0.2, 1);
}

::view-transition-new(root) {
    animation: slide-in-right 300ms cubic-bezier(0.4, 0, 0.2, 1);
}

@keyframes slide-out-left {
    from { transform: translateX(0); }
    to { transform: translateX(-100%); }
}

@keyframes slide-in-right {
    from { transform: translateX(100%); }
    to { transform: translateX(0); }
}
```


## Manual CSS Transitions

For more control or older browser support, you can implement animations manually by watching route changes and applying CSS classes:[^2]

```rust
use leptos::prelude::*;
use leptos_router::components::*;
use leptos_router::hooks::use_location;
use leptos_router::path;

#[component]
pub fn AnimatedApp() -> impl IntoView {
    let location = use_location();
    let (animation_class, set_animation_class) = signal("".to_string());

    // Watch for route changes and trigger animations
    create_effect(move |_| {
        let current_path = location.pathname.get();
        // Trigger exit animation first
        set_animation_class.set("page-exit".to_string());
        
        // After a delay, switch to enter animation
        set_timeout(
            move || {
                set_animation_class.set("page-enter".to_string());
                set_timeout(
                    move || set_animation_class.set("".to_string()),
                    std::time::Duration::from_millis(300)
                );
            },
            std::time::Duration::from_millis(300)
        );
    });

    view! {
        <Router>
            <nav class="nav">
                <A href="/" exact=true>"Home"</A>
                <A href="/about">"About"</A>
            </nav>
            <main class=move || format!("main-content {}", animation_class.get())>
                <Routes fallback=|| "Not found.">
                    <Route path=path!("/") view=Home/>
                    <Route path=path!("/about") view=About/>
                </Routes>
            </main>
        </Router>
    }
}
```


## Using AnimatedShow Component

Leptos provides the `AnimatedShow` component for enter/leave animations within individual routes:[^3]

```rust
use leptos::prelude::*;
use leptos_router::components::*;
use leptos_router::hooks::use_params_map;

#[component]
fn AnimatedPage() -> impl IntoView {
    let params = use_params_map();
    let page_id = move || params.read().get("id").unwrap_or("1".to_string());
    let (show_content, set_show_content) = signal(false);

    // Animate in content when component mounts
    create_effect(move |_| {
        set_show_content.set(false);
        set_timeout(
            move || set_show_content.set(true),
            std::time::Duration::from_millis(100)
        );
    });

    view! {
        <div class="animated-page">
            <AnimatedShow
                when=show_content
                show_class="fade-in"
                hide_class="fade-out"
                hide_delay=std::time::Duration::from_millis(300)
            >
                <div class="page-content">
                    <h1>{move || format!("Page {}", page_id())}</h1>
                    <p>"This content animates in and out!"</p>
                </div>
            </AnimatedShow>
        </div>
    }
}
```


## Third-Party Animation Libraries

Several community crates extend Leptos' animation capabilities:

### leptos-transition-group

The `leptos-transition-group` crate provides React-like transition components:[^4]

```toml
[dependencies]
leptos-transition-group = "0.2"
```

```rust
use leptos_transition_group::CSSTransition;

#[component]
pub fn TransitionExample() -> impl IntoView {
    let (show_page, set_show_page) = signal(true);
    
    view! {
        <CSSTransition
            in_prop=show_page
            timeout=300
            class_names="page"
            unmount_on_exit=true
        >
            <div class="page-content">
                "Animated content"
            </div>
        </CSSTransition>
    }
}
```


### leptos-motion

For more advanced animations, `leptos-motion` provides a Framer Motion-inspired API with high-performance WASM-powered animations.[^5]

## Direction-Aware Animations

You can create more sophisticated animations that respond to navigation direction:

```rust
#[component]
pub fn DirectionalRouter() -> impl IntoView {
    let location = use_location();
    let (route_direction, set_route_direction) = signal("forward".to_string());
    let (prev_path, set_prev_path) = signal("/".to_string());
    
    // Route order for determining direction
    let route_order = vec!["/", "/about", "/contact"];
    
    create_effect(move |_| {
        let current_path = location.pathname.get();
        let prev = prev_path.get();
        
        if let (Some(current_idx), Some(prev_idx)) = (
            route_order.iter().position(|&r| r == current_path),
            route_order.iter().position(|&r| r == prev)
        ) {
            if current_idx > prev_idx {
                set_route_direction.set("forward".to_string());
            } else {
                set_route_direction.set("backward".to_string());
            }
        }
        
        set_prev_path.set(current_path);
    });

    view! {
        <Router>
            <main class=move || format!("router-container direction-{}", route_direction.get())>
                <Routes fallback=|| "Not found.">
                    <Route path=path!("/") view=Home/>
                    <Route path=path!("/about") view=About/>
                    <Route path=path!("/contact") view=Contact/>
                </Routes>
            </main>
        </Router>
    }
}
```


## CSS Animation Patterns

Here are essential CSS patterns for smooth route animations:

```css
/* Fade transitions */
@keyframes fade-in {
    from { opacity: 0; transform: translateY(10px); }
    to { opacity: 1; transform: translateY(0); }
}

@keyframes fade-out {
    from { opacity: 1; transform: translateY(0); }
    to { opacity: 0; transform: translateY(-10px); }
}

/* Slide transitions */
@keyframes slide-in-right {
    from { transform: translateX(100%); }
    to { transform: translateX(0); }
}

/* Scale transitions */
.page-wrapper.transitioning {
    transform: scale(0.95);
    opacity: 0.8;
    transition: all 300ms ease-in-out;
}
```


## Best Practices

**Performance**: Use `transform` and `opacity` properties for animations as they don't trigger layout recalculations.[^2]

**Accessibility**: Respect user preferences with `@media (prefers-reduced-motion: reduce)`.[^2]

**Browser Support**: The View Transitions API is modern - provide fallbacks for older browsers.[^6]

**Timing**: Keep animations between 200-500ms for optimal user experience.[^2]

The View Transitions API approach is recommended for modern applications as it provides the smoothest experience with minimal code. For more complex scenarios or broader browser support, combine manual CSS transitions with Leptos' reactive system for full control over your route animations.
<span style="display:none">[^10][^11][^12][^13][^14][^15][^16][^17][^18][^19][^20][^21][^22][^23][^24][^25][^26][^27][^28][^29][^30][^31][^32][^33][^34][^35][^36][^37][^38][^39][^40][^41][^42][^43][^44][^45][^46][^47][^48][^49][^50][^51][^52][^53][^54][^55][^56][^57][^58][^59][^60][^61][^62][^63][^64][^7][^8][^9]</span>

<div align="center">⁂</div>

[^1]: https://docs.rs/leptos_router/latest/leptos_router/components/fn.Routes.html

[^2]: https://developer.mozilla.org/en-US/docs/Web/CSS/CSS_transitions/Using_CSS_transitions

[^3]: https://docs.rs/leptos/latest/leptos/control_flow/fn.AnimatedShow.html

[^4]: https://github.com/thaw-ui/leptos-transition-group

[^5]: https://github.com/cloud-shuttle/leptos-motion

[^6]: https://developer.mozilla.org/en-US/docs/Web/API/View_Transition_API

[^7]: https://book.leptos.dev/router/16_routes.html

[^8]: https://github.com/brofrain/leptos-animated-for

[^9]: https://nextjs.org/docs/app/api-reference/config/next-config-js/viewTransition

[^10]: https://docs.rs/leptos_router/latest/leptos_router/components/fn.RoutingProgress.html

[^11]: https://docs.rs/leptos_animation

[^12]: https://docs.rs/leptos_router/latest/src/leptos_router/components.rs.html

[^13]: https://book.leptos.dev/router/19_a.html

[^14]: https://lib.rs/crates/leptos_transition_group

[^15]: https://www.reddit.com/r/nextjs/comments/1fbwngq/view_transitions_api_page_transitions_package_for/

[^16]: https://github.com/leptos-rs/leptos/issues/1754

[^17]: https://book.leptos.dev/async/12_transition.html

[^18]: https://docs.rs/leptos_router/latest/leptos_router/

[^19]: https://stackoverflow.com/questions/48926912/angular-router-transition-animations-slide-both-left-and-right-conditionally

[^20]: https://leptos-use.rs/elements/use_window.html

[^21]: https://josiahparry.com/posts/2024-10-10-leptos-highlight-js

[^22]: https://github.com/leptos-rs/leptos/issues/1370

[^23]: https://www.reddit.com/r/rust/comments/1i5vcr6/typesafe_frontend_routing_in_rustleptos/

[^24]: https://crates.io/crates/leptos_animate

[^25]: https://angular-doc.ru/guide/route-animations

[^26]: https://www.thisdot.co/blog/making-seamless-page-transitions-with-the-view-transitions-api

[^27]: https://github.com/lpotthast/leptos-routes

[^28]: https://www.youtube.com/watch?v=hpt7SsZUCbs

[^29]: https://github.com/GabrielBarbosaGV/leptos-transition-flip

[^30]: https://github.com/leptos-rs/leptos/discussions/2565

[^31]: https://book.leptos.dev/islands.html

[^32]: https://crates.io/crates/leptos_router

[^33]: https://iodroplet.com/the-view-transitions-api-and-delightful-ui-animations-part-2/

[^34]: https://coursework.vschool.io/react-transitions-with-react-transition-group/

[^35]: https://docs.astro.build/en/reference/modules/astro-transitions/

[^36]: https://reactcommunity.org/react-transition-group/transition/

[^37]: https://docs.rs/leptos-motion-dom/latest/leptos_motion_dom/index.html

[^38]: https://blog.bitsrc.io/animating-reactjs-with-react-transition-group-2af6c87cab0c

[^39]: https://stackoverflow.com/questions/79527617/how-to-set-style-of-a-html-element-in-leptos

[^40]: https://reactcommunity.org/react-transition-group/css-transition/

[^41]: https://stackoverflow.com/questions/65331086/reactjs-csstransition-component-props-in-transitiongroup

[^42]: https://reactdev.ru/libs/react-transition-group/css-transition/

[^43]: https://developer.mozilla.org/en-US/docs/Web/API/View_Transition_API/Using

[^44]: https://javascript.plainenglish.io/4-awesome-examples-of-vue-router-transitions-edfd4db99b6a

[^45]: https://www.joshwcomeau.com/animation/css-transitions/

[^46]: https://book.leptos.dev/view/03_components.html

[^47]: https://www.youtube.com/watch?v=YYlFFMc0RAg

[^48]: https://docs.rs/leptos/latest/leptos/all.html

[^49]: https://stackoverflow.com/questions/78244955/how-to-use-stylance-with-leptos

[^50]: https://book.leptos.dev/metadata.html

[^51]: https://github.com/leptos-rs/leptos/issues/364

[^52]: https://leptos-use.rs/changelog.html

[^53]: https://developer.mozilla.org/en-US/docs/Web/CSS/CSS_animations/Using_CSS_animations

[^54]: https://stackoverflow.com/questions/61089053/animating-route-transitions-with-csstransitiongroup-and-react-router-v6

[^55]: https://www.debugbear.com/blog/view-transitions-spa-without-framework

[^56]: https://stackoverflow.com/questions/64964367/how-to-add-transistion-animation-whenever-the-route-is-changed-in-next-js

[^57]: https://book.leptos.dev/getting_started/

[^58]: https://book.leptos.dev/view/02_dynamic_attributes.html

[^59]: https://stackoverflow.com/questions/63140299/animated-routes-switching-with-data-fetching-vue

[^60]: https://www.freecodecamp.org/news/how-to-use-the-view-transition-api/

[^61]: https://book.leptos.dev/view/04_iteration.html

[^62]: https://habr.com/ru/articles/745708/

[^63]: https://rodneylab.com/trying-out-leptos/

[^64]: https://www.letsbuildui.dev/articles/the-view-transition-api-in-3-examples/

