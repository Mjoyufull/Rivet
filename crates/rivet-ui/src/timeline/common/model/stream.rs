use super::*;
use crate::perf;
use std::time::{Duration, Instant};

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
    if this.history_request_in_flight() || this.hit_timeline_start {
        return;
    }

    this.loading_history = true;
    cx.notify(model_handle.entity_id());

    let batch_size = this.next_history_batch_size();
    let timeline = this.timeline.clone();
    let weak_model = model_handle.downgrade();

    cx.spawn(move |cx: &mut AsyncApp| {
        let mut cx = cx.clone();
        async move {
            let paginate_started = Instant::now();
            let result = timeline.paginate_backwards(batch_size).await;
            let elapsed = paginate_started.elapsed();
            perf::log_if_slow(
                "timeline.paginate_backwards",
                paginate_started,
                Duration::from_millis(40),
                || match &result {
                    Ok(hit_timeline_start) => format!(
                        "batch_size={} hit_timeline_start={hit_timeline_start}",
                        batch_size
                    ),
                    Err(error) => format!("batch_size={} error={error:?}", batch_size),
                },
            );

            if let Some(model) = weak_model.upgrade() {
                let _ = model.update(&mut cx, |this, cx: &mut Context<TimelineModel>| {
                    this.loading_history = false;
                    match result {
                        Ok(hit_timeline_start) => {
                            this.record_history_page_result(elapsed);
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

fn spawn_initial_history_warmup(
    async_cx: AsyncApp,
    weak_model: WeakEntity<TimelineModel>,
    timeline: Arc<Timeline>,
    room: MatrixRoom,
) {
    async_cx
        .clone()
        .spawn(move |cx: &mut AsyncApp| {
            let mut cx = cx.clone();
            async move {
                cx.background_executor()
                    .timer(Duration::from_millis(INITIAL_HISTORY_WARMUP_DELAY_MS))
                    .await;

                for pass in 0..INITIAL_HISTORY_WARMUP_MAX_PASSES {
                    let mut should_continue = false;
                    let mut batch_size = 0;
                    let _ = async_cx.update(|cx: &mut App| {
                        let _ = weak_model.update(cx, |this, _| {
                            if this.hit_timeline_start
                                || this.loading_history
                                || this.rendered_items.len()
                                    >= INITIAL_HISTORY_WARMUP_TARGET_RENDERED_ITEMS
                            {
                                this.warming_history = false;
                                return;
                            }

                            batch_size = initial_history_warmup_batch_size(
                                this.rendered_items.len(),
                                pass,
                            );
                            if batch_size == 0 {
                                this.warming_history = false;
                                return;
                            }

                            this.warming_history = true;
                            should_continue = true;
                        });
                    });

                    if !should_continue {
                        return;
                    }

                    let paginate_started = Instant::now();
                    let result = timeline.paginate_backwards(batch_size).await;
                    let elapsed = paginate_started.elapsed();
                    perf::log_if_slow(
                        "timeline.init.initial_paginate",
                        paginate_started,
                        Duration::from_millis(40),
                        || match &result {
                            Ok(hit_timeline_start) => format!(
                                "room={} pass={} batch_size={} hit_timeline_start={hit_timeline_start}",
                                room.room_id(),
                                pass + 1,
                                batch_size
                            ),
                            Err(error) => format!(
                                "room={} pass={} batch_size={} error={error:?}",
                                room.room_id(),
                                pass + 1,
                                batch_size
                            ),
                        },
                    );

                    let mut stop = false;
                    if let Some(model) = weak_model.upgrade() {
                        let _ = model.update(&mut cx, |this, _| match result {
                            Ok(hit_timeline_start) => {
                                this.record_history_page_result(elapsed);
                                this.hit_timeline_start |= hit_timeline_start;
                                stop = this.hit_timeline_start
                                    || this.rendered_items.len()
                                        >= INITIAL_HISTORY_WARMUP_TARGET_RENDERED_ITEMS
                                    || elapsed
                                        > Duration::from_millis(
                                            INITIAL_HISTORY_WARMUP_CONTINUE_BUDGET_MS,
                                        );
                                this.warming_history = !stop;
                            }
                            Err(error) => {
                                tracing::error!("Failed initial warmup paginate backwards: {:?}", error);
                                this.warming_history = false;
                                stop = true;
                            }
                        });
                    } else {
                        return;
                    }

                    if stop {
                        return;
                    }
                }

                let _ = async_cx.update(|cx: &mut App| {
                    let _ = weak_model.update(cx, |this, _| {
                        this.warming_history = false;
                    });
                });
            }
        })
        .detach();
}

fn initial_history_warmup_batch_size(rendered_items: usize, pass: usize) -> u16 {
    let missing_items = INITIAL_HISTORY_WARMUP_TARGET_RENDERED_ITEMS.saturating_sub(rendered_items);
    if missing_items == 0 {
        return 0;
    }

    let pass_cap = if pass == 0 {
        INITIAL_HISTORY_WARMUP_FIRST_BATCH_SIZE
    } else {
        INITIAL_HISTORY_WARMUP_FOLLOWUP_BATCH_SIZE
    } as usize;

    missing_items.min(pass_cap) as u16
}

pub(crate) fn init(model: Entity<TimelineModel>, cx: &mut App) {
    let async_cx = cx.to_async();
    let weak_model = model.downgrade();
    let timeline = model.read(cx).timeline.clone();
    let room = model.read(cx).room.clone();

    async_cx
        .clone()
        .spawn(move |_: &mut AsyncApp| async move {
            let subscribe_started = Instant::now();
            let (items, mut stream) = timeline.subscribe().await;
            let (member_lookup, processed) = formatting::process_items_vector(&room, &items).await;
            let render_counts = formatting::compute_render_counts(&items);
            let initial_rendered_count = processed.len();
            perf::log_if_slow(
                "timeline.init.subscribe_render",
                subscribe_started,
                Duration::from_millis(40),
                || format!("room={} raw_items={} rendered_items={}", room.room_id(), items.len(), processed.len()),
            );

            let _ = async_cx.update(|cx: &mut App| {
                let _ = weak_model.update(cx, |this, cx: &mut Context<TimelineModel>| {
                    this.items = items;
                    this.member_lookup = member_lookup;
                    this.incremental_render_counts_available = true;
                    this.render_counts = render_counts;
                    this.rendered_items = processed;
                    update_list_state(this, cx);
                });
            });

            if initial_rendered_count < INITIAL_HISTORY_WARMUP_TARGET_RENDERED_ITEMS {
                spawn_initial_history_warmup(
                    async_cx.clone(),
                    weak_model.clone(),
                    timeline.clone(),
                    room.clone(),
                );
            }

            while let Some(diffs) = stream.next().await {
                let diff_started = Instant::now();
                let diff_count = diffs.len();
                let mut current_items = Vector::new();
                let mut previous_items = Vector::new();
                let mut member_lookup = formatting::MemberLookup::new();
                let mut render_counts = Vec::new();
                let mut incremental_render_counts_available = false;
                let mut success = false;
                let mut prepend_only = true;
                let _ = async_cx.update(|cx: &mut App| {
                    if let Ok((items, lookup, counts, counts_available)) =
                        weak_model.update(cx, |this, _| {
                            (
                                this.items.clone(),
                                this.member_lookup.clone(),
                                this.render_counts.clone(),
                                this.incremental_render_counts_available,
                            )
                        })
                    {
                        previous_items = items.clone();
                        current_items = items;
                        member_lookup = lookup;
                        render_counts = counts;
                        incremental_render_counts_available = counts_available;
                        success = true;
                    }
                });
                if !success {
                    break;
                }

                let mut append_values = Vec::new();
                let mut append_only = true;
                for diff in diffs {
                    match diff {
                        VectorDiff::Append { values } => {
                            prepend_only = false;
                            append_values.extend(values.iter().cloned());
                            current_items.append(values);
                        }
                        VectorDiff::Clear => {
                            append_only = false;
                            prepend_only = false;
                            current_items.clear();
                        }
                        VectorDiff::PushFront { value } => {
                            append_only = false;
                            current_items.push_front(value);
                        }
                        VectorDiff::PushBack { value } => {
                            prepend_only = false;
                            append_values.push(value.clone());
                            current_items.push_back(value);
                        }
                        VectorDiff::PopFront => {
                            append_only = false;
                            prepend_only = false;
                            current_items.pop_front();
                        }
                        VectorDiff::PopBack => {
                            append_only = false;
                            prepend_only = false;
                            current_items.pop_back();
                        }
                        VectorDiff::Insert { index, value } => {
                            append_only = false;
                            if index != 0 {
                                prepend_only = false;
                            }
                            if index <= current_items.len() {
                                current_items.insert(index, value);
                            }
                        }
                        VectorDiff::Set { index, value } => {
                            append_only = false;
                            prepend_only = false;
                            if index < current_items.len() {
                                current_items.set(index, value);
                            }
                        }
                        VectorDiff::Remove { index } => {
                            append_only = false;
                            prepend_only = false;
                            if index < current_items.len() {
                                current_items.remove(index);
                            }
                        }
                        VectorDiff::Truncate { length } => {
                            append_only = false;
                            prepend_only = false;
                            current_items.truncate(length);
                        }
                        VectorDiff::Reset { values } => {
                            append_only = false;
                            prepend_only = false;
                            current_items = values;
                        }
                    }
                }

                if append_only && !append_values.is_empty() {
                    formatting::extend_member_lookup_for_items(
                        &room,
                        &mut member_lookup,
                        append_values.iter(),
                    )
                    .await;

                    if let Some(appended_rendered) = formatting::process_appended_items_with_lookup(
                        &previous_items,
                        &append_values,
                        &member_lookup,
                    ) {
                        let items_to_save = current_items;
                        let appended_vector = append_values.into_iter().collect::<Vec<_>>();
                        let mut appended_items = Vector::new();
                        for item in appended_vector.iter().cloned() {
                            appended_items.push_back(item);
                        }
                        let appended_counts = formatting::compute_render_counts(&appended_items);
                        let previous_raw_len = previous_items.len();
                        let new_raw_len = items_to_save.len();
                        let appended_raw_len = appended_vector.len();
                        let appended_rendered_len = appended_rendered.len();
                        let _ = async_cx.update(|cx: &mut App| {
                            let _ = weak_model.update(cx, |this, cx| {
                                let old_count = this.rendered_items.len();
                                this.items = items_to_save;
                                this.member_lookup = member_lookup;
                                for item in appended_rendered.iter().cloned() {
                                    this.rendered_items.push_back(item);
                                }
                                this.render_counts.extend(appended_counts);
                                this.incremental_render_counts_available = true;
                                let added = this.rendered_items.len().saturating_sub(old_count);
                                if added != 0 {
                                    this.list_state.splice(old_count..old_count, added);
                                }
                                cx.notify();
                            });
                        });
                        perf::log_if_slow(
                            "timeline.diff.apply",
                            diff_started,
                            Duration::from_millis(8),
                            || format!(
                                "path=append diffs={} prev_raw={} new_raw={} appended_raw={} appended_rendered={}",
                                diff_count,
                                previous_raw_len,
                                new_raw_len,
                                appended_raw_len,
                                appended_rendered_len
                            ),
                        );
                        continue;
                    }
                }

                if prepend_only
                    && incremental_render_counts_available
                    && render_counts.len() == previous_items.len()
                    && current_items.len() >= previous_items.len()
                {
                    let prepended_raw = current_items.len().saturating_sub(previous_items.len());
                    if prepended_raw > 0 {
                        let overlap_raw = usize::from(!previous_items.is_empty());
                        let prefix_end = (prepended_raw + overlap_raw).min(current_items.len());
                        let mut prefix_items = Vector::new();
                        for item in current_items.iter().take(prefix_end).cloned() {
                            prefix_items.push_back(item);
                        }

                        formatting::extend_member_lookup_for_items(
                            &room,
                            &mut member_lookup,
                            prefix_items.iter(),
                        )
                        .await;

                        let prefix_rendered =
                            formatting::process_items_vector_with_lookup(&prefix_items, &member_lookup);
                        let prefix_counts = formatting::compute_render_counts(&prefix_items);
                        let old_overlap_raw = overlap_raw;
                        let old_render_remove =
                            render_counts[..old_overlap_raw].iter().sum::<usize>();
                        let previous_raw_len = previous_items.len();
                        let new_raw_len = current_items.len();
                        let prefix_rendered_len = prefix_rendered.len();
                        let items_to_save = current_items;

                        let _ = async_cx.update(|cx: &mut App| {
                            let _ = weak_model.update(cx, |this, cx| {
                                this.items = items_to_save;
                                this.member_lookup = member_lookup;
                                this.render_counts.splice(0..old_overlap_raw, prefix_counts);
                                this.incremental_render_counts_available = true;
                                this.rendered_items = replace_rendered_range(
                                    &this.rendered_items,
                                    0,
                                    old_render_remove,
                                    &prefix_rendered,
                                );

                                this.list_state.splice(0..old_render_remove, prefix_rendered_len);
                                cx.notify();
                            });
                        });
                        perf::log_if_slow(
                            "timeline.diff.apply",
                            diff_started,
                            Duration::from_millis(8),
                            || format!(
                                "path=prepend diffs={} prev_raw={} new_raw={} prepended_raw={} overlap_raw={} old_render_remove={} new_render_insert={}",
                                diff_count,
                                previous_raw_len,
                                new_raw_len,
                                prepended_raw,
                                overlap_raw,
                                old_render_remove,
                                prefix_rendered_len
                            ),
                        );
                        continue;
                    }
                }

                if incremental_render_counts_available && render_counts.len() == previous_items.len()
                {
                    let new_render_counts = formatting::compute_render_counts(&current_items);
                    let (old_changed_range, new_changed_range) =
                        perf::changed_identity_ranges(&previous_items, &current_items);

                    let old_raw_range = perf::expand_incremental_range(
                        &render_counts,
                        old_changed_range.start,
                        old_changed_range.end,
                    );
                    let new_raw_range = perf::expand_incremental_range(
                        &new_render_counts,
                        new_changed_range.start,
                        new_changed_range.end,
                    );

                    if old_raw_range != (0..previous_items.len())
                        || new_raw_range != (0..current_items.len())
                    {
                        let mut slice_items = Vector::new();
                        for item in current_items
                            .iter()
                            .skip(new_raw_range.start)
                            .take(new_raw_range.end.saturating_sub(new_raw_range.start))
                            .cloned()
                        {
                            slice_items.push_back(item);
                        }

                        formatting::extend_member_lookup_for_items(
                            &room,
                            &mut member_lookup,
                            slice_items.iter(),
                        )
                        .await;

                        let slice_rendered = formatting::process_items_vector_with_lookup(
                            &slice_items,
                            &member_lookup,
                        );
                        let slice_counts = new_render_counts[new_raw_range.clone()].to_vec();
                        let old_render_start =
                            render_counts[..old_raw_range.start].iter().sum::<usize>();
                        let old_render_remove =
                            render_counts[old_raw_range.clone()].iter().sum::<usize>();
                        let items_to_save = current_items;
                        let previous_raw_len = previous_items.len();
                        let new_raw_len = items_to_save.len();
                        let slice_rendered_len = slice_rendered.len();

                        let _ = async_cx.update(|cx: &mut App| {
                            let _ = weak_model.update(cx, |this, cx| {
                                this.items = items_to_save;
                                this.member_lookup = member_lookup;
                                this.render_counts
                                    .splice(old_raw_range.clone(), slice_counts);
                                this.incremental_render_counts_available = true;
                                this.rendered_items = replace_rendered_range(
                                    &this.rendered_items,
                                    old_render_start,
                                    old_render_remove,
                                    &slice_rendered,
                                );

                                update_list_state(this, cx);
                            });
                        });
                        perf::log_if_slow(
                            "timeline.diff.apply",
                            diff_started,
                            Duration::from_millis(8),
                            || format!(
                                "path=incremental diffs={} prev_raw={} new_raw={} old_raw_window={}..{} new_raw_window={}..{} old_render_remove={} new_render_insert={}",
                                diff_count,
                                previous_raw_len,
                                new_raw_len,
                                old_raw_range.start,
                                old_raw_range.end,
                                new_raw_range.start,
                                new_raw_range.end,
                                old_render_remove,
                                slice_rendered_len
                            ),
                        );
                        continue;
                    }
                }

                let full_rebuild_started = Instant::now();
                let (member_lookup, processed) =
                    formatting::process_items_vector(&room, &current_items).await;
                let render_counts = formatting::compute_render_counts(&current_items);
                let items_to_save = current_items;
                let rebuilt_raw_len = items_to_save.len();
                let rebuilt_rendered_len = processed.len();

                let _ = async_cx.update(|cx: &mut App| {
                    let _ = weak_model.update(cx, |this, cx| {
                        this.items = items_to_save;
                        this.member_lookup = member_lookup;
                        this.incremental_render_counts_available = true;
                        this.render_counts = render_counts;
                        this.rendered_items = processed;
                        update_list_state(this, cx);
                    });
                });
                perf::log_if_slow(
                    "timeline.diff.full_rebuild",
                    full_rebuild_started,
                    Duration::from_millis(20),
                    || format!(
                        "diffs={} raw_items={} rendered_items={}",
                        diff_count,
                        rebuilt_raw_len,
                        rebuilt_rendered_len
                    ),
                );
            }
        })
        .detach();
}

pub(crate) fn update_list_state(this: &mut TimelineModel, cx: &mut Context<TimelineModel>) {
    let new_count = this.rendered_items.len();
    let old_count = this.list_state.item_count();

    if old_count == new_count {
        cx.notify();
        return;
    }

    let prepending_history = this.loading_history || this.warming_history;

    if new_count > old_count {
        let delta = new_count - old_count;
        if prepending_history {
            this.list_state.splice(0..0, delta);
        } else {
            this.list_state.splice(old_count..old_count, delta);
        }
    } else {
        let delta = old_count - new_count;
        if prepending_history {
            this.list_state.splice(0..delta, 0);
        } else {
            this.list_state.splice(new_count..old_count, 0);
        }
    }

    cx.notify();
}

fn replace_rendered_range(
    original: &Vector<RenderedTimelineItem>,
    start: usize,
    remove: usize,
    replacement: &Vector<RenderedTimelineItem>,
) -> Vector<RenderedTimelineItem> {
    let bounded_start = start.min(original.len());
    let bounded_end = bounded_start.saturating_add(remove).min(original.len());
    let mut rebuilt = Vector::new();

    for item in original.iter().take(bounded_start).cloned() {
        rebuilt.push_back(item);
    }
    for item in replacement.iter().cloned() {
        rebuilt.push_back(item);
    }
    for item in original.iter().skip(bounded_end).cloned() {
        rebuilt.push_back(item);
    }

    rebuilt
}
