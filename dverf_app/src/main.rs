mod backend;

use std::sync::Arc;

use eframe::{Frame, egui, egui::Context};

use crate::backend::Backend;

fn main() -> eframe::Result {
	trace::setup();

	let backend = Arc::new(Backend::default());

	smol::spawn({
		let backend = Arc::clone(&backend);
		backend.run()
	})
	.detach();

	let options = eframe::NativeOptions {
		viewport: egui::ViewportBuilder::default().with_inner_size([1280.0, 1024.0]).with_drag_and_drop(true),

		..Default::default()
	};

	eframe::run_native("Dverf", options, Box::new(|cc| Ok(Box::new(App::new(cc, backend)))))
}

struct App {
	backend: Arc<Backend>,
}

impl App {
	pub fn new(_cc: &eframe::CreationContext, backend: Arc<Backend>) -> Self {
		Self { backend }
	}
}

impl eframe::App for App {
	fn update(&mut self, ctx: &Context, _frame: &mut Frame) {
		egui::TopBottomPanel::top("top_bar").frame(egui::Frame::new().inner_margin(4)).show(ctx, |ui| {
			ui.horizontal_wrapped(|ui| {
				self.bar_contents(ui);
			});
		});

		egui::CentralPanel::default().show(ctx, |_ui| {
			egui::Window::new("Device").show(ctx, |ui| {
				if let Some(descr) = self.backend.device_description() {
					egui::Grid::new("device_info").striped(true).show(ui, |ui| {
						ui.label("Title");
						ui.label(descr.info.product_string().unwrap_or("unknown"));
						ui.end_row();

						ui.label("Manufacturer");
						ui.label(descr.info.manufacturer_string().unwrap_or("unknown"));
						ui.end_row();

						ui.label("Product Id");
						ui.label(format!("{:04x}", descr.info.product_id()));
						ui.end_row();

						ui.label("Board Id");
						ui.label(descr.board_id.to_string());
						ui.end_row();

						ui.label("Board Rev");
						ui.label(descr.board_rev.to_string());
						ui.end_row();

						ui.label("Fw Version");
						ui.label(&descr.version);
						ui.end_row();
					});
				} else {
					ui.vertical_centered(|ui| {
						ui.label("No devices connected");
					});
				}
			});
		});
	}

	fn clear_color(&self, visuals: &egui::Visuals) -> [f32; 4] {
		let color = egui::lerp(egui::Rgba::from(visuals.panel_fill)..=egui::Rgba::from(visuals.extreme_bg_color), 0.5);
		let color = egui::Color32::from(color);
		color.to_normalized_gamma_f32()
	}
}

impl App {
	fn bar_contents(&self, ui: &mut egui::Ui) {
		egui::widgets::global_theme_preference_switch(ui);
		ui.separator();
		ui.label("Dverf - SDR Software");
	}
}

mod trace {
	use tracing::Subscriber;
	use tracing_subscriber::{
		EnvFilter, Layer,
		filter::{LevelFilter, ParseError},
		fmt::format::FmtSpan,
		layer::SubscriberExt,
		util::SubscriberInitExt,
	};

	fn env_filter() -> Result<EnvFilter, ParseError> {
		let directive = LevelFilter::INFO.into();

		let filter = EnvFilter::builder().with_default_directive(directive).from_env_lossy();

		Ok(filter)
	}

	fn new_fmt<S>() -> Box<dyn Layer<S> + Send + Sync>
	where
		S: Subscriber + for<'a> tracing_subscriber::registry::LookupSpan<'a> + Send + Sync,
	{
		let filter = env_filter().expect("Failed to build `EnvFilter`");

		tracing_subscriber::fmt::layer()
			.compact()
			.with_ansi(true)
			.with_span_events(FmtSpan::NONE)
			.with_writer(std::io::stderr)
			.with_filter(filter)
			.boxed()
	}

	pub(crate) fn setup() {
		tracing_subscriber::registry().with(new_fmt()).init();
	}
}
