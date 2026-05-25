mod ui_types;
mod logger;
mod worker;
mod worker_config;
mod market_data;
mod ui;

use std::fs::OpenOptions;
use tracing_subscriber::{fmt::writer::MakeWriterExt, layer::SubscriberExt, util::SubscriberInitExt, EnvFilter};
use ui_types::{UiCommand, WorkerUpdate};
use worker_config::{PollConfig, SharedPollConfig, Queue};
use logger::GuiLogger;
use worker::PolymarketWorker;
use ui::PolymarketDashboardApp;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // Gracefully handle missing .env files
    let _ = dotenv::dotenv();

    let poll_config: SharedPollConfig =
        std::sync::Arc::new(PollConfig::new());

    // 1. Initialize bounded communication channels
    let (cmd_tx, cmd_rx) = tokio::sync::mpsc::channel::<UiCommand>(100);
    let (update_tx, update_rx) = tokio::sync::mpsc::channel::<WorkerUpdate>(100);

    let env_filter = EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| EnvFilter::new("info,polymarket_node=trace"));

    // -----------------------------------------------------------------
    // Pipeline Telemetry Setup
    // -----------------------------------------------------------------
    let file = OpenOptions::new()
        .create(true)
        .append(true)
        .open("polymarket.log")?;

    let file_writer = tracing_subscriber::fmt::layer()
        .with_writer(file.with_max_level(tracing::Level::TRACE))
        .with_ansi(false)
        .with_timer(tracing_subscriber::fmt::time::ChronoLocal::rfc_3339());

    let stdout_layer = tracing_subscriber::fmt::layer().with_level(true);
    let gui_layer = GuiLogger { tx: update_tx.clone() };

    tracing_subscriber::registry()
        .with(env_filter)
        .with(stdout_layer)
        .with(file_writer)
        //.with(gui_layer)
        .init();

    tracing::info!("Polymarket Advanced Trading Node Client Bootstrap Base Running...");

    // 2. Window presentation configuration parameters
    let native_options = eframe::NativeOptions {
        //renderer: eframe::Renderer::Wgpu,
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([1100.0, 750.0])
            .with_min_inner_size([800.0, 500.0]),
        ..Default::default()
    };

    // --- FIX: Spawn the worker out here before eframe blocks the current thread context ---
    let worker_update_tx = update_tx.clone();
    
    // We will extract the egui context later inside the App initialization, 
    // so we pass a clone of the channel pairs out here.
    let mut worker = PolymarketWorker {
        cmd_rx,
        update_tx: worker_update_tx,
        ctx: egui::Context::default(), // Will be updated dynamically by the worker structure
        clob_client: std::sync::Arc::new(tokio::sync::Mutex::new(None)),
        poll_config: poll_config.clone(),
        market_tasks:
            std::sync::Arc::new(
                tokio::sync::Mutex::new(
                    std::collections::HashMap::new()
                )
            ),
    };

    // 3. Kick off native engine runtime loops
    eframe::run_native(
        "Polymarket SDK Controller App Console",
        native_options,
        Box::new(move |cc| {
            
            cc.egui_ctx.set_visuals(
                egui::Visuals::dark()
            );

            let mut style =
                (*cc.egui_ctx.style()).clone();

            style.spacing.item_spacing =
                egui::vec2(8.0, 8.0);

            style.visuals.window_corner_radius =
                8.0.into();

            cc.egui_ctx.set_style(style);

            // Update the worker with the valid runtime UI context instance
            worker.ctx = cc.egui_ctx.clone();

            // Spawn safely on the active runtime
            tokio::spawn(async move {
                worker.run().await;
            });

            Ok(Box::new(
                PolymarketDashboardApp::new(
                    cc,
                    cmd_tx,
                    update_rx,
                    poll_config,
                )
            ))
        }),
    )
    .map_err(|e| anyhow::anyhow!("Eframe running failure: {:?}", e))?;

    Ok(())
}