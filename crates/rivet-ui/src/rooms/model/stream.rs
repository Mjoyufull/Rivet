use super::RoomInfo;
use super::RoomListModel;
use crate::rooms::model::enrichment::process_room_items;
use futures::StreamExt;
use gpui::*;
use matrix_sdk_ui::eyeball_im::VectorDiff;
use rivet_core::client::RivetClient;

fn apply_diff_update(cx: &mut AsyncApp, model: &Entity<RoomListModel>, diff: VectorDiff<RoomInfo>) {
    let _ = cx.update(|cx| {
        let _ = model.update(cx, |this, cx| {
            this.apply_diff(diff);
            cx.notify();
        });
    });
}

fn apply_diff_updates(
    cx: &mut AsyncApp,
    model: &Entity<RoomListModel>,
    diffs: Vec<VectorDiff<RoomInfo>>,
) {
    if diffs.is_empty() {
        return;
    }

    let _ = cx.update(|cx| {
        let _ = model.update(cx, |this, cx| {
            for diff in diffs {
                this.apply_diff(diff);
            }
            cx.notify();
        });
    });
}

pub(super) fn init(model: Entity<RoomListModel>, client: RivetClient, cx: &mut App) {
    cx.spawn(|cx: &mut AsyncApp| {
        let mut cx = cx.clone();
        async move {
            tracing::info!("Initializing Reactive RoomListModel");

            let room_list_service = client.room_list_service().await;
            let Some(service) = room_list_service else {
                tracing::error!("No RoomListService available!");
                return;
            };

            tracing::info!("Got RoomListService, getting all_rooms...");
            let room_list = match service.all_rooms().await {
                Ok(list) => {
                    tracing::info!("Got room list successfully");
                    list
                }
                Err(e) => {
                    tracing::error!("Failed to get all_rooms: {:?}", e);
                    return;
                }
            };

            let (stream, controller) = room_list.entries_with_dynamic_adapters(100);
            controller.set_filter(Box::new(|_| true));

            let _ = model.update(&mut cx, |this, _| {
                this._room_list_controller = Some(controller);
            });

            let mut stream = std::pin::pin!(stream);
            tracing::info!("Starting room list stream listener...");

            while let Some(diffs) = stream.next().await {
                tracing::info!("Received {} diffs from room list stream", diffs.len());

                let mut passthrough_diffs = Vec::new();
                for diff in diffs {
                    match diff {
                        VectorDiff::Reset { values } => {
                            tracing::info!("VectorDiff::Reset with {} rooms", values.len());
                            let processed = process_room_items(values.into_iter().collect()).await;
                            apply_diff_update(
                                &mut cx,
                                &model,
                                VectorDiff::Reset {
                                    values: processed.into(),
                                },
                            );
                        }
                        VectorDiff::Append { values } => {
                            let processed = process_room_items(values.into_iter().collect()).await;
                            apply_diff_update(
                                &mut cx,
                                &model,
                                VectorDiff::Append {
                                    values: processed.into(),
                                },
                            );
                        }
                        VectorDiff::PushBack { value } => {
                            let processed = process_room_items(vec![value]).await;
                            if let Some(item) = processed.into_iter().next() {
                                passthrough_diffs.push(VectorDiff::PushBack { value: item });
                            }
                        }
                        VectorDiff::PushFront { value } => {
                            let processed = process_room_items(vec![value]).await;
                            if let Some(item) = processed.into_iter().next() {
                                passthrough_diffs.push(VectorDiff::PushFront { value: item });
                            }
                        }
                        VectorDiff::Insert { index, value } => {
                            let processed = process_room_items(vec![value]).await;
                            if let Some(item) = processed.into_iter().next() {
                                passthrough_diffs.push(VectorDiff::Insert { index, value: item });
                            }
                        }
                        VectorDiff::Set { index, value } => {
                            let processed = process_room_items(vec![value]).await;
                            if let Some(item) = processed.into_iter().next() {
                                passthrough_diffs.push(VectorDiff::Set { index, value: item });
                            }
                        }
                        VectorDiff::Remove { index } => {
                            passthrough_diffs.push(VectorDiff::Remove { index });
                        }
                        VectorDiff::PopFront => passthrough_diffs.push(VectorDiff::PopFront),
                        VectorDiff::PopBack => passthrough_diffs.push(VectorDiff::PopBack),
                        VectorDiff::Clear => passthrough_diffs.push(VectorDiff::Clear),
                        VectorDiff::Truncate { length } => {
                            passthrough_diffs.push(VectorDiff::Truncate { length });
                        }
                    }
                }

                apply_diff_updates(&mut cx, &model, passthrough_diffs);
            }

            tracing::warn!("Room list stream ended unexpectedly");
        }
    })
    .detach();
}
