# Red Siren Feature Inventory + BOM

Date: 2026-04-20
Scope: Implemented features only (frontend, backend, audio runtime/DSP, shared contracts, build and platform config)

## Executive Summary

Red Siren is a cross-platform Tauri + Leptos instrument application with a native audio runtime, a FunDSP signal graph, tuner and editor tooling, and a command/event contract shared between frontend and backend crates.

Current implementation includes:
- A complete route-based frontend shell with player, tuner, about/donate, permissions, and devtools pages.
- A CPAL-backed audio runtime with quality adaptation, telemetry, and snapshot/snoop data surfaces.
- Shared command/event/type contracts in common for instrument, tuner, setup, health, navigation, and intro flows.
- Tauri backend state modules for setup, health, instrument, tuner, intro, persistence, and window lifecycle.
- Trunk + Tauri build wiring for desktop and mobile targets.

## Layer Inventory

| Layer | Feature Band | Status | Key Files |
|---|---|---|---|
| Frontend | Routes and page shell | Implemented | crates/frontend/src/app.rs, crates/frontend/src/routes.rs, crates/frontend/src/pages |
| Frontend | Instrument UI and controls | Implemented | crates/frontend/src/components/instrument |
| Frontend | Tuner UI and sensor controls | Implemented | crates/frontend/src/components/tuner |
| Frontend | Shared UI controls | Implemented | crates/frontend/src/components/button.rs, crates/frontend/src/components/range_slider.rs, crates/frontend/src/components/switch.rs |
| Frontend | Setup/playback/tuner utility services | Implemented | crates/frontend/src/util |
| Audio System | Runtime trait and controller selection | Implemented | crates/audio-system/src/rt/mod.rs |
| Audio System | Native CPAL backend | Implemented | crates/audio-system/src/rt/cpal |
| Audio System | Web runtime placeholder | Partial (stub) | crates/audio-system/src/rt/web/mod.rs |
| Audio System | DSP graph and node pipeline | Implemented | crates/audio-system/src/system/output.rs |
| Audio System | Input analysis and excitation | Implemented | crates/audio-system/src/system/input, crates/audio-system/src/system/excitor |
| Audio System | Quality gates and telemetry | Implemented | crates/audio-system/src/quality.rs, crates/audio-system/src/rt/telemetry.rs, crates/audio-system/src/rt/gate_manager.rs |
| Common | Commands and payloads | Implemented | crates/common/src/commands |
| Common | Events and payloads | Implemented | crates/common/src/events |
| Common | Shared instrument/tuner/navigation types | Implemented | crates/common/src/instrument, crates/common/src/tuner, crates/common/src/navigation |
| Backend (Tauri) | Command handlers and module state | Implemented | src-tauri/src/lib.rs, src-tauri/src/instrument, src-tauri/src/tuner, src-tauri/src/setup, src-tauri/src/health.rs |
| Backend (Tauri) | Intro engine and frame API | Implemented | src-tauri/src/intro |
| Backend (Tauri) | Persistence helpers and keys | Implemented | src-tauri/src/persistence.rs |
| Build | Trunk and Tauri configuration | Implemented | Trunk.toml, src-tauri/tauri.conf.json, src-tauri/tauri.*.conf.json |

## Frontend BOM

### Routes and Pages

| Route | Page | Notes |
|---|---|---|
| / | Home | Main menu with animated card and route entry points |
| /play | Play | Instrument interaction, playback controls, quality and excitement toggles |
| /tune | Tune | Spectrum/tuner screen with sensor controls and probe toggles |
| /about | About | Project and external links |
| /donate | Donate | Donation screen |
| /permissions | Permissions | Microphone permission prompt/flow |
| /edit | Edit (parent) | Devtools-only editor shell with tab routing |
| /edit/layout | EditLayout | Devtools-only layout editor |
| /edit/finetuned-values | EditFineTunedValues | Devtools-only DSP parameter editor |
| /test-node | TestNode | Devtools-only node excitation and snoop inspection |

### Core UI Components

| Component Band | Implemented Pieces |
|---|---|
| Instrument | instrument/mod.rs, keyboard.rs, element.rs, strings.rs, spectrum.rs, debug.rs, context.rs |
| Tuner | tuner/mod.rs, spectrum.rs, sensor_handles.rs, context.rs |
| Menus/Layout | menu/mod.rs, content_page.rs, card.rs, routed_tabs.rs |
| Controls | button.rs, range_slider.rs, switch.rs, icon.rs |
| App toggles | appearance_toggle.rs, excitement_source_toggle.rs, playback_quality_switch.rs |
| UX infra | notifications.rs, toaster.rs, tooltip.rs, intro/mod.rs |
| Editor | editor/layout.rs, editor/finetuned_values.rs, editor/test_node.rs |

### Frontend Utility Services

| Utility | Responsibility |
|---|---|
| boot_flags.rs | Boot configuration flags from injected window data |
| setup_context.rs | Setup state access (mic permission, devtools, readiness) |
| playback_service.rs | Playback command callbacks and shared command lifecycle |
| tauri_resource.rs | invoke + event subscription resource wrapper |
| layout_context.rs | Reactive layout/orientation/safe-area context |
| channel.rs | Typed channel helpers for Tauri interop |
| drawing.rs | Canvas drawing/theming utilities |
| view_transitions.rs | View transition class toggling |
| secondary_window.rs | Secondary window detection |
| raf_fn_fps.rs | RAF callback pacing |
| log.rs | Tauri logging hook |

## Audio Runtime + DSP BOM

### Runtime Interfaces and Backends

| Area | Implemented |
|---|---|
| AudioRuntime trait | Full lifecycle, control, snapshot, tuner, quality, preset, editor (gated) methods |
| CPAL backend | Input/output stream threads, callback processing, quality-aware config selection |
| Runtime subsystem | DSP net ownership, graph rewire/update, shared controls, snoops |
| Telemetry | Processing mode metrics, timing and latency estimates, quality decision signal |
| Quality gate manager | Auto quality adaptation for auto mode, manual override respected |
| Web backend | Null placeholder controller (non-functional parity target) |

### Quality Ladder

| Gate | Sample Type | Sample Rate |
|---|---|---|
| LoFi | f32 | 32000 |
| Medium | f32 | 44100 |
| HiFi | f64 | 48000 |
| Ultra | f64 | 96000 |

### DSP Node/Signal Chain Materials

| Scope | Materials |
|---|---|
| Per key node | SirenWithInputs, SourceOscillator, FormantBank, BellFilter, key/band shared controls |
| Band processing | Metro grid/cadastre timing, delay/cross-talk routing, group-level shaping |
| Channel processing | HP/BP/LP branches, shelf EQ, reverb, chorus, cross-talk blend, DC block |
| System outputs | mono_system, stereo_system, multi_channel_system factories |
| Input analysis | FFTAnalyzer, preamp chain, NewYork compressor, input ADSR smoothing |
| Excitation | Entropy excitation, mic-driven excitation, manual node hit/release path |

### Snoop/Data Tap Materials

| Snoop Surface | Data |
|---|---|
| String output snoops | Per-node output sample vectors |
| Excitement snoops | Per-node complex (real, imag) vectors |
| Processed output spectrum | Frequency/magnitude snapshots |
| Input snoop | Preprocessed input sample vectors |

## Shared Contract BOM (common)

### Commands by Domain

| Domain | Scope |
|---|---|
| setup | Window appearance/size, safe-area apply, new window, go back |
| health | GUI ready, mic permission request, setup state query |
| instrument | Playback lifecycle, excitement source, layout, controls, quality, snoops, spectra |
| tuner | Config/layout/spectrum, sensor updates, range/threshold/wet, stream/probe, input snoop |
| edit (devtools) | Fine-tuned values get/set, test node hit/release |
| intro | Intro pause/resume/frame pull |

### Events by Domain

| Domain | Scope |
|---|---|
| health | Setup state, notifications |
| setup | Appearance and window updates, safe-area event |
| instrument | Playback state, excitement source, layout, band/key reflect events |
| tuner | Config/layout/spectrum/constraints updates |
| navigation | requested/gated/started/committed/completed/canceled/sync |
| intro | Intro animation ready lifecycle event |

### Shared Type Families

| Family | Materials |
|---|---|
| Instrument | Config, BandConfig, NodeConfig, Layout, Preset, Scale, BandChannel, PlaybackQuality |
| Tuner | Config, SensorData, Layout, SpectrumData and update payloads |
| Navigation | RouteId, EditorRouteId and navigation payload types |
| Geometry/System | NodeKey, NodeKeyRegistry, SafeArea, LayoutOrientation, DeviceData, geometry aliases |

## Tauri Backend BOM

### Managed Module State

| Module | Managed State |
|---|---|
| setup | WindowState (dark, override, size, ui/system safe-area) |
| health | SetupState (gui_ready, initial_mic_permission) |
| instrument | InstrumentState + Inner (playing/source/layout/config/resize lock/controller) |
| tuner | TunerState (config, layout, hold/current magnitudes/excitements, stream/probe state) |
| intro | IntroEngineState (engine thread control, snoops, paused/started flags) |

### Plugin Materials

| Plugin | Purpose |
|---|---|
| tauri-plugin-window-state | Persist desktop window geometry/state |
| tauri-plugin-store | JSON-backed key/value persistence |
| tauri-plugin-log | Runtime logging bridge |
| tauri-plugin-opener | Open external links/targets |
| tauri-plugin-safe-area-insets-css | CSS-safe-area support |

### Persistence Materials

| Store | Keys |
|---|---|
| health.json | initial_mic_permission (legacy migration from mic_permission) |
| setup.json | dark_override |
| instrument.json | presets, input_device, output_device, resize_lock, tuner_config |

## Command -> State Wiring Snapshot

| Command Domain | Primary Backend State Mutations |
|---|---|
| setup | WindowState fields and layout resizing flow |
| health | SetupState gui_ready/mic permission and setup-state event emission |
| instrument | InstrumentState playback/source/layout/control/preset/quality fields and runtime controller calls |
| tuner | TunerState config/sensors/constraints/stream/probe and runtime tuner update calls |
| intro | Intro engine control channel and frame snapshot generation |

## Devtools-Only Inventory

### Frontend (runtime/devtools flag)
- Editor page shell and routed editor tabs (/edit/*)
- Test node page (/test-node)
- Debug overlays and advanced instrumentation surfaces in instrument UI

### Backend (compile-time devtools feature)
- Devtools commands in src-tauri/src/instrument/commands.rs:
  - instrument_edit_finetuned_values
  - instrument_get_finetuned_values
  - test_node_hit
  - test_node_release
- Devtools invoke registrations in src-tauri/src/lib.rs are feature-gated.
- Devtools state/runtime entry points in src-tauri/src/instrument/state.rs are feature-gated.

## Platform and Build Matrix

| Concern | Implemented |
|---|---|
| Frontend dev server | trunk serve on localhost:1420 |
| Frontend release build | trunk build --release to dist |
| Tauri app build | Uses dist output via tauri.conf.json |
| Platform config variants | macOS, iOS, Android, Windows, Linux tauri.*.conf.json files present |
| Capabilities | src-tauri/capabilities/default.json and desktop.json |

## Known Partial/Stubbed Areas

| Area | Status |
|---|---|
| Web audio runtime (rt_web) | Placeholder NullController implementation only |
| Full CPAL parity on web | Not implemented yet |

No todo!/unimplemented! markers were found in workspace Rust sources during this audit pass.

## Verification Checklist (Completed)

- Route inventory aligned with frontend route definitions and page implementations.
- Command/event families aligned across common and Tauri invoke/event surfaces.
- Devtools feature-gated backend surfaces enumerated from cfg(feature = "devtools") matches.
- Audio runtime surfaces include trait, CPAL backend, subsystem, telemetry, quality gates, and DSP materials.
- Build and platform BOM includes trunk/tauri/capabilities artifacts.