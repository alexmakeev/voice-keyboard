//! Integration tests for transcription
//!
//! These tests require:
//! 1. A Whisper model (tiny recommended for CI)
//! 2. Test audio files in test-assets/
//!
//! To run:
//!   MODEL_PATH=./models/ggml-tiny.bin cargo test --test transcription_test
//!
//! To skip if model not available:
//!   cargo test --test transcription_test -- --ignored

use std::path::PathBuf;
use voice_keyboard::transcribe::Transcriber;

fn get_model_path() -> Option<PathBuf> {
    // Check environment variable first
    if let Ok(path) = std::env::var("MODEL_PATH") {
        let p = PathBuf::from(path);
        if p.exists() {
            return Some(p);
        }
    }

    // Check common locations
    let paths = [
        "./models/ggml-tiny.bin",
        "./models/ggml-base.bin",
        "./test-assets/ggml-tiny.bin",
        "../models/ggml-tiny.bin",
    ];

    for path in paths {
        let p = PathBuf::from(path);
        if p.exists() {
            return Some(p);
        }
    }

    None
}

fn get_test_audio_path() -> Option<PathBuf> {
    let paths = [
        "./test-assets/test-en.wav",
        "./test-assets/test.wav",
        "../test-assets/test-en.wav",
    ];

    for path in paths {
        let p = PathBuf::from(path);
        if p.exists() {
            return Some(p);
        }
    }

    None
}

#[test]
#[ignore = "Requires Whisper model and test audio"]
fn test_transcribe_english() {
    let model_path = get_model_path()
        .expect("Model not found. Set MODEL_PATH env var or place model in ./models/ggml-tiny.bin");

    let audio_path =
        get_test_audio_path().expect("Test audio not found. Place test.wav in ./test-assets/");

    let transcriber = Transcriber::new(&model_path).expect("Failed to load model");
    let result = transcriber
        .transcribe_file(&audio_path)
        .expect("Transcription failed");

    println!("Transcribed: {}", result.text);
    println!("Language: {:?}", result.language);
    println!("Duration: {}ms", result.duration_ms);

    // Basic assertions
    assert!(!result.text.is_empty(), "Transcription should not be empty");
    assert!(result.duration_ms > 0, "Duration should be positive");
}

#[test]
#[ignore = "Requires Whisper model"]
fn test_transcribe_samples() {
    let model_path = get_model_path().expect("Model not found");

    let transcriber = Transcriber::new(&model_path).expect("Failed to load model");

    // Generate 1 second of silence (should produce empty or minimal output)
    let silence: Vec<f32> = vec![0.0; 16000];

    let result = transcriber
        .transcribe_samples(&silence)
        .expect("Transcription failed");

    println!("Silence transcription: '{}'", result.text);
    // Silence might produce some artifacts, but should be very short
    assert!(
        result.text.len() < 50,
        "Silence should produce minimal text"
    );
}

#[test]
#[ignore = "Requires Whisper model"]
fn test_transcribe_sine_wave() {
    let model_path = get_model_path().expect("Model not found");

    let transcriber = Transcriber::new(&model_path).expect("Failed to load model");

    // Generate 1 second of 440Hz sine wave (not speech, should produce minimal output)
    let sample_rate = 16000.0;
    let frequency = 440.0;
    let duration = 1.0;
    let samples: Vec<f32> = (0..(sample_rate * duration) as usize)
        .map(|i| (2.0 * std::f32::consts::PI * frequency * i as f32 / sample_rate).sin() * 0.5)
        .collect();

    let result = transcriber
        .transcribe_samples(&samples)
        .expect("Transcription failed");

    println!("Sine wave transcription: '{}'", result.text);
    // Non-speech audio should produce minimal text
}

/// Benchmark transcription speed
#[test]
#[ignore = "Requires Whisper model and test audio"]
fn bench_transcription_speed() {
    let model_path = get_model_path().expect("Model not found");
    let audio_path = get_test_audio_path().expect("Test audio not found");

    let transcriber = Transcriber::new(&model_path).expect("Failed to load model");

    // Warm up
    let _ = transcriber.transcribe_file(&audio_path);

    // Benchmark
    let iterations = 3;
    let mut total_ms = 0u64;

    for i in 0..iterations {
        let start = std::time::Instant::now();
        let _ = transcriber
            .transcribe_file(&audio_path)
            .expect("Transcription failed");
        let elapsed = start.elapsed().as_millis() as u64;
        total_ms += elapsed;
        println!("Iteration {}: {}ms", i + 1, elapsed);
    }

    let avg_ms = total_ms / iterations as u64;
    println!("Average: {}ms over {} iterations", avg_ms, iterations);
}
