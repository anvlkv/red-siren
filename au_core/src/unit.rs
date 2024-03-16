mod model;
mod resolve;

use std::sync::atomic::Ordering;

use ::shared::*;
pub use crux_core::App;
use crux_macros::Effect;
use hecs::Entity;
use serde::{Deserialize, Serialize};

use model::UnitModel;
use resolve::Resolve;

use super::system::*;

pub const FFT_RES: usize = 1024;

#[derive(Default)]
pub struct RedSirenAU;

#[derive(Effect)]
#[effect(app = "RedSirenAU")]
pub struct AUCapabilities {
    pub resolve: Resolve<UnitEvent>,
    pub sys: SystemCapability<UnitEvent>,
}

#[derive(Deserialize, Serialize, Debug, Clone)]
pub enum UnitEvent {
    CreateSystem(u32, u32),
    Ready(bool),
    Resume,
    Suspend,
    SendFFT,
    SendSnoops,
    ButtonPressed(Entity),
    ButtonReleased(Entity),
    Detune(Entity, f32),
    Configure(Vec<NodeData>),
    SetControl(Entity, f32),
    ListenToInput,
    IgnoreInput,
}

impl App for RedSirenAU {
    type Event = UnitEvent;

    type Model = UnitModel;

    type ViewModel = ();

    type Capabilities = AUCapabilities;

    fn update(&self, event: Self::Event, model: &mut Self::Model, caps: &Self::Capabilities) {
        match event {
            UnitEvent::CreateSystem(sample_rate, buffer_size) => {
                let fft_res = Ord::max(FFT_RES, buffer_size as usize);
                let system = System::new(sample_rate);

                model.buffer_size = buffer_size;
                model.sample_rate = sample_rate;
                model.fft_res = fft_res;
                model.input_analyzer_enabled.store(true, Ordering::Release);
                model.system = Some(system);
                caps.resolve.create()
            }
            UnitEvent::SendFFT => {
                if let Some(data) = model.app_au_buffer.read_fft_data() {
                    caps.resolve.fft(data);
                }
            }
            UnitEvent::SendSnoops => {
                if let Some(data) = model.app_au_buffer.read_snoops_data() {
                    caps.resolve.snoops(data);
                }
            }
            UnitEvent::Ready(success) => caps.resolve.run(success),
            UnitEvent::Suspend => {} //caps.streams.playing(UnitEvent::UnitState, false)},
            UnitEvent::Resume => {}  //caps.streams.playing(UnitEvent::UnitState, true)},
            // UnitEvent::UnitState(state) => {
            //     match state {
            //         UnitState::None => caps.resolve.run(false),
            //         UnitState::Init => caps.resolve.run(true),
            //         _ => caps.resolve.update(true),
            //     }

            //     model.state = state;
            // }
            // UnitEvent::InputStreamData(input) => {
            //     caps.streams.process_input(input);
            // }
            ev => self.update_system(ev, model, caps),
        }
    }

    fn view(&self, _model: &Self::Model) -> Self::ViewModel {
        ()
    }
}

const DESIRED_BUFFER_SIZE: u32 = 1024;

impl RedSirenAU {
    fn update_system(&self, ev: UnitEvent, model: &mut UnitModel, caps: &AUCapabilities) {
        let sys = model.system.as_mut().unwrap();

        match ev {
            UnitEvent::ButtonPressed(e) => sys.press(e, true),
            UnitEvent::ButtonReleased(e) => sys.press(e, false),
            UnitEvent::Detune(e, val) => sys.move_f(e, val),
            UnitEvent::SetControl(e, val) => sys.control_node(&e, val),
            UnitEvent::Configure(nodes) => {
                *sys = System::new(model.buffer_size);
                model.snoops = sys.replace_nodes(nodes.into_iter().map(|d| d.into()).collect());
                caps.sys.send_be(sys, UnitEvent::Ready);
            }
            UnitEvent::ListenToInput => {
                model.input_analyzer_enabled.store(true, Ordering::Release);
            }
            UnitEvent::IgnoreInput => {
                model.input_analyzer_enabled.store(false, Ordering::Release);
                for e in sys.nodes.keys() {
                    sys.control_node(e, 0.0);
                }
            }
            _ => unreachable!(),
        };

        caps.resolve.update(true);
    }
}
