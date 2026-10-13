mod app;
mod file;
mod tree;

fn main() -> eframe::Result<()> {
    let native_options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_title("❄ Snowflake — Story Outliner")
            .with_inner_size([1280.0, 800.0])
            .with_min_inner_size([640.0, 480.0])
            .with_icon(load_icon()),
        ..Default::default()
    };
    eframe::run_native(
        "Snowflake",
        native_options,
        Box::new(|cc| Ok(Box::new(app::SnowflakeApp::new(cc)))),
    )
}

fn load_icon() -> egui::IconData {
    // Simple programmatic snowflake icon (32×32 RGBA)
    let size = 32usize;
    let mut rgba = vec![0u8; size * size * 4];
    let cx = size as f32 / 2.0;
    let cy = size as f32 / 2.0;
    for y in 0..size {
        for x in 0..size {
            let dx = x as f32 - cx;
            let dy = y as f32 - cy;
            let r = (dx * dx + dy * dy).sqrt();
            // Draw a simple circle silhouette
            let alpha = if r < 14.0 { 200u8 } else { 0u8 };
            let idx = (y * size + x) * 4;
            rgba[idx] = 120;
            rgba[idx + 1] = 90;
            rgba[idx + 2] = 220;
            rgba[idx + 3] = alpha;
        }
    }
    egui::IconData {
        rgba,
        width: size as u32,
        height: size as u32,
    }
}
