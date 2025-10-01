use shared::events::intro::{IntroSnoopBatchPayload, IntroSnoopSample};
use shared::error::{Result, IntroError};
use tauri::State;

use super::engine::{IntroEngineState, Control};

#[tauri::command]
pub async fn intro_pause(state: State<'_, IntroEngineState>) -> Result<()> {
    let mut inner = state
        .inner
        .lock();
    if inner.paused {
        return Ok(());
    }
    if let Some(tx) = &inner.tx {
        tx.send(Control::Pause)
            .map_err(|e| IntroError::PauseFailed { detail: Some(e.to_string()) })?;
        inner.paused = true;
    }
    Ok(())
}

#[tauri::command]
pub async fn intro_resume(state: State<'_, IntroEngineState>) -> Result<()> {
    let mut inner = state
        .inner
        .lock();
    if !inner.paused {
        return Ok(());
    }
    if let Some(tx) = &inner.tx {
        tx.send(Control::Resume)
            .map_err(|e| IntroError::ResumeFailed { detail: Some(e.to_string()) })?;
        inner.paused = false;
    }
    Ok(())
}



// -----------------------------------------------------------------------------
// New on-demand frame command
// -----------------------------------------------------------------------------
#[tauri::command]
pub async fn intro_next_frame(
    state: State<'_, IntroEngineState>,
) -> Result<IntroSnoopBatchPayload> {
    use std::time::{SystemTime, UNIX_EPOCH};

    // Lazy start engine if not running.
    {
        let mut inner = state
            .inner
            .lock();
        if !inner.started {
            let config = super::engine::EngineConfig::default();
            let (tx, handle, snoops, _depths) = super::engine::spawn_engine(inner.paused, config);
            inner.tx = Some(tx);
            inner.join = Some(handle);
            inner.snoops = snoops;
            inner.started = true;
        }
    }

    // Collect snapshot.
    let mut inner = state
        .inner
        .lock();
    if inner.snoops.is_empty() {
        return Err(IntroError::EngineNotReady.into());
    }

    let num = inner.snoops.len();
    let t_unix_ms = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|_| IntroError::FrameTimeError)?
        .as_millis() as u64;

    let mut snoop_payloads = Vec::with_capacity(num);
    for (i, snoop) in inner.snoops.iter_mut().enumerate() {
        snoop.update();
        let cap = snoop.capacity(); // 864
        let stride = super::engine::INTRO_DECIMATION_STRIDE;
        let mut samples = Vec::with_capacity(cap / stride + 2);
        for rev in (0..cap).rev().step_by(stride) {
            samples.push(snoop.at(rev));
        }
        snoop_payloads.push(IntroSnoopSample {
            snoop_id: (i + 1) as u8,
            samples,
        });
    }

    Ok(IntroSnoopBatchPayload {
        t_unix_ms,
        snoops: snoop_payloads,
    })
}
