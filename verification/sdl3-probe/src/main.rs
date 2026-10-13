use sdl3::audio::{AudioCallback, AudioFormat, AudioSpec, AudioStream};
use sdl3::event::Event;
use sdl3::pixels::Color;

const FRAMES: u32 = 120;

struct Silence;

impl AudioCallback<f32> for Silence {
    fn callback(&mut self, stream: &mut AudioStream, requested: i32) {
        let zeros = vec![0u8; requested.max(0) as usize];
        let _ = stream.put_data(&zeros);
    }
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let sdl = sdl3::init()?;
    let video = sdl.video()?;
    let window = video.window("v1check", 512, 384).build()?;
    let mut canvas = window.into_canvas();

    let gamepad = sdl.gamepad()?;
    let pads = gamepad.gamepads()?;
    println!("gamepads attached: {}", pads.len());

    let audio = sdl.audio()?;
    let spec = AudioSpec { freq: Some(44100), channels: Some(2), format: Some(AudioFormat::f32_sys()) };
    let device = audio.open_playback_device(&spec)?;
    let stream = audio.open_playback_stream_with_callback(&device, &spec, Silence)?;
    stream.resume()?;

    let creator = canvas.texture_creator();
    let mut target = creator.create_texture_target(None, 256, 192)?;
    let mut events = sdl.event_pump()?;

    for frame in 0..FRAMES {
        canvas.with_texture_canvas(&mut target, |c| {
            c.set_draw_color(Color::RGB((frame % 256) as u8, 64, 128));
            c.clear();
        })?;
        canvas.set_draw_color(Color::RGB(0, 0, 0));
        canvas.clear();
        canvas.copy(&target, None, None)?;
        canvas.present();
        for event in events.poll_iter() {
            if let Event::GamepadButtonDown { .. } = event {
                println!("gamepad button");
            }
        }
    }
    println!("ran {FRAMES} frames");
    Ok(())
}
