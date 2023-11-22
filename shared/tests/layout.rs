use hecs::World;
use shared::instrument::{keyboard, string, Config, Layout, LayoutRoot};

#[test]
fn creates_layout_from_config() {
    let config = Config::new(430.0, 932.0, 476.0, Default::default());
    let mut world = World::new();

    let inbound = string::InboundString::spawn(&mut world, &config);
    let outbound = string::OutboundString::spawn(&mut world, &config);
    let keyboard = keyboard::Keyboard::spawn(&mut world, &config);

    let root = LayoutRoot::spawn(&mut world, inbound, outbound, keyboard);
    let layout = Layout::new(&world, &root).expect("failed to create layout");

    insta::assert_yaml_snapshot!(layout)
}
