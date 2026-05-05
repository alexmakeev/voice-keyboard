//! VAD integration tests against the production VadPhraseDetector.
//! Run with: cargo test --test vad_test -- --nocapture

use std::path::PathBuf;
use voice_keyboard::vad::{
    VadPhraseDetector, MIN_FRAGMENT_DURATION_MS, MIN_VOICE_RATIO_FOR_SPEECH, VAD_ENERGY_THRESHOLD,
    VAD_MIN_SPEECH_MS, VAD_SILENCE_MS, VAD_WINDOW_MS,
};

/// Load WAV at `rel_path` (relative to test_data/), return (mono f32 samples, sample_rate).
fn load_wav(rel_path: &str) -> (Vec<f32>, u32) {
    let path = PathBuf::from("test_data").join(rel_path);
    let reader = hound::WavReader::open(&path)
        .unwrap_or_else(|e| panic!("Failed to open {}: {}", path.display(), e));
    let spec = reader.spec();
    let raw: Vec<f32> = match spec.sample_format {
        hound::SampleFormat::Int => reader
            .into_samples::<i16>()
            .filter_map(|s| s.ok())
            .map(|s| s as f32 / i16::MAX as f32)
            .collect(),
        hound::SampleFormat::Float => reader
            .into_samples::<f32>()
            .filter_map(|s| s.ok())
            .collect(),
    };
    let mono: Vec<f32> = if spec.channels == 2 {
        raw.chunks(2).map(|c| (c[0] + c[1]) / 2.0).collect()
    } else {
        raw
    };
    (mono, spec.sample_rate)
}

/// Drive `detect_phrase` to completion against the full sample buffer.
fn collect_phrases(vad: &mut VadPhraseDetector, samples: &[f32]) -> Vec<(Vec<f32>, usize, usize)> {
    let mut out = Vec::new();
    while let Some(p) = vad.detect_phrase(samples) {
        out.push(p);
    }
    out
}

fn sine(duration_ms: u64, freq: f32, sample_rate: u32, amplitude: f32) -> Vec<f32> {
    let n = (duration_ms as f32 * sample_rate as f32 / 1000.0) as usize;
    (0..n)
        .map(|i| {
            let t = i as f32 / sample_rate as f32;
            amplitude * (2.0 * std::f32::consts::PI * freq * t).sin()
        })
        .collect()
}

fn silence(duration_ms: u64, sample_rate: u32) -> Vec<f32> {
    vec![0.0; (duration_ms as f32 * sample_rate as f32 / 1000.0) as usize]
}

#[test]
fn test_vad_construction_uses_passed_sample_rate() {
    let vad16 = VadPhraseDetector::new(16000);
    assert_eq!(vad16.window_samples, (VAD_WINDOW_MS as usize) * 16);

    let vad48 = VadPhraseDetector::new(48000);
    assert_eq!(vad48.window_samples, (VAD_WINDOW_MS as usize) * 48);
}

#[test]
fn test_vad_silence_yields_no_phrases() {
    let samples = silence(2000, 16000);
    let mut vad = VadPhraseDetector::new(16000);
    assert!(vad.detect_phrase(&samples).is_none());
    assert!(vad.get_remaining(&samples).is_none());
}

#[test]
fn test_vad_short_speech_buffered() {
    // 500ms speech (>= VAD_MIN_SPEECH_MS=400) but < MIN_FRAGMENT_DURATION_MS=1000
    let mut samples = silence(50, 16000);
    samples.extend(sine(500, 200.0, 16000, 0.2));
    samples.extend(silence(500, 16000));

    let mut vad = VadPhraseDetector::new(16000);
    let phrases = collect_phrases(&mut vad, &samples);
    // Short fragment is buffered, not emitted.
    assert!(
        phrases.is_empty(),
        "expected empty (buffered), got {} phrases",
        phrases.len()
    );
    // Buffered position should be set or detection continued past it.
    // (Documents the buffering behaviour; exact field state depends on algorithm internals.)
}

#[test]
fn test_vad_long_phrase_emitted_inline() {
    // 1500ms speech (>= MIN_FRAGMENT_DURATION_MS=1000) at 200Hz (voice band).
    let mut samples = silence(50, 16000);
    samples.extend(sine(1500, 200.0, 16000, 0.2));
    samples.extend(silence(500, 16000));

    let mut vad = VadPhraseDetector::new(16000);
    let phrases = collect_phrases(&mut vad, &samples);
    assert!(
        !phrases.is_empty(),
        "expected at least 1 phrase from 1500ms 200Hz sine + 500ms silence"
    );
}

#[test]
fn test_vad_real_speech_russian_10s() {
    let (samples, sr) = load_wav("russian_speech_10s.wav");
    assert_eq!(sr, 16000, "russian_speech_10s.wav expected at 16kHz");
    let mut vad = VadPhraseDetector::new(sr);
    let mut phrases = collect_phrases(&mut vad, &samples);
    if let Some(rem) = vad.get_remaining(&samples) {
        phrases.push(rem);
    }
    assert!(
        !phrases.is_empty(),
        "expected at least 1 phrase from 10s of real Russian speech"
    );
    for (i, (p, _, _)) in phrases.iter().enumerate() {
        let dur_ms = p.len() as f64 * 1000.0 / sr as f64;
        assert!(
            dur_ms >= (VAD_MIN_SPEECH_MS as f64) - 50.0,
            "phrase {} too short: {}ms",
            i,
            dur_ms
        );
        eprintln!("phrase {}: {:.0}ms", i, dur_ms);
    }
}

#[test]
fn test_vad_real_speech_russian_30s() {
    let (samples, sr) = load_wav("russian_speech_30s.wav");
    assert_eq!(sr, 16000);
    let mut vad = VadPhraseDetector::new(sr);
    let mut phrases = collect_phrases(&mut vad, &samples);
    if let Some(rem) = vad.get_remaining(&samples) {
        phrases.push(rem);
    }
    assert!(
        phrases.len() >= 2,
        "expected >=2 phrases from 30s real speech, got {}",
        phrases.len()
    );
}

#[test]
fn test_vad_real_speech_english() {
    let (samples, sr) = load_wav("english_test.wav");
    let mut vad = VadPhraseDetector::new(sr);
    let phrases = collect_phrases(&mut vad, &samples);
    let remaining = vad.get_remaining(&samples);
    assert!(
        !phrases.is_empty() || remaining.is_some(),
        "expected at least 1 phrase OR pending fragment from english_test.wav (sr={})",
        sr
    );
}

#[test]
fn test_vad_piano_no_panic() {
    // Negative case: piano has no formant structure; VAD behavior is undefined
    // but must not panic. Document actual phrase count via stderr.
    let (samples, sr) = load_wav("samples/piano0.wav");
    eprintln!("piano0.wav: sr={}, samples={}", sr, samples.len());
    let mut vad = VadPhraseDetector::new(sr);
    let phrases = collect_phrases(&mut vad, &samples);
    eprintln!("piano0 produced {} phrases", phrases.len());
}

#[test]
fn test_vad_reset_clears_state() {
    let mut samples = silence(50, 16000);
    samples.extend(sine(1500, 200.0, 16000, 0.2));
    samples.extend(silence(500, 16000));

    let mut vad = VadPhraseDetector::new(16000);
    let _ = vad.detect_phrase(&samples);
    vad.reset();

    assert!(!vad.in_speech);
    assert_eq!(vad.silent_windows, 0);
    assert_eq!(vad.phrase_start, 0);
    assert_eq!(vad.processed_pos, 0);
    assert_eq!(vad.voice_windows_count, 0);
    assert_eq!(vad.phrase_windows_count, 0);
    assert_eq!(vad.last_transcribed_end, 0);
    assert!(vad.buffered_start.is_none());
}

#[test]
fn test_vad_has_voice_content_silence_returns_false() {
    let samples = silence(1000, 16000);
    let vad = VadPhraseDetector::new(16000);
    let (has_voice, ratio) = vad.has_voice_content(&samples);
    assert!(!has_voice);
    assert!(ratio < MIN_VOICE_RATIO_FOR_SPEECH);
}

#[test]
fn test_vad_has_voice_content_real_speech_returns_true() {
    let (samples, sr) = load_wav("russian_speech_10s.wav");
    let vad = VadPhraseDetector::new(sr);
    let (has_voice, ratio) = vad.has_voice_content(&samples);
    assert!(
        has_voice,
        "real speech should be classified as voice (ratio={})",
        ratio
    );
    assert!(ratio >= MIN_VOICE_RATIO_FOR_SPEECH);
}

// Reference unused imports to silence warnings (these are part of the module's pub API).
#[allow(dead_code)]
const _ENERGY_REF: f32 = VAD_ENERGY_THRESHOLD;
#[allow(dead_code)]
const _SILENCE_REF: u64 = VAD_SILENCE_MS;
#[allow(dead_code)]
const _FRAGMENT_REF: u64 = MIN_FRAGMENT_DURATION_MS;
