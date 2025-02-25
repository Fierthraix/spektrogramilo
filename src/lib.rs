use wasm_bindgen::prelude::*;
use wasm_bindgen_futures::JsFuture;
use web_sys::{
    self, AnalyserNode, AudioContext, HtmlCanvasElement, MediaStream, MediaStreamConstraints,
};

#[wasm_bindgen]
pub struct Spectrogram {
    context: AudioContext,
    analyser: AnalyserNode,
    time_canvas: HtmlCanvasElement,
    freq_canvas: HtmlCanvasElement,
    time_data: Vec<u8>,
    freq_data: Vec<u8>,
}

#[wasm_bindgen]
impl Spectrogram {
    #[wasm_bindgen(constructor)]
    pub fn new(time_canvas_id: &str, freq_canvas_id: &str) -> Result<Spectrogram, JsValue> {
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

        let context = AudioContext::new()?;
        let analyser = context.create_analyser()?;
        analyser.set_fft_size(2048);

        let time_data = vec![0; analyser.frequency_bin_count() as usize];
        let freq_data = vec![0; analyser.frequency_bin_count() as usize];

        Ok(Spectrogram {
            context,
            analyser,
            time_canvas,
            freq_canvas,
            time_data,
            freq_data,
        })
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
        Ok(())
    }
}
