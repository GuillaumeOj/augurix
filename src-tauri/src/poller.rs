use std::time::Duration;
use tauri::{AppHandle, Emitter};

pub fn spawn(app: AppHandle) {
    std::thread::spawn(move || {
        let rt = match tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
        {
            Ok(rt) => rt,
            Err(e) => {
                eprintln!("poller runtime error: {e}");
                return;
            }
        };
        rt.block_on(async move {
            let mut interval = tokio::time::interval(Duration::from_secs(15));
            // Skip the immediate first tick to avoid hammering on startup
            interval.tick().await;
            loop {
                interval.tick().await;
                let _ = app.emit("status-tick", ());
            }
        });
    });
}
