use mint::Vector2;
use tokio::sync::Mutex;

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
    pub async fn set_is_dark(&self, is_dark: bool) {
        let mut layout = self.inner.layout.lock().await;
        layout.scale = if is_dark {
            shared::instrument::Scale::In
        } else {
            shared::instrument::Scale::Yo
        };

        let mut config = self.inner.config.lock().await;

        *config = shared::instrument::Config::from(*layout);
        log::info!("Created new config for [dark: {is_dark}]: {:#?}", *config);
    }
    pub async fn set_size(&self, width: f64, height: f64) {
        let mut layout = self.inner.layout.lock().await;
        *layout = shared::instrument::Layout{
            scale: layout.scale,
            ..shared::instrument::Layout::from_screen_estate(Vector2 {x: width as f32, y: height as f32})
        };

        let mut config = self.inner.config.lock().await;

        *config = shared::instrument::Config::from(*layout);
        log::info!("Created new config for [width: {width}, height: {height}]: {:#?}", *config);
    }
}
