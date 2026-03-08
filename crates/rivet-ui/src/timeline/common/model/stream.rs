use super::*;

pub(crate) fn install_scroll_handler(model: &Entity<TimelineModel>, cx: &App) {
    let weak_model = model.downgrade();
    model
        .read(cx)
        .list_state
        .set_scroll_handler(move |event, _window, cx| {
            if event.visible_range.start > HISTORY_PREFETCH_THRESHOLD {
                return;
            }

            let Some(model) = weak_model.upgrade() else {
                return;
            };

            let handle = model.clone();
            let _ = model.update(cx, |this, cx| {
                this.load_more_history(handle.clone(), cx);
            });
        });
}

pub(crate) fn load_more_history(
    this: &mut TimelineModel,
    model_handle: Entity<TimelineModel>,
    cx: &mut App,
) {
    if this.loading_history || this.hit_timeline_start {
        return;
    }

    this.loading_history = true;
    cx.notify(model_handle.entity_id());

    let timeline = this.timeline.clone();
    let weak_model = model_handle.downgrade();

    cx.spawn(|cx: &mut AsyncApp| {
        let mut cx = cx.clone();
        async move {
            let result = timeline.paginate_backwards(HISTORY_BATCH_SIZE).await;

            if let Some(model) = weak_model.upgrade() {
                let _ = model.update(&mut cx, |this, cx: &mut Context<TimelineModel>| {
                    this.loading_history = false;
                    match result {
                        Ok(hit_timeline_start) => {
                            this.hit_timeline_start |= hit_timeline_start;
                        }
                        Err(e) => {
                            tracing::error!("Failed to paginate backwards: {:?}", e);
                        }
                    }
                    cx.notify();
                });
            }
        }
    })
    .detach();
}

pub(crate) fn init(model: Entity<TimelineModel>, cx: &mut App) {
    let async_cx = cx.to_async();
    let weak_model = model.downgrade();
    let timeline = model.read(cx).timeline.clone();

    async_cx
        .clone()
        .spawn(move |_: &mut AsyncApp| async move {
            let (items, mut stream) = timeline.subscribe().await;

            let homeserver_url = async_cx
                .update(|cx: &mut App| {
                    weak_model
                        .upgrade()
                        .map(|this| this.read(cx).homeserver_url.clone())
                        .unwrap_or_default()
                })
                .unwrap_or_default();
            let processed = formatting::process_items_vector(&items, &homeserver_url).await;

            let _ = async_cx.update(|cx: &mut App| {
                let _ = weak_model.update(cx, |this, cx: &mut Context<TimelineModel>| {
                    this.items = items;
                    this.rendered_items = processed;
                    update_list_state(this, cx);
                });
            });

            let _ = async_cx.update(|cx: &mut App| {
                let _ = weak_model.update(cx, |this, cx| {
                    this.loading_history = true;
                    cx.notify();
                });
            });

            let initial_prefetch = timeline
                .paginate_backwards(INITIAL_HISTORY_BATCH_SIZE)
                .await;
            if let Ok(hit_timeline_start) = initial_prefetch {
                let _ = async_cx.update(|cx: &mut App| {
                    let _ = weak_model.update(cx, |this, cx| {
                        this.hit_timeline_start |= hit_timeline_start;
                        this.loading_history = false;
                        cx.notify();
                    });
                });
            } else if let Err(e) = initial_prefetch {
                tracing::error!("Failed initial paginate backwards: {:?}", e);
                let _ = async_cx.update(|cx: &mut App| {
                    let _ = weak_model.update(cx, |this, cx| {
                        this.loading_history = false;
                        cx.notify();
                    });
                });
            }

            while let Some(diffs) = stream.next().await {
                let mut current_items = Vector::new();
                let mut success = false;
                let _ = async_cx.update(|cx: &mut App| {
                    if let Ok(items) = weak_model.update(cx, |this, _| this.items.clone()) {
                        current_items = items;
                        success = true;
                    }
                });
                if !success {
                    break;
                }

                for diff in diffs {
                    match diff {
                        VectorDiff::Append { values } => current_items.append(values),
                        VectorDiff::Clear => current_items.clear(),
                        VectorDiff::PushFront { value } => current_items.push_front(value),
                        VectorDiff::PushBack { value } => current_items.push_back(value),
                        VectorDiff::PopFront => {
                            current_items.pop_front();
                        }
                        VectorDiff::PopBack => {
                            current_items.pop_back();
                        }
                        VectorDiff::Insert { index, value } => {
                            if index <= current_items.len() {
                                current_items.insert(index, value);
                            }
                        }
                        VectorDiff::Set { index, value } => {
                            if index < current_items.len() {
                                current_items.set(index, value);
                            }
                        }
                        VectorDiff::Remove { index } => {
                            if index < current_items.len() {
                                current_items.remove(index);
                            }
                        }
                        VectorDiff::Truncate { length } => current_items.truncate(length),
                        VectorDiff::Reset { values } => current_items = values,
                    }
                }

                let homeserver_url = async_cx
                    .update(|cx: &mut App| {
                        weak_model
                            .update(cx, |this, _| this.homeserver_url.clone())
                            .unwrap_or_default()
                    })
                    .unwrap_or_default();
                let processed =
                    formatting::process_items_vector(&current_items, &homeserver_url).await;
                let items_to_save = current_items;

                let _ = async_cx.update(|cx: &mut App| {
                    let _ = weak_model.update(cx, |this, cx| {
                        this.items = items_to_save;
                        this.rendered_items = processed;
                        update_list_state(this, cx);
                    });
                });
            }
        })
        .detach();
}

pub(crate) fn update_list_state(this: &mut TimelineModel, cx: &mut Context<TimelineModel>) {
    let new_count = this.rendered_items.len();
    let old_count = this.list_state.item_count();
    tracing::info!(
        "timeline: syncing list state old_count={} new_count={} loading_history={}",
        old_count,
        new_count,
        this.loading_history
    );

    if old_count == new_count {
        cx.notify();
        return;
    }

    if new_count > old_count {
        let delta = new_count - old_count;
        if this.loading_history {
            this.list_state.splice(0..0, delta);
        } else {
            this.list_state.splice(old_count..old_count, delta);
        }
    } else {
        let delta = old_count - new_count;
        if this.loading_history {
            this.list_state.splice(0..delta, 0);
        } else {
            this.list_state.splice(new_count..old_count, 0);
        }
    }

    cx.notify();
}
