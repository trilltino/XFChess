fn main() {
    // This will fail but the compiler will suggest the correct path
    let _: bevy::ecs::event::EventWriter<()> = ();
}
