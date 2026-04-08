use wasm_bindgen::prelude::*;
use wasm_bindgen_futures::JsFuture;
use web_sys::{
    self, AnalyserNode, AudioContext, HtmlCanvasElement, MediaStream, MediaStreamConstraints,
};

// Piano roll constants
const A4_FREQUENCY: f64 = 440.0;
const MIDI_A4: u8 = 69;
const MIN_MIDI_NOTE: u8 = 21; // A0
const MAX_MIDI_NOTE: u8 = 108; // C8
const WHITE_KEYS: [bool; 12] = [
    true, false, true, false, true, true, false, true, false, true, false, true,
];

#[wasm_bindgen]
pub struct Spectrogram {
    context: AudioContext,
    analyser: AnalyserNode,
    time_canvas: HtmlCanvasElement,
    freq_canvas: HtmlCanvasElement,
    waterfall_canvas: HtmlCanvasElement,
    highlight_canvas: Option<HtmlCanvasElement>, // Separate canvas for highlights
    time_data: Vec<u8>,
    freq_data: Vec<u8>,
    waterfall_x: f64,
    piano_roll_width: f64,
    min_frequency: f64,
    max_frequency: f64,
    highlighted_frequency: Option<f64>,
    need_full_redraw: bool,
}

#[wasm_bindgen]
impl Spectrogram {
    #[wasm_bindgen(constructor)]
    pub fn new(
        time_canvas_id: &str,
        freq_canvas_id: &str,
        waterfall_canvas_id: &str,
    ) -> Result<Spectrogram, JsValue> {
        console_error_panic_hook::set_once();

        let window = web_sys::window().ok_or("no window found")?;
        let document = window.document().ok_or("no document found")?;
        let time_canvas = document
            .get_element_by_id(time_canvas_id)
            .ok_or("time canvas not found")?
            .dyn_into::<HtmlCanvasElement>()?;
        let freq_canvas = document
            .get_element_by_id(freq_canvas_id)
            .ok_or("freq canvas not found")?
            .dyn_into::<HtmlCanvasElement>()?;

        let waterfall_canvas = document
            .get_element_by_id(waterfall_canvas_id)
            .ok_or("waterfall canvas not found")?
            .dyn_into::<HtmlCanvasElement>()?;

        let context = AudioContext::new()?;
        let analyser = context.create_analyser()?;
        analyser.set_fft_size(2048);

        let time_data = vec![0; analyser.frequency_bin_count() as usize];
        let freq_data = vec![0; analyser.frequency_bin_count() as usize];

        // Create highlight canvas (not attached to DOM)
        let highlight_canvas = document
            .create_element("canvas")?
            .dyn_into::<HtmlCanvasElement>()?;
        highlight_canvas.set_width(waterfall_canvas.width());
        highlight_canvas.set_height(waterfall_canvas.height());

        Ok(Spectrogram {
            context,
            analyser,
            time_canvas,
            freq_canvas,
            waterfall_canvas,
            highlight_canvas: Some(highlight_canvas),
            time_data,
            freq_data,
            waterfall_x: 0.0,
            piano_roll_width: 40.0, // Width of piano roll in pixels
            min_frequency: 27.5,    // A0 frequency
            max_frequency: 4186.01, // C8 frequency
            highlighted_frequency: None,
            need_full_redraw: true,
        })
    }

    // Set highlighted frequency based on mouse position
    #[wasm_bindgen]
    pub fn set_highlighted_frequency(&mut self, y: f64) {
        // Just update highlighted frequency
        if y < 0.0 {
            self.highlighted_frequency = None;
        } else {
            // Convert y-position to frequency
            let height = self.waterfall_canvas.height() as f64;
            let frequency = self.y_position_to_frequency(y, height);
            self.highlighted_frequency = Some(frequency);
        }

        // No full redraw needed, we'll handle this in draw_frame
    }

    // Convert y position to frequency (inverse of frequency_to_y_position)
    fn y_position_to_frequency(&self, y: f64, height: f64) -> f64 {
        let log_min = self.min_frequency.ln();
        let log_max = self.max_frequency.ln();

        // Invert the y-to-frequency mapping
        let log_freq = log_min + (1.0 - y / height) * (log_max - log_min);
        log_freq.exp()
    }

    pub async fn start(&mut self) -> Result<(), JsValue> {
        let window = web_sys::window().unwrap();
        let navigator = window.navigator();
        let media_devices = navigator.media_devices()?;

        let constraints = MediaStreamConstraints::new();
        constraints.set_audio(&JsValue::from_bool(true));

        let media_stream_promise = media_devices.get_user_media_with_constraints(&constraints)?;
        let media_stream = JsFuture::from(media_stream_promise).await?;
        let media_stream: MediaStream = media_stream.dyn_into()?;

        let source = self.context.create_media_stream_source(&media_stream)?;
        source.connect_with_audio_node(&self.analyser)?;

        Ok(())
    }

    // Helper method to convert frequency to MIDI note number
    fn frequency_to_midi_note(&self, freq: f64) -> f64 {
        12.0 * (freq / A4_FREQUENCY).log2() + MIDI_A4 as f64
    }

    // Helper method to convert MIDI note to frequency
    fn midi_note_to_frequency(&self, note: f64) -> f64 {
        A4_FREQUENCY * 2.0_f64.powf((note - MIDI_A4 as f64) / 12.0)
    }

    // Maps a frequency to y position using logarithmic scale
    fn frequency_to_y_position(&self, freq: f64, height: f64) -> f64 {
        let log_min = self.min_frequency.ln();
        let log_max = self.max_frequency.ln();
        let log_freq = freq.ln();

        // Invert y-axis (0 at top, height at bottom)
        height * (1.0 - (log_freq - log_min) / (log_max - log_min))
    }

    // Get note name and octave for a frequency
    fn get_note_info(&self, freq: f64) -> (String, i32) {
        // Convert frequency to MIDI note
        let midi_note = self.frequency_to_midi_note(freq).round() as u8;
        let note_idx = (midi_note % 12) as usize;

        // Get the note name
        let note_name = match note_idx {
            0 => "C".to_string(),
            1 => "C#".to_string(),
            2 => "D".to_string(),
            3 => "D#".to_string(),
            4 => "E".to_string(),
            5 => "F".to_string(),
            6 => "F#".to_string(),
            7 => "G".to_string(),
            8 => "G#".to_string(),
            9 => "A".to_string(),
            10 => "A#".to_string(),
            11 => "B".to_string(),
            _ => "".to_string(),
        };

        let octave = (midi_note / 12) - 1;

        (note_name, octave as i32)
    }

    // Draw the piano roll on the canvas
    fn draw_piano_roll(
        &self,
        ctx: &web_sys::CanvasRenderingContext2d,
        height: f64,
    ) -> Result<(), JsValue> {
        // Draw piano roll background
        ctx.set_fill_style_str("#222");
        ctx.fill_rect(0.0, 0.0, self.piano_roll_width, height);

        // Draw piano keys
        for note in MIN_MIDI_NOTE..=MAX_MIDI_NOTE {
            let note_freq = self.midi_note_to_frequency(note as f64);
            let y = self.frequency_to_y_position(note_freq, height);

            // Calculate the note index (0-11) to determine if it's white or black key
            let note_idx = (note % 12) as usize;
            let is_white = WHITE_KEYS[note_idx];

            // Draw the key
            if is_white {
                ctx.set_fill_style_str("#aaa");
                ctx.fill_rect(0.0, y - 1.0, self.piano_roll_width, 2.0);

                // Draw note name for C notes (and A4 for reference)
                if note_idx == 0 || note == MIDI_A4 {
                    let octave = (note / 12) - 1;
                    let note_name = if note_idx == 0 {
                        format!("C{}", octave)
                    } else {
                        "A4".to_string()
                    };

                    ctx.set_font("10px Arial");
                    ctx.set_fill_style_str("#fff");
                    ctx.set_text_align("left");
                    ctx.fill_text(&note_name, 3.0, y - 3.0)?;
                }
            } else {
                ctx.set_fill_style_str("#666");
                ctx.fill_rect(0.0, y - 0.5, self.piano_roll_width * 0.6, 1.0);
            }
        }

        // Draw dividing line between piano roll and spectrogram
        ctx.set_stroke_style_str("#555");
        ctx.begin_path();
        ctx.move_to(self.piano_roll_width, 0.0);
        ctx.line_to(self.piano_roll_width, height);
        ctx.stroke();

        Ok(())
    }

    // Draw horizontal frequency line
    fn draw_frequency_line(
        &self,
        ctx: &web_sys::CanvasRenderingContext2d,
        freq: f64,
        width: f64,
        height: f64,
    ) -> Result<(), JsValue> {
        let y = self.frequency_to_y_position(freq, height);

        // Get note name and octave
        let (note_name, octave) = self.get_note_info(freq);

        // Draw a horizontal line across the ENTIRE waterfall - use a bright, visible yellow
        ctx.set_stroke_style_str("rgba(255, 255, 0, 0.8)");
        ctx.set_line_width(1.5); // Slightly thicker for visibility
        ctx.begin_path();
        ctx.move_to(0.0, y); // Start from the very left edge
        ctx.line_to(width, y); // Go all the way to the right edge
        ctx.stroke();

        // Draw frequency text with note name
        ctx.set_font("10px Arial");
        ctx.set_fill_style_str("#fff");
        let freq_text = format!("{}{} - {:.1} Hz", note_name, octave, freq);
        ctx.fill_text(&freq_text, self.piano_roll_width + 5.0, y - 5.0)?;

        Ok(())
    }

    pub fn draw_frame(&mut self) -> Result<(), JsValue> {
        // Draw time domain
        let time_ctx = self
            .time_canvas
            .get_context("2d")
            .unwrap()
            .unwrap()
            .dyn_into::<web_sys::CanvasRenderingContext2d>()
            .unwrap();

        let time_width = self.time_canvas.width() as f64;
        let time_height = self.time_canvas.height() as f64;

        // Clear time canvas
        time_ctx.set_fill_style_str("#000");
        time_ctx.fill_rect(0.0, 0.0, time_width, time_height);

        self.analyser.get_byte_time_domain_data(&mut self.time_data);
        time_ctx.set_stroke_style_str("#0f0");
        time_ctx.begin_path();

        let time_slice_width = time_width / self.time_data.len() as f64;
        let mut x = 0.0;

        for &value in self.time_data.iter() {
            let y = (value as f64 / 128.0) * time_height;
            if x == 0.0 {
                time_ctx.move_to(x, y);
            } else {
                time_ctx.line_to(x, y);
            }
            x += time_slice_width;
        }
        time_ctx.stroke();

        // Draw frequency domain
        let freq_ctx = self
            .freq_canvas
            .get_context("2d")
            .unwrap()
            .unwrap()
            .dyn_into::<web_sys::CanvasRenderingContext2d>()
            .unwrap();

        let freq_width = self.freq_canvas.width() as f64;
        let freq_height = self.freq_canvas.height() as f64;

        // Clear frequency canvas
        freq_ctx.set_fill_style_str("#000");
        freq_ctx.fill_rect(0.0, 0.0, freq_width, freq_height);

        self.analyser.get_byte_frequency_data(&mut self.freq_data);

        let bar_width = freq_width / self.freq_data.len() as f64;
        let mut x = 0.0;

        for &value in self.freq_data.iter() {
            let bar_height = (value as f64 / 255.0) * freq_height;

            let hue = x / freq_width * 360.0;
            freq_ctx.set_fill_style_str(&format!("hsl({}, 100%, {}%)", hue, 50.0));

            freq_ctx.fill_rect(x, freq_height - bar_height, bar_width, bar_height);

            x += bar_width;
        }

        // Draw waterfall plot
        let waterfall_ctx = self
            .waterfall_canvas
            .get_context("2d")
            .unwrap()
            .unwrap()
            .dyn_into::<web_sys::CanvasRenderingContext2d>()
            .unwrap();

        let waterfall_width = self.waterfall_canvas.width() as f64;
        let waterfall_height = self.waterfall_canvas.height() as f64;

        // Handle redrawing
        if self.need_full_redraw {
            // Only do a full reset when we've reached the edge
            waterfall_ctx.set_fill_style_str("#000");
            waterfall_ctx.fill_rect(0.0, 0.0, waterfall_width, waterfall_height);
            self.need_full_redraw = false;
        } else {
            // Normal operation - clear the piano roll area
            waterfall_ctx.set_fill_style_str("#000");
            waterfall_ctx.fill_rect(0.0, 0.0, self.piano_roll_width, waterfall_height);
            // Clear the current column
            waterfall_ctx.fill_rect(self.waterfall_x, 0.0, 1.0, waterfall_height);
        }

        // Draw piano roll
        self.draw_piano_roll(&waterfall_ctx, waterfall_height)?;

        // First create a copy of the current state
        // Create a temporary canvas for saving/restoring the waterfall state without highlights
        let document = web_sys::window().unwrap().document().unwrap();
        let temp_canvas = document
            .create_element("canvas")
            .unwrap()
            .dyn_into::<HtmlCanvasElement>()
            .unwrap();
        temp_canvas.set_width(self.waterfall_canvas.width());
        temp_canvas.set_height(self.waterfall_canvas.height());

        // Copy the current state to the temp canvas
        let temp_ctx = temp_canvas
            .get_context("2d")
            .unwrap()
            .unwrap()
            .dyn_into::<web_sys::CanvasRenderingContext2d>()
            .unwrap();
        temp_ctx.draw_image_with_html_canvas_element(&self.waterfall_canvas, 0.0, 0.0)?;

        // Now draw the highlighted frequency line if active
        if let Some(freq) = self.highlighted_frequency {
            // Draw the line across the entire waterfall width
            self.draw_frequency_line(&waterfall_ctx, freq, waterfall_width, waterfall_height)?;
        }

        // Reset x position when reaching the end
        if self.waterfall_x >= waterfall_width {
            // Mark for full redraw on next frame - we need to wrap around
            self.need_full_redraw = true;
            // Reset position
            self.waterfall_x = self.piano_roll_width;
        } else if self.waterfall_x < self.piano_roll_width {
            self.waterfall_x = self.piano_roll_width;
        }

        // Clear just the current column where we'll draw new data
        waterfall_ctx.set_fill_style_str("#000");
        waterfall_ctx.fill_rect(self.waterfall_x, 0.0, 1.0, waterfall_height);

        // Calculate height of each frequency bin in pixels
        // Note: we're using logarithmic frequency mapping for the piano roll
        // but the FFT data still uses linear mapping
        let bar_height = waterfall_height / self.freq_data.len() as f64;

        // Draw new line at current x position
        for (i, &value) in self.freq_data.iter().rev().enumerate() {
            let y = i as f64 * bar_height;
            let normalized_value = value as f64 / 255.0;
            let hue = 240.0 * (1.0 - normalized_value); // Blue (240) to Red (0)
            waterfall_ctx.set_fill_style_str(&format!("hsl({}, 100%, {}%)", hue, 50.0));
            waterfall_ctx.fill_rect(self.waterfall_x, y, 1.0, bar_height);
        }

        // Move to next x position
        self.waterfall_x += 1.0;

        // If we have a highlighted frequency, we need to get a clean copy of the current waterfall
        // state without the highlight for the next frame
        if self.highlighted_frequency.is_some() {
            // We already have a temp_canvas with the clean state from before highlighting
            // Store it in our highlight_canvas for the next frame
            if let Some(highlight_canvas) = &self.highlight_canvas {
                let highlight_ctx = highlight_canvas
                    .get_context("2d")
                    .unwrap()
                    .unwrap()
                    .dyn_into::<web_sys::CanvasRenderingContext2d>()
                    .unwrap();

                // Clear it first
                highlight_ctx.clear_rect(0.0, 0.0, waterfall_width, waterfall_height);

                // Then copy the clean version from temp_canvas
                highlight_ctx.draw_image_with_html_canvas_element(&temp_canvas, 0.0, 0.0)?;
            }
        }

        Ok(())
    }
}
