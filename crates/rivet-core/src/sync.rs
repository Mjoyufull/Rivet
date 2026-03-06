use matrix_sdk_ui::sync_service::SyncService;
use std::sync::Arc;
use tracing::info;

pub async fn run_sync_state_monitor(sync_service: Arc<SyncService>) {
    let mut state_stream = sync_service.state();

    while let Some(state) = state_stream.next().await {
        info!("Sync state changed to: {:?}", state);
        // Here we could handle state changes, e.g. retrying on error idk
    }
}
