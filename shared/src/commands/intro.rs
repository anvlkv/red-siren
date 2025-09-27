/*!
Intro (splash) DSP visualization commands.

Refactored to a pull model (no streaming channel):
- Background engine thread continually advances the low-frequency Fundsp graph.
- Frontend requests the latest snoop batch on demand with INTRO_NEXT_FRAME.
- Pause/Resume still toggle engine stepping without destroying graph state.
- No channel plumbing; each call returns a full `IntroSnoopBatchPayload`.

MAYA DRY KISS:
Keep this file limited to string constants; avoid premature abstractions.
*/

/// Request the next intro snoop batch (pull model).
pub const INTRO_NEXT_FRAME: &str = "intro_next_frame";

/// Temporarily pause DSP generation (no events emitted while paused).
pub const INTRO_PAUSE: &str = "intro_pause";

/// Resume DSP generation after a pause.
pub const INTRO_RESUME: &str = "intro_resume";
