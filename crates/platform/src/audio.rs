//! Audio output: an SDL3 playback stream fed by a small software mixer.
//!
//! The mixer ([`Mixer`]) is plain Rust with no SDL types, so it is tested without an audio
//! device. [`AudioOutput`] opens a mono `f32` SDL3 stream whose callback renders the mixer.
//!
//! Sound is placeholder only: each sound effect or BGM ID plays a short generated tone (or
//! silence, see [`Placeholder`]). Real SDAT playback replaces the voice source later (#66);
//! the play/stop API and the volume controls stay the same.

use std::sync::{Arc, Mutex, MutexGuard};

use sdl3::audio::{AudioCallback, AudioFormat, AudioSpec, AudioStream, AudioStreamWithCallback};

use crate::{PlatformError, ctx};

/// Output sample rate in Hz (mono).
pub const SAMPLE_RATE: u32 = 32_000;
/// Most sound effects that sound at once; starting one more drops the oldest.
pub const MAX_SFX_VOICES: usize = 16;
/// Length of a placeholder sound effect, in seconds.
const SFX_SECONDS: f32 = 0.25;
/// Peak level of one voice before volume is applied.
const VOICE_LEVEL: f32 = 0.25;

/// Identifies a sound effect.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct SfxId(pub u16);

/// Identifies a background music track.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct BgmId(pub u16);

/// What a placeholder voice sounds like.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Placeholder {
    /// A triangle tone whose pitch depends on the ID.
    #[default]
    Tones,
    /// Voices run and stop normally but produce no sound.
    Silence,
}

/// Master, BGM and SFX volumes, each clamped to `0.0..=1.0`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Volumes {
    pub master: f32,
    pub bgm: f32,
    pub sfx: f32,
}

impl Default for Volumes {
    fn default() -> Self {
        Volumes {
            master: 1.0,
            bgm: 1.0,
            sfx: 1.0,
        }
    }
}

fn clamp01(v: f32) -> f32 {
    if v.is_nan() { 0.0 } else { v.clamp(0.0, 1.0) }
}

/// One playing placeholder tone.
#[derive(Debug, Clone, PartialEq)]
struct Voice {
    /// Phase in cycles, `0.0..1.0`.
    phase: f32,
    /// Phase step per sample.
    step: f32,
    /// Samples left, or `None` to loop until stopped.
    remaining: Option<u32>,
    /// Total length in samples, for the decay envelope (0 when looping).
    total: u32,
}

impl Voice {
    fn new(freq: f32, sample_rate: u32, seconds: Option<f32>) -> Voice {
        let total = seconds.map_or(0, |s| (s * sample_rate as f32) as u32);
        Voice {
            phase: 0.0,
            step: freq / sample_rate as f32,
            remaining: seconds.map(|_| total),
            total,
        }
    }

    /// Next sample, or `None` when a finite voice has ended.
    fn next(&mut self) -> Option<f32> {
        let mut gain = 1.0;
        if let Some(left) = self.remaining.as_mut() {
            if *left == 0 {
                return None;
            }
            gain = *left as f32 / self.total.max(1) as f32;
            *left -= 1;
        }
        // Triangle wave in -1..1.
        let tri = 4.0 * (self.phase - 0.5).abs() - 1.0;
        self.phase += self.step;
        if self.phase >= 1.0 {
            self.phase -= 1.0;
        }
        Some(tri * gain * VOICE_LEVEL)
    }
}

/// Software mixer: up to [`MAX_SFX_VOICES`] sound effects plus one BGM track.
#[derive(Debug, Clone, PartialEq)]
pub struct Mixer {
    sample_rate: u32,
    placeholder: Placeholder,
    volumes: Volumes,
    bgm: Option<(BgmId, Voice)>,
    sfx: Vec<(SfxId, Voice)>,
}

impl Mixer {
    pub fn new(sample_rate: u32, placeholder: Placeholder) -> Mixer {
        Mixer {
            sample_rate: sample_rate.max(1),
            placeholder,
            volumes: Volumes::default(),
            bgm: None,
            sfx: Vec::new(),
        }
    }

    /// Starts sound effect `id`. Several instances of one ID may sound at once.
    pub fn play_sfx(&mut self, id: SfxId) {
        if self.sfx.len() >= MAX_SFX_VOICES {
            self.sfx.remove(0);
        }
        let freq = 220.0 * 2f32.powf((id.0 % 24) as f32 / 12.0);
        self.sfx
            .push((id, Voice::new(freq, self.sample_rate, Some(SFX_SECONDS))));
    }

    /// Stops every playing instance of sound effect `id`.
    pub fn stop_sfx(&mut self, id: SfxId) {
        self.sfx.retain(|(i, _)| *i != id);
    }

    /// Stops all sound effects.
    pub fn stop_all_sfx(&mut self) {
        self.sfx.clear();
    }

    /// Starts BGM `id` looping, replacing the current track. Restarts it if already playing.
    pub fn play_bgm(&mut self, id: BgmId) {
        let freq = 110.0 * 2f32.powf((id.0 % 24) as f32 / 12.0);
        self.bgm = Some((id, Voice::new(freq, self.sample_rate, None)));
    }

    /// Stops the BGM track.
    pub fn stop_bgm(&mut self) {
        self.bgm = None;
    }

    /// The BGM track currently playing.
    pub fn current_bgm(&self) -> Option<BgmId> {
        self.bgm.as_ref().map(|(id, _)| *id)
    }

    /// Number of sound effects still sounding.
    pub fn active_sfx(&self) -> usize {
        self.sfx.len()
    }

    /// Whether sound effect `id` is sounding.
    pub fn is_sfx_playing(&self, id: SfxId) -> bool {
        self.sfx.iter().any(|(i, _)| *i == id)
    }

    pub fn volumes(&self) -> Volumes {
        self.volumes
    }

    pub fn set_master_volume(&mut self, v: f32) {
        self.volumes.master = clamp01(v);
    }

    pub fn set_bgm_volume(&mut self, v: f32) {
        self.volumes.bgm = clamp01(v);
    }

    pub fn set_sfx_volume(&mut self, v: f32) {
        self.volumes.sfx = clamp01(v);
    }

    /// Overwrites `out` with the next mono samples. Ended sound effects are dropped.
    pub fn render(&mut self, out: &mut [f32]) {
        out.fill(0.0);
        let silent = self.placeholder == Placeholder::Silence;
        let Volumes { master, bgm, sfx } = self.volumes;
        if let Some((_, voice)) = self.bgm.as_mut() {
            for o in out.iter_mut() {
                if let Some(s) = voice.next() {
                    *o += s * bgm;
                }
            }
        }
        for (_, voice) in self.sfx.iter_mut() {
            for o in out.iter_mut() {
                match voice.next() {
                    Some(s) => *o += s * sfx,
                    None => break,
                }
            }
        }
        self.sfx.retain(|(_, v)| v.remaining != Some(0));
        for o in out.iter_mut() {
            *o = if silent {
                0.0
            } else {
                (*o * master).clamp(-1.0, 1.0)
            };
        }
    }
}

fn lock(m: &Mutex<Mixer>) -> MutexGuard<'_, Mixer> {
    // A poisoned mixer is still usable: it holds only plain data.
    m.lock().unwrap_or_else(|e| e.into_inner())
}

struct MixerCallback {
    mixer: Arc<Mutex<Mixer>>,
    buf: Vec<f32>,
}

impl AudioCallback<f32> for MixerCallback {
    fn callback(&mut self, stream: &mut AudioStream, requested: i32) {
        let n = requested.max(0) as usize;
        self.buf.resize(n, 0.0);
        lock(&self.mixer).render(&mut self.buf);
        let _ = stream.put_data_f32(&self.buf);
    }
}

/// An open SDL3 audio stream playing a shared [`Mixer`].
///
/// Dropping it closes the stream.
pub struct AudioOutput {
    mixer: Arc<Mutex<Mixer>>,
    // Field order matters: the stream drops before the subsystem handle it keeps alive.
    _stream: AudioStreamWithCallback<MixerCallback>,
    _audio: sdl3::AudioSubsystem,
}

impl AudioOutput {
    /// Opens the default playback device (or the one `SDL_AUDIO_DRIVER` selects) and starts it.
    pub fn open(sdl: &sdl3::Sdl, placeholder: Placeholder) -> Result<AudioOutput, PlatformError> {
        let audio = sdl.audio().map_err(ctx("audio subsystem"))?;
        let spec = AudioSpec::new(
            Some(SAMPLE_RATE as i32),
            Some(1),
            Some(AudioFormat::f32_sys()),
        );
        let mixer = Arc::new(Mutex::new(Mixer::new(SAMPLE_RATE, placeholder)));
        let callback = MixerCallback {
            mixer: Arc::clone(&mixer),
            buf: Vec::new(),
        };
        let stream = audio
            .open_playback_stream(&spec, callback)
            .map_err(ctx("open audio stream"))?;
        stream.resume().map_err(ctx("resume audio stream"))?;
        Ok(AudioOutput {
            mixer,
            _stream: stream,
            _audio: audio,
        })
    }

    /// Runs `f` on the mixer, between audio callbacks.
    pub fn with_mixer<R>(&self, f: impl FnOnce(&mut Mixer) -> R) -> R {
        f(&mut lock(&self.mixer))
    }

    pub fn play_sfx(&self, id: SfxId) {
        self.with_mixer(|m| m.play_sfx(id));
    }

    pub fn stop_sfx(&self, id: SfxId) {
        self.with_mixer(|m| m.stop_sfx(id));
    }

    pub fn play_bgm(&self, id: BgmId) {
        self.with_mixer(|m| m.play_bgm(id));
    }

    pub fn stop_bgm(&self) {
        self.with_mixer(|m| m.stop_bgm());
    }

    pub fn set_master_volume(&self, v: f32) {
        self.with_mixer(|m| m.set_master_volume(v));
    }

    pub fn set_bgm_volume(&self, v: f32) {
        self.with_mixer(|m| m.set_bgm_volume(v));
    }

    pub fn set_sfx_volume(&self, v: f32) {
        self.with_mixer(|m| m.set_sfx_volume(v));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn mixer() -> Mixer {
        Mixer::new(SAMPLE_RATE, Placeholder::Tones)
    }

    fn render(m: &mut Mixer, n: usize) -> Vec<f32> {
        let mut out = vec![0.0; n];
        m.render(&mut out);
        out
    }

    fn peak(s: &[f32]) -> f32 {
        s.iter().fold(0.0, |a, v| a.max(v.abs()))
    }

    #[test]
    fn idle_mixer_is_silent() {
        assert_eq!(peak(&render(&mut mixer(), 256)), 0.0);
    }

    #[test]
    fn sfx_plays_then_ends() {
        let mut m = mixer();
        m.play_sfx(SfxId(3));
        assert!(m.is_sfx_playing(SfxId(3)));
        assert!(peak(&render(&mut m, 256)) > 0.0);
        let len = (SFX_SECONDS * SAMPLE_RATE as f32) as usize;
        render(&mut m, len);
        assert_eq!(m.active_sfx(), 0);
        assert_eq!(peak(&render(&mut m, 256)), 0.0);
    }

    #[test]
    fn stop_sfx_removes_every_instance_of_that_id_only() {
        let mut m = mixer();
        m.play_sfx(SfxId(1));
        m.play_sfx(SfxId(1));
        m.play_sfx(SfxId(2));
        m.stop_sfx(SfxId(1));
        assert_eq!(m.active_sfx(), 1);
        assert!(m.is_sfx_playing(SfxId(2)));
        m.stop_all_sfx();
        assert_eq!(m.active_sfx(), 0);
    }

    #[test]
    fn sfx_voice_cap_drops_oldest() {
        let mut m = mixer();
        for i in 0..(MAX_SFX_VOICES as u16 + 4) {
            m.play_sfx(SfxId(i));
        }
        assert_eq!(m.active_sfx(), MAX_SFX_VOICES);
        assert!(!m.is_sfx_playing(SfxId(0)));
        assert!(m.is_sfx_playing(SfxId(MAX_SFX_VOICES as u16 + 3)));
    }

    #[test]
    fn bgm_loops_until_stopped_and_replaces() {
        let mut m = mixer();
        m.play_bgm(BgmId(5));
        assert_eq!(m.current_bgm(), Some(BgmId(5)));
        // Several SFX lengths later it is still sounding.
        render(&mut m, SAMPLE_RATE as usize * 2);
        assert!(peak(&render(&mut m, 256)) > 0.0);
        m.play_bgm(BgmId(6));
        assert_eq!(m.current_bgm(), Some(BgmId(6)));
        m.stop_bgm();
        assert_eq!(m.current_bgm(), None);
        assert_eq!(peak(&render(&mut m, 256)), 0.0);
    }

    #[test]
    fn different_ids_give_different_tones() {
        let (mut a, mut b) = (mixer(), mixer());
        a.play_sfx(SfxId(0));
        b.play_sfx(SfxId(7));
        assert_ne!(render(&mut a, 128), render(&mut b, 128));
    }

    #[test]
    fn voices_sum() {
        let (mut one, mut two) = (mixer(), mixer());
        one.play_sfx(SfxId(0));
        two.play_sfx(SfxId(0));
        two.play_sfx(SfxId(0));
        let a = render(&mut one, 64);
        let b = render(&mut two, 64);
        for (x, y) in a.iter().zip(&b) {
            assert!((x * 2.0 - y).abs() < 1e-6);
        }
    }

    #[test]
    fn master_volume_scales_and_zero_mutes() {
        let (mut full, mut half, mut off) = (mixer(), mixer(), mixer());
        for m in [&mut full, &mut half, &mut off] {
            m.play_bgm(BgmId(0));
        }
        half.set_master_volume(0.5);
        off.set_master_volume(0.0);
        let f = render(&mut full, 128);
        let h = render(&mut half, 128);
        for (x, y) in f.iter().zip(&h) {
            assert!((x * 0.5 - y).abs() < 1e-6);
        }
        assert_eq!(peak(&render(&mut off, 128)), 0.0);
    }

    #[test]
    fn bgm_and_sfx_volumes_are_independent() {
        let mut m = mixer();
        m.play_bgm(BgmId(0));
        m.set_bgm_volume(0.0);
        assert_eq!(peak(&render(&mut m, 128)), 0.0);
        m.play_sfx(SfxId(0));
        assert!(peak(&render(&mut m, 128)) > 0.0);
        m.set_sfx_volume(0.0);
        m.set_bgm_volume(1.0);
        let mut sfx_off = m.clone();
        sfx_off.stop_bgm();
        assert_eq!(peak(&render(&mut sfx_off, 128)), 0.0);
        assert!(peak(&render(&mut m, 128)) > 0.0);
    }

    #[test]
    fn volumes_clamp_and_ignore_nan() {
        let mut m = mixer();
        m.set_master_volume(3.0);
        m.set_bgm_volume(-1.0);
        m.set_sfx_volume(f32::NAN);
        assert_eq!(
            m.volumes(),
            Volumes {
                master: 1.0,
                bgm: 0.0,
                sfx: 0.0
            }
        );
    }

    #[test]
    fn silence_placeholder_tracks_voices_but_outputs_zero() {
        let mut m = Mixer::new(SAMPLE_RATE, Placeholder::Silence);
        m.play_sfx(SfxId(1));
        m.play_bgm(BgmId(1));
        assert_eq!(peak(&render(&mut m, 256)), 0.0);
        assert!(m.is_sfx_playing(SfxId(1)));
        assert_eq!(m.current_bgm(), Some(BgmId(1)));
    }

    #[test]
    fn output_stays_in_range_with_many_voices() {
        let mut m = mixer();
        m.play_bgm(BgmId(0));
        for _ in 0..MAX_SFX_VOICES {
            m.play_sfx(SfxId(0));
        }
        assert!(peak(&render(&mut m, 1024)) <= 1.0);
    }

    /// Opens a real SDL3 stream on the dummy driver. Runs only when the environment selects it:
    /// `SDL_AUDIO_DRIVER=dummy cargo test -p rescue-platform audio_stream`.
    #[test]
    fn audio_stream_opens_headless() {
        if std::env::var("SDL_AUDIO_DRIVER").as_deref() != Ok("dummy") {
            eprintln!("SKIPPED: set SDL_AUDIO_DRIVER=dummy to open the audio stream");
            return;
        }
        let sdl = sdl3::init().expect("SDL init");
        let out = AudioOutput::open(&sdl, Placeholder::Tones).expect("open stream");
        out.play_bgm(BgmId(1));
        out.play_sfx(SfxId(2));
        out.set_master_volume(0.5);
        std::thread::sleep(std::time::Duration::from_millis(300));
        // The dummy device drains the stream, so the SFX has ended by now.
        assert!(!out.with_mixer(|m| m.is_sfx_playing(SfxId(2))));
        assert_eq!(out.with_mixer(|m| m.current_bgm()), Some(BgmId(1)));
        out.stop_bgm();
        assert_eq!(out.with_mixer(|m| m.current_bgm()), None);
    }
}
