//! Voice Activity Detection: phrase boundary detection on raw f32 PCM.
//!
//! Uses RMS energy gate + Goertzel filter on voice-band frequencies (100/150/200/250 Hz)
//! vs. noise-band (50/400/600/1000 Hz) to compute a voice ratio. Phrases are emitted
//! when energy drops for `VAD_SILENCE_MS` and the buffered phrase passes voice/duration
//! gates. Short fragments (<`MIN_FRAGMENT_DURATION_MS`) are merged with the next phrase.
//!
//! This module preserves the exact algorithm previously inlined in voice_typer.rs.
//! The only API change vs. the original is that sample rate is now a constructor
//! parameter instead of a hardcoded constant.

pub const VAD_SILENCE_MS: u64 = 100;
pub const VAD_MIN_SPEECH_MS: u64 = 400;
pub const VAD_WINDOW_MS: u64 = 30;
pub const VAD_ENERGY_THRESHOLD: f32 = 0.001;
pub const VAD_VOICE_RATIO_THRESHOLD: f32 = 0.15;
pub const VAD_SPEECH_CONFIRM_WINDOWS: usize = 2;
pub const MIN_FRAGMENT_DURATION_MS: u64 = 1000;
pub const MIN_VOICE_RATIO_FOR_SPEECH: f32 = 0.10;

pub struct VadPhraseDetector {
    pub window_samples: usize,
    pub silence_windows_threshold: usize,
    pub min_speech_windows: usize,
    pub silent_windows: usize,
    pub speech_confirm_count: usize,
    pub in_speech: bool,
    pub phrase_start: usize,
    pub processed_pos: usize,
    pub voice_ratio: f32,
    pub voice_windows_count: usize,
    pub phrase_windows_count: usize,
    pub last_transcribed_end: usize,
    pub buffered_start: Option<usize>,
    sample_rate: u32,
}

impl VadPhraseDetector {
    pub fn new(sample_rate: u32) -> Self {
        let window_samples = (VAD_WINDOW_MS as f32 * sample_rate as f32 / 1000.0) as usize;
        let silence_windows_threshold = (VAD_SILENCE_MS / VAD_WINDOW_MS) as usize;
        let min_speech_windows = (VAD_MIN_SPEECH_MS / VAD_WINDOW_MS) as usize;

        Self {
            window_samples,
            silence_windows_threshold,
            min_speech_windows,
            silent_windows: 0,
            speech_confirm_count: 0,
            in_speech: false,
            phrase_start: 0,
            processed_pos: 0,
            voice_ratio: 0.0,
            voice_windows_count: 0,
            phrase_windows_count: 0,
            last_transcribed_end: 0,
            buffered_start: None,
            sample_rate,
        }
    }

    pub fn calculate_energy(&self, samples: &[f32]) -> f32 {
        if samples.is_empty() {
            return 0.0;
        }
        let sum_sq: f32 = samples.iter().map(|s| s * s).sum();
        (sum_sq / samples.len() as f32).sqrt()
    }

    fn goertzel_energy(&self, samples: &[f32], target_freq: f32, sample_rate: f32) -> f32 {
        let n = samples.len();
        let k = (0.5 + (n as f32 * target_freq / sample_rate)) as usize;
        let w = 2.0 * std::f32::consts::PI * k as f32 / n as f32;
        let coeff = 2.0 * w.cos();

        let mut s1 = 0.0f32;
        let mut s2 = 0.0f32;

        for &sample in samples {
            let s0 = sample + coeff * s1 - s2;
            s2 = s1;
            s1 = s0;
        }

        s1 * s1 + s2 * s2 - coeff * s1 * s2
    }

    fn calculate_voice_ratio(&self, samples: &[f32]) -> f32 {
        if samples.is_empty() {
            return 0.0;
        }

        let sample_rate = self.sample_rate as f32;

        let mut voice_energy = 0.0f32;
        let voice_freqs = [100.0, 150.0, 200.0, 250.0];
        for &freq in &voice_freqs {
            voice_energy += self.goertzel_energy(samples, freq, sample_rate);
        }
        voice_energy /= voice_freqs.len() as f32;

        let mut noise_energy = 0.0f32;
        let noise_freqs = [50.0, 400.0, 600.0, 1000.0];
        for &freq in &noise_freqs {
            noise_energy += self.goertzel_energy(samples, freq, sample_rate);
        }
        noise_energy /= noise_freqs.len() as f32;

        let total = voice_energy + noise_energy;
        if total > 0.0 {
            voice_energy / total
        } else {
            0.0
        }
    }

    fn is_speech(&mut self, samples: &[f32]) -> bool {
        let energy = self.calculate_energy(samples);

        if energy < VAD_ENERGY_THRESHOLD {
            self.voice_ratio = 0.0;
            return false;
        }

        self.voice_ratio = self.calculate_voice_ratio(samples);
        self.voice_ratio >= VAD_VOICE_RATIO_THRESHOLD
    }

    /// Returns (samples, start_pos, end_pos) if phrase detected
    pub fn detect_phrase(&mut self, all_samples: &[f32]) -> Option<(Vec<f32>, usize, usize)> {
        while self.processed_pos + self.window_samples <= all_samples.len() {
            let window_start = self.processed_pos;
            let window_end = window_start + self.window_samples;
            let window = &all_samples[window_start..window_end];

            let is_speech = self.is_speech(window);
            let has_voice = self.voice_ratio >= VAD_VOICE_RATIO_THRESHOLD;

            if is_speech {
                self.speech_confirm_count += 1;
                self.phrase_windows_count += 1;
                if has_voice {
                    self.voice_windows_count += 1;
                }

                if !self.in_speech {
                    self.in_speech = true;
                    self.phrase_start = window_start;
                    self.voice_windows_count = if has_voice { 1 } else { 0 };
                    self.phrase_windows_count = 1;
                }

                if self.speech_confirm_count >= VAD_SPEECH_CONFIRM_WINDOWS {
                    self.silent_windows = 0;
                }
            } else {
                self.speech_confirm_count = 0;

                if self.in_speech {
                    self.silent_windows += 1;

                    if self.silent_windows >= self.silence_windows_threshold {
                        let phrase_end =
                            window_start - (self.silent_windows - 1) * self.window_samples;
                        let phrase_len = phrase_end.saturating_sub(self.phrase_start);

                        let voice_ratio = if self.phrase_windows_count > 0 {
                            self.voice_windows_count as f32 / self.phrase_windows_count as f32
                        } else {
                            0.0
                        };
                        // Lowered from 0.3 to 0.2 - less strict voice requirement
                        let has_enough_voice = voice_ratio >= 0.2;

                        let duration_ms = phrase_len as f32 / self.sample_rate as f32 * 1000.0;
                        let min_duration_ms = (self.min_speech_windows * self.window_samples)
                            as f32
                            / self.sample_rate as f32
                            * 1000.0;

                        if phrase_len >= self.min_speech_windows * self.window_samples
                            && has_enough_voice
                        {
                            // Use buffered start if we have a short fragment waiting
                            let start_pos = self.buffered_start.unwrap_or(self.phrase_start);
                            let end_pos = phrase_end;
                            let combined_len = end_pos.saturating_sub(start_pos);
                            let combined_duration_ms =
                                combined_len as f32 / self.sample_rate as f32 * 1000.0;
                            let min_fragment_samples = (MIN_FRAGMENT_DURATION_MS as f32
                                * self.sample_rate as f32
                                / 1000.0)
                                as usize;

                            // Check if combined fragment is long enough to send
                            if combined_len >= min_fragment_samples {
                                let phrase = all_samples[start_pos..end_pos].to_vec();
                                if self.buffered_start.is_some() {
                                    println!(
                                        "[VAD] ✓ Phrase ACCEPTED (merged): {:.0}ms, {:.0}% voice",
                                        combined_duration_ms,
                                        voice_ratio * 100.0
                                    );
                                } else {
                                    println!(
                                        "[VAD] ✓ Phrase ACCEPTED: {:.0}ms, {:.0}% voice ({}/{} windows)",
                                        duration_ms,
                                        voice_ratio * 100.0,
                                        self.voice_windows_count,
                                        self.phrase_windows_count
                                    );
                                }
                                self.in_speech = false;
                                self.silent_windows = 0;
                                self.voice_windows_count = 0;
                                self.phrase_windows_count = 0;
                                self.last_transcribed_end = end_pos;
                                self.phrase_start = window_end;
                                self.processed_pos = window_end;
                                self.buffered_start = None; // Clear buffer
                                return Some((phrase, start_pos, end_pos));
                            } else {
                                // Fragment too short - buffer it for merging with next
                                if self.buffered_start.is_none() {
                                    self.buffered_start = Some(start_pos);
                                }
                                println!(
                                    "[VAD] ⏳ Phrase BUFFERED: {:.0}ms < {}ms min, waiting for next",
                                    combined_duration_ms,
                                    MIN_FRAGMENT_DURATION_MS
                                );
                                self.in_speech = false;
                                self.silent_windows = 0;
                                self.voice_windows_count = 0;
                                self.phrase_windows_count = 0;
                                self.phrase_start = window_end;
                                self.processed_pos = window_end;
                                // Don't return - continue to next iteration
                            }
                        } else {
                            // Log rejection reason
                            let reject_reason = if phrase_len
                                < self.min_speech_windows * self.window_samples
                            {
                                format!(
                                    "too short ({:.0}ms < {:.0}ms min)",
                                    duration_ms, min_duration_ms
                                )
                            } else {
                                format!("low voice ({:.0}% < 20% threshold)", voice_ratio * 100.0)
                            };

                            // Even rejected phrases should be buffered if we have existing buffer
                            // This ensures pauses between fragments don't lose audio
                            if self.buffered_start.is_some() {
                                println!(
                                    "[VAD] ⏳ Phrase REJECTED but keeping buffer: {} - {:.0}ms",
                                    reject_reason, duration_ms
                                );
                            } else {
                                println!(
                                    "[VAD] ✗ Phrase REJECTED: {} - {:.0}ms, {:.0}% voice ({}/{} windows)",
                                    reject_reason,
                                    duration_ms,
                                    voice_ratio * 100.0,
                                    self.voice_windows_count,
                                    self.phrase_windows_count
                                );
                            }
                            self.in_speech = false;
                            self.silent_windows = 0;
                            self.voice_windows_count = 0;
                            self.phrase_windows_count = 0;
                            self.phrase_start = window_end;
                        }
                    }
                }
            }

            self.processed_pos = window_end;
        }

        None
    }

    /// Returns (samples, start_pos, end_pos) for remaining audio
    pub fn get_remaining(&self, all_samples: &[f32]) -> Option<(Vec<f32>, usize, usize)> {
        // Minimum samples for final segment - lower than mid-recording threshold
        // because user explicitly released key = they finished speaking
        // 200ms is a compromise: short enough to catch final words, long enough to avoid noise
        let min_final_samples = (200.0 * self.sample_rate as f32 / 1000.0) as usize; // 200ms

        // Start from the position after the last transcribed phrase
        // This prevents double transcription when VAD and key release happen simultaneously
        // If we have a buffered short fragment, use its start position
        let start_pos = if let Some(buffered) = self.buffered_start {
            buffered
        } else if self.in_speech {
            self.phrase_start
        } else {
            // Use the maximum of processed_pos and last_transcribed_end
            // to avoid re-transcribing already processed audio
            self.processed_pos.max(self.last_transcribed_end)
        };

        let total_samples = all_samples.len();
        let duration_total_ms = total_samples as f32 / self.sample_rate as f32 * 1000.0;

        println!(
            "[VAD] get_remaining: total={} samples ({:.0}ms), in_speech={}, phrase_start={}, processed_pos={}, last_transcribed_end={}, start_pos={}",
            total_samples, duration_total_ms, self.in_speech, self.phrase_start, self.processed_pos, self.last_transcribed_end, start_pos
        );

        if start_pos >= all_samples.len() {
            println!("[VAD] ✗ Final REJECTED: start_pos >= total_samples (no remaining audio)");
            return None;
        }

        let remaining = &all_samples[start_pos..];
        let remaining_len = remaining.len();
        let end_pos = all_samples.len();
        let remaining_ms = remaining_len as f32 / self.sample_rate as f32 * 1000.0;
        let min_final_ms = min_final_samples as f32 / self.sample_rate as f32 * 1000.0;

        if remaining_len < min_final_samples {
            println!(
                "[VAD] ✗ Final REJECTED: too short ({:.0}ms < {:.0}ms min)",
                remaining_ms, min_final_ms
            );
            return None;
        }

        // For final segment, use lower voice threshold - user released key intentionally
        let mut voice_windows = 0;
        let mut total_windows = 0;

        for chunk in remaining.chunks(self.window_samples) {
            if chunk.len() < self.window_samples {
                break;
            }
            total_windows += 1;

            let voice_ratio = self.calculate_voice_ratio(chunk);
            let energy = self.calculate_energy(chunk);

            // Lower threshold for final segment
            if energy >= VAD_ENERGY_THRESHOLD * 0.5
                && voice_ratio >= VAD_VOICE_RATIO_THRESHOLD * 0.5
            {
                voice_windows += 1;
            }
        }

        let voice_percent = if total_windows > 0 {
            voice_windows as f32 / total_windows as f32
        } else {
            0.0
        };

        // Lowered from 0.15 to 0.10 - less strict for final segment
        // BUT: if we have buffered audio, always send it (user spoke something earlier)
        if voice_percent < 0.10 && self.buffered_start.is_none() {
            println!(
                "[VAD] ✗ Final REJECTED: low voice ({:.0}% < 10% threshold) - {:.0}ms, {}/{} windows",
                voice_percent * 100.0,
                remaining_ms,
                voice_windows,
                total_windows
            );
            return None;
        }

        if self.buffered_start.is_some() {
            println!(
                "[VAD] ✓ Final ACCEPTED (with buffer): {:.0}ms, {:.0}% voice ({}/{} windows)",
                remaining_ms,
                voice_percent * 100.0,
                voice_windows,
                total_windows
            );
        } else {
            println!(
                "[VAD] ✓ Final ACCEPTED: {:.0}ms, {:.0}% voice ({}/{} windows)",
                remaining_ms,
                voice_percent * 100.0,
                voice_windows,
                total_windows
            );
        }
        Some((remaining.to_vec(), start_pos, end_pos))
    }

    /// Check if audio samples contain voice content
    /// Used to filter out accidental short recordings with only silence/noise
    /// Returns (has_voice, voice_ratio) where voice_ratio is percentage of windows with voice
    pub fn has_voice_content(&self, samples: &[f32]) -> (bool, f32) {
        if samples.is_empty() {
            return (false, 0.0);
        }

        let mut voice_windows = 0;
        let mut total_windows = 0;

        for chunk in samples.chunks(self.window_samples) {
            if chunk.len() < self.window_samples {
                break;
            }
            total_windows += 1;

            let energy = self.calculate_energy(chunk);
            if energy < VAD_ENERGY_THRESHOLD {
                continue; // Skip silent windows
            }

            let voice_ratio = self.calculate_voice_ratio(chunk);
            if voice_ratio >= VAD_VOICE_RATIO_THRESHOLD {
                voice_windows += 1;
            }
        }

        let voice_percent = if total_windows > 0 {
            voice_windows as f32 / total_windows as f32
        } else {
            0.0
        };

        let has_voice = voice_percent >= MIN_VOICE_RATIO_FOR_SPEECH;
        (has_voice, voice_percent)
    }

    pub fn reset(&mut self) {
        self.silent_windows = 0;
        self.speech_confirm_count = 0;
        self.in_speech = false;
        self.phrase_start = 0;
        self.processed_pos = 0;
        self.voice_ratio = 0.0;
        self.voice_windows_count = 0;
        self.phrase_windows_count = 0;
        self.last_transcribed_end = 0;
        self.buffered_start = None;
    }
}
