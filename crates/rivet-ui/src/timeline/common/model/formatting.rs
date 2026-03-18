use super::*;
use futures::stream::{self, StreamExt};
use std::collections::{HashMap, HashSet};

pub(crate) type MemberLookup = HashMap<String, (String, Option<String>)>;
const MEMBER_LOOKUP_CONCURRENCY: usize = 16;
const MEMBER_LOOKUP_FALLBACK_LIMIT: usize = 12;

#[derive(Clone, Debug)]
struct PendingCallEntry {
    pub id: String,
    pub sender_name: String,
    pub content: String,
    pub timestamp: String,
}

#[derive(Clone, Debug, Default)]
struct RenderPassState {
    last_sender: Option<String>,
    last_minute_bucket: Option<i64>,
    pending_calls: Vec<PendingCallEntry>,
    pending_date_divider: Option<String>,
}

fn flush_call_group(rendered: &mut Vector<RenderedTimelineItem>, state: &mut RenderPassState) {
    if state.pending_calls.is_empty() {
        return;
    }

    if let Some(divider) = state.pending_date_divider.take() {
        rendered.push_back(RenderedTimelineItem::Separator(divider));
    }

    let first = &state.pending_calls[0];
    let label = if state.pending_calls.len() == 1 {
        first.content.clone()
    } else {
        "Call activity".to_string()
    };
    let entries = state
        .pending_calls
        .iter()
        .map(|entry| RenderedCallEntry {
            content: entry.content.clone(),
            timestamp: entry.timestamp.clone(),
        })
        .collect::<Vec<_>>();

    rendered.push_back(RenderedTimelineItem::CallGroup {
        id: format!("call-group-{}", first.id),
        sender_name: first.sender_name.clone(),
        label,
        entries,
    });

    state.pending_calls.clear();
}

fn insert_member_lookup_entry(
    member_lookup: &mut MemberLookup,
    member: &matrix_sdk::room::RoomMember,
) {
    let user_id = member.user_id().to_string();
    let display_name = member
        .display_name()
        .filter(|name| !name.trim().is_empty())
        .map(str::to_string)
        .unwrap_or_else(|| fallback_sender_name(&user_id));
    let avatar_url = member.avatar_url().map(|url| url.to_string());
    member_lookup.insert(user_id, (display_name, avatar_url));
}

pub(crate) async fn extend_member_lookup_for_items<'a, I>(
    room: &MatrixRoom,
    member_lookup: &mut MemberLookup,
    items: I,
) where
    I: IntoIterator<Item = &'a Arc<TimelineItem>>,
{
    let mut pending_sender_ids = Vec::new();
    let mut seen_sender_ids = HashSet::new();

    for item in items {
        let Some(event) = item.as_event() else {
            continue;
        };

        let sender_id = event.sender();
        if member_lookup.contains_key(sender_id.as_str()) {
            continue;
        }

        if let TimelineDetails::Ready(profile) = event.sender_profile() {
            let display_name = profile
                .display_name
                .clone()
                .filter(|name| !name.trim().is_empty())
                .unwrap_or_else(|| fallback_sender_name(sender_id.as_str()));
            let avatar_url = profile.avatar_url.as_ref().map(ToString::to_string);
            member_lookup.insert(sender_id.to_string(), (display_name, avatar_url));
            continue;
        }

        if seen_sender_ids.insert(sender_id.to_owned()) {
            pending_sender_ids.push(sender_id.to_owned());
        }
    }

    if pending_sender_ids.len() > MEMBER_LOOKUP_FALLBACK_LIMIT {
        for sender_id in pending_sender_ids {
            member_lookup
                .entry(sender_id.to_string())
                .or_insert_with(|| (fallback_sender_name(sender_id.as_str()), None));
        }
        return;
    }

    let room = room.clone();
    let member_results = stream::iter(pending_sender_ids.into_iter())
        .map(|sender_id| {
            let room = room.clone();
            async move {
                let member = room.get_member_no_sync(&sender_id).await.ok().flatten();
                (sender_id, member)
            }
        })
        .buffer_unordered(MEMBER_LOOKUP_CONCURRENCY)
        .collect::<Vec<_>>()
        .await;

    for (sender_id, member) in member_results {
        if let Some(member) = member {
            insert_member_lookup_entry(member_lookup, &member);
        } else {
            member_lookup
                .entry(sender_id.to_string())
                .or_insert_with(|| (fallback_sender_name(sender_id.as_str()), None));
        }
    }
}

pub(crate) async fn build_member_lookup(
    room: &MatrixRoom,
    items: &Vector<Arc<TimelineItem>>,
) -> MemberLookup {
    let mut member_lookup = MemberLookup::new();
    extend_member_lookup_for_items(room, &mut member_lookup, items.iter()).await;
    member_lookup
}

pub(crate) async fn process_items_vector(
    room: &MatrixRoom,
    items: &Vector<Arc<TimelineItem>>,
) -> (MemberLookup, Vector<RenderedTimelineItem>) {
    let member_lookup = build_member_lookup(room, items).await;
    let rendered = process_items_vector_with_lookup(items, &member_lookup);
    (member_lookup, rendered)
}

pub(crate) fn process_items_vector_with_lookup(
    items: &Vector<Arc<TimelineItem>>,
    member_lookup: &MemberLookup,
) -> Vector<RenderedTimelineItem> {
    let mut rendered = Vector::new();
    let mut state = RenderPassState::default();
    render_items(items.iter(), member_lookup, &mut rendered, &mut state);
    flush_call_group(&mut rendered, &mut state);
    rendered
}

pub(crate) fn process_appended_items_with_lookup(
    previous_items: &Vector<Arc<TimelineItem>>,
    appended_items: &[Arc<TimelineItem>],
    member_lookup: &MemberLookup,
) -> Option<Vector<RenderedTimelineItem>> {
    if appended_items.is_empty()
        || appended_items
            .iter()
            .any(|item| !supports_incremental_append(item))
    {
        return None;
    }

    let mut rendered = Vector::new();
    let mut state = initial_append_state(previous_items);
    render_items(
        appended_items.iter(),
        member_lookup,
        &mut rendered,
        &mut state,
    );
    flush_call_group(&mut rendered, &mut state);
    Some(rendered)
}

pub(crate) fn compute_render_counts(items: &Vector<Arc<TimelineItem>>) -> Vec<usize> {
    let mut counts = vec![0; items.len()];
    let mut pending_date_divider_ix: Option<usize> = None;
    let mut pending_calls: Vec<usize> = Vec::new();

    for (ix, item) in items.iter().enumerate() {
        if let Some(virtual_item) = item.as_virtual() {
            if !pending_calls.is_empty() {
                if let Some(divider_ix) = pending_date_divider_ix.take() {
                    counts[divider_ix] += 1;
                }
                if let Some(last_call_ix) = pending_calls.last().copied() {
                    counts[last_call_ix] += 1;
                }
                pending_calls.clear();
            }

            match virtual_item {
                VirtualTimelineItem::DateDivider(_) => {
                    pending_date_divider_ix = Some(ix);
                }
                VirtualTimelineItem::ReadMarker | VirtualTimelineItem::TimelineStart => {}
            }
            continue;
        }

        let Some(event) = item.as_event() else {
            if !pending_calls.is_empty() {
                if let Some(divider_ix) = pending_date_divider_ix.take() {
                    counts[divider_ix] += 1;
                }
                if let Some(last_call_ix) = pending_calls.last().copied() {
                    counts[last_call_ix] += 1;
                }
                pending_calls.clear();
            }
            pending_date_divider_ix = None;
            continue;
        };

        if event_is_call_like(event.content()) {
            pending_calls.push(ix);
            continue;
        }

        if event_renders(event.content()) {
            if let Some(divider_ix) = pending_date_divider_ix.take() {
                counts[divider_ix] += 1;
            }
            counts[ix] += 1;
        } else if !pending_calls.is_empty() {
            if let Some(divider_ix) = pending_date_divider_ix.take() {
                counts[divider_ix] += 1;
            }
            if let Some(last_call_ix) = pending_calls.last().copied() {
                counts[last_call_ix] += 1;
            }
            pending_calls.clear();
        }
    }

    if !pending_calls.is_empty() {
        if let Some(divider_ix) = pending_date_divider_ix.take() {
            counts[divider_ix] += 1;
        }
        if let Some(last_call_ix) = pending_calls.last().copied() {
            counts[last_call_ix] += 1;
        }
    }

    counts
}

fn supports_incremental_append(item: &Arc<TimelineItem>) -> bool {
    let Some(event) = item.as_event() else {
        return false;
    };

    !event_is_call_like(event.content())
}

fn initial_append_state(previous_items: &Vector<Arc<TimelineItem>>) -> RenderPassState {
    let Some(last_item) = previous_items.iter().last() else {
        return RenderPassState::default();
    };

    let Some(event) = last_item.as_event() else {
        return RenderPassState::default();
    };

    if event_is_call_like(event.content()) {
        return RenderPassState::default();
    }

    if !event_renders_as_chat_item(event.content()) {
        return RenderPassState::default();
    }

    let timestamp = event
        .timestamp()
        .to_system_time()
        .unwrap_or(std::time::SystemTime::now());
    let dt: DateTime<Local> = timestamp.into();

    RenderPassState {
        last_sender: Some(event.sender().to_string()),
        last_minute_bucket: Some(dt.timestamp() / 60),
        ..RenderPassState::default()
    }
}

fn event_is_call_like(content: &TimelineItemContent) -> bool {
    match content {
        TimelineItemContent::CallInvite | TimelineItemContent::RtcNotification => true,
        TimelineItemContent::OtherState(other_state) => matches!(
            other_state.content(),
            AnyOtherFullStateEventContent::_Custom { event_type }
                if event_type == "org.matrix.msc3401.call.member"
        ),
        _ => false,
    }
}

fn event_renders(content: &TimelineItemContent) -> bool {
    if content.as_message().is_some()
        || content.as_sticker().is_some()
        || content.as_unable_to_decrypt().is_some()
    {
        return true;
    }

    match content {
        TimelineItemContent::MembershipChange(_)
        | TimelineItemContent::ProfileChange(_)
        | TimelineItemContent::FailedToParseMessageLike { .. }
        | TimelineItemContent::FailedToParseState { .. } => true,
        TimelineItemContent::OtherState(_) => true,
        TimelineItemContent::MsgLike(msglike) => matches!(
            &msglike.kind,
            MsgLikeKind::Poll(_) | MsgLikeKind::Redacted | MsgLikeKind::Other(_)
        ),
        TimelineItemContent::CallInvite | TimelineItemContent::RtcNotification => false,
    }
}

fn event_renders_as_chat_item(content: &TimelineItemContent) -> bool {
    if let Some(message) = content.as_message() {
        return !matches!(message.msgtype(), MessageType::ServerNotice(_));
    }

    if content.as_sticker().is_some() || content.as_unable_to_decrypt().is_some() {
        return true;
    }

    matches!(
        content,
        TimelineItemContent::MsgLike(msglike) if matches!(&msglike.kind, MsgLikeKind::Poll(_))
    )
}

fn render_items<'a, I>(
    items: I,
    member_lookup: &MemberLookup,
    rendered: &mut Vector<RenderedTimelineItem>,
    state: &mut RenderPassState,
) where
    I: IntoIterator<Item = &'a Arc<TimelineItem>>,
{
    let now = Local::now();

    for item in items {
        if let Some(virtual_item) = item.as_virtual() {
            flush_call_group(rendered, state);
            state.last_sender = None;
            state.last_minute_bucket = None;

            match virtual_item {
                VirtualTimelineItem::DateDivider(timestamp) => {
                    let dt: DateTime<Local> = timestamp
                        .to_system_time()
                        .unwrap_or(std::time::SystemTime::now())
                        .into();
                    state.pending_date_divider = Some(format_date_divider(dt));
                }
                VirtualTimelineItem::ReadMarker | VirtualTimelineItem::TimelineStart => {}
            }
            continue;
        }

        let Some(event) = item.as_event() else {
            state.last_sender = None;
            state.last_minute_bucket = None;
            continue;
        };

        let sender_id = event.sender().to_string();
        let timestamp = event
            .timestamp()
            .to_system_time()
            .unwrap_or(std::time::SystemTime::now());
        let dt: DateTime<Local> = timestamp.into();
        let formatted_timestamp = format_message_timestamp(dt, now);
        let minute_bucket = dt.timestamp() / 60;
        let fallback_name = fallback_sender_name(&sender_id);

        let member_profile = member_lookup.get(&sender_id);
        let (sender_name, avatar_url) = match event.sender_profile() {
            TimelineDetails::Ready(profile) => {
                let name = profile
                    .display_name
                    .clone()
                    .filter(|name| !name.trim().is_empty())
                    .or_else(|| member_profile.map(|(name, _)| name.clone()))
                    .unwrap_or_else(|| fallback_name.clone());
                let avatar = profile
                    .avatar_url
                    .as_ref()
                    .map(|mxc| mxc.to_string())
                    .or_else(|| member_profile.and_then(|(_, avatar)| avatar.clone()));
                (name, avatar)
            }
            _ => member_profile
                .map(|(name, avatar)| (name.clone(), avatar.clone()))
                .unwrap_or((fallback_name, None)),
        };

        let is_grouped = state.last_sender.as_ref() == Some(&sender_id)
            && state.last_minute_bucket == Some(minute_bucket);
        let id = timeline_item_id(item, Some(event));
        let reply_to = render_reply_preview(event.content().in_reply_to());

        let rendered_item = if let Some(message) = event.content().as_message() {
            match message.msgtype() {
                MessageType::Text(text) => Some(RenderedTimelineItem::Message {
                    id,
                    sender_id: sender_id.clone(),
                    sender_name: sender_name.clone(),
                    body: rendered_body_from_text(&text.body, text.formatted.as_ref()),
                    timestamp: formatted_timestamp.clone(),
                    is_own: event.is_own(),
                    is_grouped,
                    avatar_url: avatar_url.clone(),
                    edited: message.is_edited(),
                    is_notice: false,
                    reply_to,
                }),
                MessageType::Notice(notice) => Some(RenderedTimelineItem::Message {
                    id,
                    sender_id: sender_id.clone(),
                    sender_name: sender_name.clone(),
                    body: rendered_body_from_text(&notice.body, notice.formatted.as_ref()),
                    timestamp: formatted_timestamp.clone(),
                    is_own: event.is_own(),
                    is_grouped,
                    avatar_url: avatar_url.clone(),
                    edited: message.is_edited(),
                    is_notice: true,
                    reply_to,
                }),
                MessageType::Emote(emote) => Some(RenderedTimelineItem::Message {
                    id,
                    sender_id: sender_id.clone(),
                    sender_name: sender_name.clone(),
                    body: rendered_body_from_text(&emote.body, emote.formatted.as_ref()),
                    timestamp: formatted_timestamp.clone(),
                    is_own: event.is_own(),
                    is_grouped,
                    avatar_url: avatar_url.clone(),
                    edited: message.is_edited(),
                    is_notice: false,
                    reply_to,
                }),
                MessageType::Image(image) => Some(RenderedTimelineItem::Image {
                    id,
                    sender_id: sender_id.clone(),
                    sender_name: sender_name.clone(),
                    source: image.source.clone(),
                    mimetype: image.info.as_ref().and_then(|i| i.mimetype.clone()),
                    dimensions: image.info.as_ref().and_then(|info| image_dimensions(info)),
                    caption: optional_caption(&image.body),
                    timestamp: formatted_timestamp.clone(),
                    is_own: event.is_own(),
                    is_grouped,
                    avatar_url: avatar_url.clone(),
                    edited: message.is_edited(),
                    reply_to,
                }),
                MessageType::Audio(audio) => Some(RenderedTimelineItem::Message {
                    id,
                    sender_id: sender_id.clone(),
                    sender_name: sender_name.clone(),
                    body: attachment_body("Audio", &audio.body),
                    timestamp: formatted_timestamp.clone(),
                    is_own: event.is_own(),
                    is_grouped,
                    avatar_url: avatar_url.clone(),
                    edited: message.is_edited(),
                    is_notice: false,
                    reply_to,
                }),
                MessageType::File(file) => Some(RenderedTimelineItem::Message {
                    id,
                    sender_id: sender_id.clone(),
                    sender_name: sender_name.clone(),
                    body: attachment_body("File", &file.body),
                    timestamp: formatted_timestamp.clone(),
                    is_own: event.is_own(),
                    is_grouped,
                    avatar_url: avatar_url.clone(),
                    edited: message.is_edited(),
                    is_notice: false,
                    reply_to,
                }),
                MessageType::Video(video) => Some(RenderedTimelineItem::Message {
                    id,
                    sender_id: sender_id.clone(),
                    sender_name: sender_name.clone(),
                    body: attachment_body("Video", &video.body),
                    timestamp: formatted_timestamp.clone(),
                    is_own: event.is_own(),
                    is_grouped,
                    avatar_url: avatar_url.clone(),
                    edited: message.is_edited(),
                    is_notice: false,
                    reply_to,
                }),
                MessageType::Location(location) => Some(RenderedTimelineItem::Message {
                    id,
                    sender_id: sender_id.clone(),
                    sender_name: sender_name.clone(),
                    body: attachment_body("Location", &location.body),
                    timestamp: formatted_timestamp.clone(),
                    is_own: event.is_own(),
                    is_grouped,
                    avatar_url: avatar_url.clone(),
                    edited: message.is_edited(),
                    is_notice: false,
                    reply_to,
                }),
                MessageType::VerificationRequest(request) => Some(RenderedTimelineItem::Message {
                    id,
                    sender_id: sender_id.clone(),
                    sender_name: sender_name.clone(),
                    body: attachment_body("Verification request", &request.body),
                    timestamp: formatted_timestamp.clone(),
                    is_own: event.is_own(),
                    is_grouped,
                    avatar_url: avatar_url.clone(),
                    edited: message.is_edited(),
                    is_notice: true,
                    reply_to,
                }),
                MessageType::ServerNotice(server_notice) => Some(RenderedTimelineItem::System {
                    id,
                    content: server_notice.body.clone(),
                    timestamp: formatted_timestamp.clone(),
                    arrow: Some("icons/badge-info.svg".to_string()),
                }),
                _ => Some(RenderedTimelineItem::Message {
                    id,
                    sender_id: sender_id.clone(),
                    sender_name: sender_name.clone(),
                    body: attachment_body(message.msgtype().msgtype(), message.body()),
                    timestamp: formatted_timestamp.clone(),
                    is_own: event.is_own(),
                    is_grouped,
                    avatar_url: avatar_url.clone(),
                    edited: message.is_edited(),
                    is_notice: false,
                    reply_to,
                }),
            }
        } else if let Some(sticker) = event.content().as_sticker() {
            match &sticker.content().source {
                StickerMediaSource::Plain(url) => Some(RenderedTimelineItem::Image {
                    id,
                    sender_id: sender_id.clone(),
                    sender_name: sender_name.clone(),
                    source: matrix_sdk::ruma::events::room::MediaSource::Plain(url.clone()),
                    mimetype: None,
                    dimensions: image_dimensions(&sticker.content().info),
                    caption: optional_caption_str(&sticker.content().body),
                    timestamp: formatted_timestamp.clone(),
                    is_own: event.is_own(),
                    is_grouped,
                    avatar_url: avatar_url.clone(),
                    edited: false,
                    reply_to,
                }),
                _ => Some(RenderedTimelineItem::Message {
                    id,
                    sender_id: sender_id.clone(),
                    sender_name: sender_name.clone(),
                    body: attachment_body("Sticker", &sticker.content().body),
                    timestamp: formatted_timestamp.clone(),
                    is_own: event.is_own(),
                    is_grouped,
                    avatar_url: avatar_url.clone(),
                    edited: false,
                    is_notice: false,
                    reply_to,
                }),
            }
        } else if event.content().as_unable_to_decrypt().is_some() {
            Some(RenderedTimelineItem::Message {
                id,
                sender_id: sender_id.clone(),
                sender_name: sender_name.clone(),
                body: RenderedBody::Plain(
                    "[Unable to decrypt message - check your keys]".to_string(),
                ),
                timestamp: formatted_timestamp.clone(),
                is_own: event.is_own(),
                is_grouped,
                avatar_url: avatar_url.clone(),
                edited: false,
                is_notice: true,
                reply_to,
            })
        } else {
            match event.content() {
                TimelineItemContent::MembershipChange(change) => {
                    let (content, arrow) =
                        format_membership_change(change, &sender_name, &sender_id);
                    Some(RenderedTimelineItem::System {
                        id,
                        content,
                        timestamp: formatted_timestamp.clone(),
                        arrow,
                    })
                }
                TimelineItemContent::ProfileChange(change) => {
                    let (content, arrow) = format_profile_change(change, &sender_name);
                    Some(RenderedTimelineItem::System {
                        id,
                        content,
                        timestamp: formatted_timestamp.clone(),
                        arrow,
                    })
                }
                TimelineItemContent::OtherState(other_state) => {
                    format_other_state(other_state, &sender_name).map(|(content, arrow)| {
                        RenderedTimelineItem::System {
                            id,
                            content,
                            timestamp: formatted_timestamp.clone(),
                            arrow,
                        }
                    })
                }
                TimelineItemContent::FailedToParseMessageLike { event_type, .. } => {
                    Some(RenderedTimelineItem::System {
                        id,
                        content: format!(
                            "{sender_name} sent an event we could not parse: {event_type}."
                        ),
                        timestamp: formatted_timestamp.clone(),
                        arrow: Some("icons/triangle-alert.svg".to_string()),
                    })
                }
                TimelineItemContent::FailedToParseState { event_type, .. } => {
                    Some(RenderedTimelineItem::System {
                        id,
                        content: format!(
                            "{sender_name} sent a state event we could not parse: {event_type}."
                        ),
                        timestamp: formatted_timestamp.clone(),
                        arrow: Some("icons/triangle-alert.svg".to_string()),
                    })
                }
                TimelineItemContent::CallInvite => Some(RenderedTimelineItem::System {
                    id,
                    content: "Call activity".to_string(),
                    timestamp: formatted_timestamp.clone(),
                    arrow: Some("icons/phone-call.svg".to_string()),
                }),
                TimelineItemContent::RtcNotification => Some(RenderedTimelineItem::System {
                    id,
                    content: "Call activity".to_string(),
                    timestamp: formatted_timestamp.clone(),
                    arrow: Some("icons/phone-call.svg".to_string()),
                }),
                TimelineItemContent::MsgLike(msglike) => match &msglike.kind {
                    MsgLikeKind::Poll(poll) => Some(RenderedTimelineItem::Message {
                        id,
                        sender_id: sender_id.clone(),
                        sender_name: sender_name.clone(),
                        body: rendered_body_from_text(
                            &poll
                                .fallback_text()
                                .unwrap_or_else(|| format!("Poll: {}", poll.results().question)),
                            None,
                        ),
                        timestamp: formatted_timestamp.clone(),
                        is_own: event.is_own(),
                        is_grouped,
                        avatar_url: avatar_url.clone(),
                        edited: poll.is_edit(),
                        is_notice: false,
                        reply_to,
                    }),
                    MsgLikeKind::Redacted => Some(RenderedTimelineItem::System {
                        id,
                        content: format!("{sender_name} removed a message."),
                        timestamp: formatted_timestamp.clone(),
                        arrow: Some("icons/eraser.svg".to_string()),
                    }),
                    MsgLikeKind::Other(other) => Some(RenderedTimelineItem::System {
                        id,
                        content: format!(
                            "{sender_name} sent an event of type {}.",
                            other.event_type()
                        ),
                        timestamp: formatted_timestamp.clone(),
                        arrow: Some("icons/badge-info.svg".to_string()),
                    }),
                    MsgLikeKind::Message(_)
                    | MsgLikeKind::Sticker(_)
                    | MsgLikeKind::UnableToDecrypt(_) => None,
                },
            }
        };

        if let Some(rendered_item) = rendered_item {
            let call_entry = match &rendered_item {
                RenderedTimelineItem::System {
                    id,
                    content,
                    timestamp,
                    ..
                } if content == "Call activity"
                    || content.contains("started a call")
                    || content.contains("call notification") =>
                {
                    Some(PendingCallEntry {
                        id: id.clone(),
                        sender_name: sender_name.clone(),
                        content: content.clone(),
                        timestamp: timestamp.clone(),
                    })
                }
                _ => None,
            };

            if let Some(call_entry) = call_entry {
                state.pending_calls.push(call_entry);
                state.last_sender = None;
                state.last_minute_bucket = None;
                continue;
            }

            flush_call_group(rendered, state);

            let is_chat_item = matches!(
                rendered_item,
                RenderedTimelineItem::Message { .. } | RenderedTimelineItem::Image { .. }
            );

            if is_chat_item {
                state.last_sender = Some(sender_id);
                state.last_minute_bucket = Some(minute_bucket);
            } else {
                state.last_sender = None;
                state.last_minute_bucket = None;
            }

            rendered.push_back(rendered_item);
        }
    }
}

fn image_dimensions(info: &matrix_sdk::ruma::events::room::ImageInfo) -> Option<(u32, u32)> {
    Some((
        u32::try_from(u64::from(info.width?)).ok()?,
        u32::try_from(u64::from(info.height?)).ok()?,
    ))
}
