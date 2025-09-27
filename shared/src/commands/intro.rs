/*!
Intro (splash) DSP visualization commands.

These commands orchestrate the backend low-frequency Fundsp engine
that generates “snoop” line data for the intro animation.

Design Choices:
- Stream initiation is idempotent. First invoke starts the engine thread.
  Subsequent invokes simply replace the channel sink (e.g. after a UI reload).
- Pause/Resume control emission & internal stepping without tearing down
  the network (phases preserved).
- No separate STOP command for now (engine lives for app lifetime).
  Add one later if lifecycle needs to shrink.

JS Pattern (mirrors the Tauri channel example):
```ts
import { invoke, channel } from '@tauri-apps/api/core';

const onEvent = new channel.Channel<IntroSnoopBatchPayload>();
onEvent.onmessage = (batch) => {
  // batch.tUnixMs, batch.snoops[...]
};

await invoke(INTRO_STREAM, { onEvent });  // payload is `()` on Rust side
// Later:
await invoke(INTRO_PAUSE,  () => {});
await invoke(INTRO_RESUME, () => {});
```

MAYA DRY KISS:
Keep this module limited to string constants; avoid premature abstractions.
*/

/// Start (or reattach to) the intro DSP stream.
/// Expects an argument object containing `onEvent` (Tauri Channel).
pub const INTRO_STREAM: &str = "intro_stream";

/// Temporarily pause DSP generation (no events emitted while paused).
pub const INTRO_PAUSE: &str = "intro_pause";

/// Resume DSP generation after a pause.
pub const INTRO_RESUME: &str = "intro_resume";
