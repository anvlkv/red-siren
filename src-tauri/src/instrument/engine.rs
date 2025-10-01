use mint::Vector2;
use parking_lot::Mutex;

#[derive(Default, Debug, Clone, Copy)]
pub enum ActivationSource {
    #[default]
    Entropy,
    Mic,
}

impl From<u8> for ActivationSource {
    fn from(value: u8) -> Self {
        match value {
            0 => Self::Entropy,
            _ => Self::Mic,
        }
    }
}

impl From<ActivationSource> for u8 {
    fn from(value: ActivationSource) -> u8 {
        match value {
            ActivationSource::Entropy => 0,
            ActivationSource::Mic => 1,
        }
    }
}

#[derive(Default)]
pub struct InstrumentEngine {
    pub(super) inner: Inner,
}

#[derive(Default)]
pub(super) struct Inner {
    pub playing: Mutex<bool>,
    pub activation_source: Mutex<ActivationSource>,
    pub layout: Mutex<shared::instrument::Layout>,
    pub config: Mutex<shared::instrument::Config>,
}

impl InstrumentEngine {
    pub fn set_is_dark(&self, is_dark: bool) -> shared::error::Result<()> {
        // Narrow lock scope: first mutate layout & derive new config while holding only layout lock
        let new_config = {
            let mut layout = self.inner.layout.lock();
            layout.scale = if is_dark {
                shared::instrument::Scale::In
            } else {
                shared::instrument::Scale::Yo
            };
            shared::instrument::Config::try_from(*layout)?
        };
        // Now update config under its own lock
        {
            let mut config = self.inner.config.lock();
            *config = new_config;
            log::info!("Created new config for [dark: {is_dark}]: {:#?}", *config);
        }
        Ok(())
    }
    pub fn set_size(&self, width: f64, height: f64) -> shared::error::Result<()> {
        // Rebuild layout & derive config with only layout locked
        let new_config = {
            let mut layout = self.inner.layout.lock();
            let scale = layout.scale;
            *layout = shared::instrument::Layout {
                scale,
                ..shared::instrument::Layout::from_screen_estate(Vector2 {
                    x: width as f32,
                    y: height as f32,
                })
            };
            shared::instrument::Config::try_from(*layout)?
        };
        // Apply new config under its own lock
        {
            let mut config = self.inner.config.lock();
            *config = new_config;
            log::info!(
                "Created new config for [width: {width}, height: {height}]: {:#?}",
                *config
            );
        }
        Ok(())
    }

    pub fn set_safe_area(
        &self,
        top: f32,
        right: f32,
        bottom: f32,
        left: f32,
    ) -> shared::error::Result<()> {
        // Rebuild layout for same space with new safe area, preserving current scale
        let new_config = {
            let mut layout = self.inner.layout.lock();
            let space = layout.space;
            let scale = layout.scale;
            *layout = shared::instrument::Layout::from_screen_estate_with_safe_area(
                space, top, right, bottom, left,
            );
            // Preserve the previously selected scale (appearance-based)
            layout.scale = scale;
            shared::instrument::Config::try_from(*layout)?
        };
        {
            let mut config = self.inner.config.lock();
            *config = new_config;
            log::info!(
                "Updated config after safe area change [top: {top}, right: {right}, bottom: {bottom}, left: {left}]: {:#?}",
                *config
            );
        }
        Ok(())
    }
}
