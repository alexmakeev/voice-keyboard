//! Whisper transcription tests
//!
//! Run with: MODEL_PATH=./models/ggml-tiny.bin cargo test --test whisper_test --features whisper -- --nocapture

#[cfg(feature = "whisper")]
mod tests {
    use std::path::PathBuf;
    use voice_keyboard::vad::VadPhraseDetector;
    use whisper_rs::{FullParams, SamplingStrategy, WhisperContext, WhisperContextParameters};

    fn get_model_path() -> PathBuf {
        let path = std::env::var("MODEL_PATH")
            .map(PathBuf::from)
            .unwrap_or_else(|_| PathBuf::from("./models/ggml-tiny.bin"));

        // In CI with CI_REQUIRE_MODEL=1, panic if model is missing instead of silently skipping
        if !path.exists() {
            if std::env::var("CI_REQUIRE_MODEL").as_deref() == Ok("1") {
                panic!(
                    "CI_REQUIRE_MODEL=1 but model not found at {}. \
                     Ensure the model is downloaded before running tests.",
                    path.display()
                );
            }
        }

        path
    }

    fn load_wav(path: &str) -> Vec<f32> {
        let reader = hound::WavReader::open(path).expect("Failed to open WAV file");
        let spec = reader.spec();

        println!(
            "WAV spec: {} Hz, {} channels, {} bits",
            spec.sample_rate, spec.channels, spec.bits_per_sample
        );

        let samples: Vec<f32> = match spec.sample_format {
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

        // Convert to mono if stereo
        let mono: Vec<f32> = if spec.channels == 2 {
            samples.chunks(2).map(|c| (c[0] + c[1]) / 2.0).collect()
        } else {
            samples
        };

        // Resample to 16kHz if needed (simple decimation)
        let resampled = if spec.sample_rate != 16000 {
            let ratio = spec.sample_rate as f32 / 16000.0;
            let new_len = (mono.len() as f32 / ratio) as usize;
            (0..new_len)
                .map(|i| {
                    let src_idx = (i as f32 * ratio) as usize;
                    mono.get(src_idx).copied().unwrap_or(0.0)
                })
                .collect()
        } else {
            mono
        };

        resampled
    }

    fn transcribe(ctx: &WhisperContext, samples: &[f32]) -> String {
        let mut params = FullParams::new(SamplingStrategy::Greedy { best_of: 1 });

        params.set_print_special(false);
        params.set_print_progress(false);
        params.set_print_realtime(false);
        params.set_print_timestamps(false);
        params.set_translate(false);
        params.set_no_context(true);
        params.set_language(Some("en")); // Force English for test file

        let mut state = ctx.create_state().expect("Failed to create state");
        state.full(params, samples).expect("Transcription failed");

        let num_segments = state.full_n_segments();

        let mut text = String::new();
        for i in 0..num_segments {
            if let Some(segment) = state.get_segment(i) {
                if let Ok(segment_text) = segment.to_str_lossy() {
                    text.push_str(&segment_text);
                }
            }
        }

        text.trim().to_string()
    }

    #[test]
    fn test_model_loads() {
        let model_path = get_model_path();

        if !model_path.exists() {
            eprintln!("Skipping test: model not found at {}", model_path.display());
            eprintln!("Download with: curl -L -o {} https://huggingface.co/ggerganov/whisper.cpp/resolve/main/ggml-tiny.bin",
                model_path.display());
            return;
        }

        println!("Loading model from: {}", model_path.display());

        let params = WhisperContextParameters::default();
        let ctx = WhisperContext::new_with_params(model_path.to_str().unwrap(), params);

        assert!(ctx.is_ok(), "Failed to load model: {:?}", ctx.err());
        println!("Model loaded successfully!");
    }

    #[test]
    fn test_english_transcription() {
        let model_path = get_model_path();

        if !model_path.exists() {
            eprintln!("Skipping test: model not found");
            return;
        }

        let test_wav = PathBuf::from("test_data/english_test.wav");
        if !test_wav.exists() {
            eprintln!(
                "Skipping test: test file not found at {}",
                test_wav.display()
            );
            return;
        }

        println!("Loading model...");
        let params = WhisperContextParameters::default();
        let ctx = WhisperContext::new_with_params(model_path.to_str().unwrap(), params)
            .expect("Failed to load model");

        println!("Loading audio...");
        let samples = load_wav(test_wav.to_str().unwrap());
        println!(
            "Loaded {} samples ({:.1}s at 16kHz)",
            samples.len(),
            samples.len() as f32 / 16000.0
        );

        println!("Transcribing...");
        let result = transcribe(&ctx, &samples);
        println!("Result: \"{}\"", result);

        // The test file contains: "She had your dark suit in greasy wash water all year"
        let result_lower = result.to_lowercase();

        // Check for key words (model might not get it exactly right)
        assert!(
            result_lower.contains("dark") ||
            result_lower.contains("suit") ||
            result_lower.contains("wash") ||
            result_lower.contains("water") ||
            result_lower.contains("year"),
            "Expected transcription to contain keywords from 'She had your dark suit in greasy wash water all year', got: '{}'",
            result
        );

        println!("Test passed!");
    }

    #[test]
    fn test_silence_detection() {
        let model_path = get_model_path();

        if !model_path.exists() {
            eprintln!("Skipping test: model not found");
            return;
        }

        println!("Loading model...");
        let params = WhisperContextParameters::default();
        let ctx = WhisperContext::new_with_params(model_path.to_str().unwrap(), params)
            .expect("Failed to load model");

        // Create 2 seconds of silence
        let silence: Vec<f32> = vec![0.0; 16000 * 2];

        println!("Transcribing silence...");
        let result = transcribe(&ctx, &silence);
        println!("Result for silence: \"{}\"", result);

        // Silence should produce empty or very short result
        assert!(
            result.len() < 50,
            "Silence should not produce much text, got {} chars: '{}'",
            result.len(),
            result
        );

        println!("Silence test passed!");
    }

    /// Transcribe with Russian language
    fn transcribe_russian(ctx: &WhisperContext, samples: &[f32]) -> String {
        let mut params = FullParams::new(SamplingStrategy::Greedy { best_of: 1 });

        params.set_print_special(false);
        params.set_print_progress(false);
        params.set_print_realtime(false);
        params.set_print_timestamps(false);
        params.set_translate(false);
        params.set_no_context(true);
        params.set_language(Some("ru")); // Force Russian

        let mut state = ctx.create_state().expect("Failed to create state");
        state.full(params, samples).expect("Transcription failed");

        let num_segments = state.full_n_segments();

        let mut text = String::new();
        for i in 0..num_segments {
            if let Some(segment) = state.get_segment(i) {
                if let Ok(segment_text) = segment.to_str_lossy() {
                    text.push_str(&segment_text);
                }
            }
        }

        text.trim().to_string()
    }

    #[test]
    fn test_russian_transcription_10s() {
        let model_path = get_model_path();

        if !model_path.exists() {
            eprintln!("Skipping test: model not found");
            return;
        }

        let test_wav = PathBuf::from("test_data/russian_speech_10s.wav");
        if !test_wav.exists() {
            eprintln!(
                "Skipping test: test file not found at {}",
                test_wav.display()
            );
            return;
        }

        println!("Loading model...");
        let params = WhisperContextParameters::default();
        let ctx = WhisperContext::new_with_params(model_path.to_str().unwrap(), params)
            .expect("Failed to load model");

        println!("Loading Russian audio...");
        let samples = load_wav(test_wav.to_str().unwrap());
        println!(
            "Loaded {} samples ({:.1}s at 16kHz)",
            samples.len(),
            samples.len() as f32 / 16000.0
        );

        println!("Transcribing Russian...");
        let result = transcribe_russian(&ctx, &samples);
        println!("Russian result: \"{}\"", result);

        // Should produce some text (LibriVox audiobook)
        assert!(
            result.len() > 10,
            "Expected Russian transcription to contain text, got: '{}'",
            result
        );

        // Russian transcription must contain Cyrillic characters
        assert!(
            result.chars().any(|c| c >= '\u{0400}' && c <= '\u{04FF}'),
            "Russian transcription should contain Cyrillic characters, got: '{}'",
            result
        );

        println!("Russian 10s test passed!");
    }

    #[test]
    fn test_russian_transcription_30s() {
        let model_path = get_model_path();

        if !model_path.exists() {
            eprintln!("Skipping test: model not found");
            return;
        }

        let test_wav = PathBuf::from("test_data/russian_speech_30s.wav");
        if !test_wav.exists() {
            eprintln!(
                "Skipping test: test file not found at {}",
                test_wav.display()
            );
            return;
        }

        println!("Loading model...");
        let params = WhisperContextParameters::default();
        let ctx = WhisperContext::new_with_params(model_path.to_str().unwrap(), params)
            .expect("Failed to load model");

        println!("Loading Russian audio (30s)...");
        let samples = load_wav(test_wav.to_str().unwrap());
        println!(
            "Loaded {} samples ({:.1}s at 16kHz)",
            samples.len(),
            samples.len() as f32 / 16000.0
        );

        println!("Transcribing Russian...");
        let start = std::time::Instant::now();
        let result = transcribe_russian(&ctx, &samples);
        let elapsed = start.elapsed();
        println!("Russian result (30s): \"{}\"", result);
        println!("Transcription took: {:?}", elapsed);

        // Should produce text
        assert!(
            result.len() > 20,
            "Expected Russian transcription to contain text, got: '{}'",
            result
        );

        println!("Russian 30s test passed!");
    }

    #[test]
    fn test_russian_vad_phrase_detection() {
        let model_path = get_model_path();

        if !model_path.exists() {
            eprintln!("Skipping test: model not found");
            return;
        }

        let test_wav = PathBuf::from("test_data/russian_speech_30s.wav");
        if !test_wav.exists() {
            eprintln!(
                "Skipping test: test file not found at {}",
                test_wav.display()
            );
            return;
        }

        println!("Loading model...");
        let params = WhisperContextParameters::default();
        let ctx = WhisperContext::new_with_params(model_path.to_str().unwrap(), params)
            .expect("Failed to load model");

        println!("Loading Russian audio (30s)...");
        let samples = load_wav(test_wav.to_str().unwrap());
        println!(
            "Loaded {} samples ({:.1}s at 16kHz)",
            samples.len(),
            samples.len() as f32 / 16000.0
        );

        // Use VAD to detect phrases
        let mut vad = VadPhraseDetector::new(16000);
        let mut phrases = Vec::new();

        while let Some((phrase, _, _)) = vad.detect_phrase(&samples) {
            phrases.push(phrase);
        }
        if let Some((rem, _, _)) = vad.get_remaining(&samples) {
            phrases.push(rem);
        }

        println!("VAD detected {} phrases", phrases.len());

        // Transcribe each phrase
        let mut full_text = String::new();
        for (i, phrase) in phrases.iter().enumerate() {
            let duration = phrase.len() as f32 / 16000.0;
            println!(
                "Phrase {}: {:.1}s ({} samples)",
                i + 1,
                duration,
                phrase.len()
            );

            let text = transcribe_russian(&ctx, phrase);
            println!("  Text: \"{}\"", text);

            if !text.is_empty() {
                if !full_text.is_empty() {
                    full_text.push(' ');
                }
                full_text.push_str(&text);
            }
        }

        println!("\nFull transcription from VAD phrases:");
        println!("\"{}\"", full_text);

        // Verify we got meaningful output
        assert!(
            full_text.len() > 20,
            "Expected meaningful transcription from VAD phrases, got: '{}'",
            full_text
        );

        println!("\nRussian VAD phrase detection test passed!");
    }

    #[test]
    fn test_russian_60s_with_vad() {
        let model_path = get_model_path();

        if !model_path.exists() {
            eprintln!("Skipping test: model not found");
            return;
        }

        let test_wav = PathBuf::from("test_data/russian_speech_60s.wav");
        if !test_wav.exists() {
            eprintln!(
                "Skipping test: test file not found at {}",
                test_wav.display()
            );
            return;
        }

        println!("Loading model...");
        let params = WhisperContextParameters::default();
        let ctx = WhisperContext::new_with_params(model_path.to_str().unwrap(), params)
            .expect("Failed to load model");

        println!("Loading Russian audio (60s)...");
        let samples = load_wav(test_wav.to_str().unwrap());
        println!(
            "Loaded {} samples ({:.1}s at 16kHz)",
            samples.len(),
            samples.len() as f32 / 16000.0
        );

        // Use VAD to detect phrases
        let mut vad = VadPhraseDetector::new(16000);
        let mut phrases = Vec::new();

        let vad_start = std::time::Instant::now();
        while let Some((phrase, _, _)) = vad.detect_phrase(&samples) {
            phrases.push(phrase);
        }
        if let Some((rem, _, _)) = vad.get_remaining(&samples) {
            phrases.push(rem);
        }
        let total_vad_time = vad_start.elapsed();

        println!(
            "VAD detected {} phrases in {:?}",
            phrases.len(),
            total_vad_time
        );

        // Transcribe each phrase and measure time
        let mut full_text = String::new();
        let mut total_transcribe_time = std::time::Duration::ZERO;

        for (i, phrase) in phrases.iter().enumerate() {
            let duration = phrase.len() as f32 / 16000.0;

            let start = std::time::Instant::now();
            let text = transcribe_russian(&ctx, phrase);
            let elapsed = start.elapsed();
            total_transcribe_time += elapsed;

            println!(
                "Phrase {}: {:.1}s -> {:?} -> \"{}\"",
                i + 1,
                duration,
                elapsed,
                text
            );

            if !text.is_empty() {
                if !full_text.is_empty() {
                    full_text.push(' ');
                }
                full_text.push_str(&text);
            }
        }

        println!("\n=== Summary ===");
        println!("Audio duration: 60s");
        println!("Phrases detected: {}", phrases.len());
        println!("VAD processing time: {:?}", total_vad_time);
        println!("Total transcription time: {:?}", total_transcribe_time);
        println!("Full text ({} chars): \"{}\"", full_text.len(), full_text);

        assert!(
            full_text.len() > 50,
            "Expected meaningful transcription from 60s audio, got: '{}'",
            full_text
        );

        println!("\nRussian 60s VAD test passed!");
    }
}

#[cfg(not(feature = "whisper"))]
fn main() {
    eprintln!("Tests require 'whisper' feature. Run with:");
    eprintln!(
        "  MODEL_PATH=./models/ggml-tiny.bin cargo test --test whisper_test --features whisper"
    );
}
