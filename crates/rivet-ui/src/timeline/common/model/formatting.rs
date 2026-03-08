use super::*;

#[derive(Clone, Debug)]
struct PendingCallEntry {
    pub id: String,
    pub sender_name: String,
    pub content: String,
    pub timestamp: String,
}

fn flush_call_group(
    rendered: &mut Vector<RenderedTimelineItem>,
    pending_calls: &mut Vec<PendingCallEntry>,
    pending_date_divider: &mut Option<String>,
) {
    if pending_calls.is_empty() {
        return;
    }

    if let Some(divider) = pending_date_divider.take() {
        rendered.push_back(RenderedTimelineItem::Separator(divider));
    }

    let first = &pending_calls[0];
    let label = if pending_calls.len() == 1 {
        first.content.clone()
    } else {
        "Call activity".to_string()
    };
    let entries = pending_calls
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

    pending_calls.clear();
}

pub(crate) async fn process_items_vector(
    items: &Vector<Arc<TimelineItem>>,
    _homeserver_url: &str,
) -> Vector<RenderedTimelineItem> {
    tracing::info!("timeline: processing {} raw items", items.len());
    let mut rendered = Vector::new();
    let now = Local::now();
    let mut last_sender: Option<String> = None;
    let mut last_minute_bucket: Option<i64> = None;
    let mut pending_calls = Vec::new();
    let mut pending_date_divider: Option<String> = None;

    for item in items.iter() {
        if let Some(virtual_item) = item.as_virtual() {
            flush_call_group(&mut rendered, &mut pending_calls, &mut pending_date_divider);
            last_sender = None;
            last_minute_bucket = None;

            match virtual_item {
                VirtualTimelineItem::DateDivider(timestamp) => {
                    let dt: DateTime<Local> = timestamp
                        .to_system_time()
                        .unwrap_or(std::time::SystemTime::now())
                        .into();
                    pending_date_divider = Some(format_date_divider(dt));
                }
                VirtualTimelineItem::ReadMarker | VirtualTimelineItem::TimelineStart => {}
            }
            continue;
        }

        let Some(event) = item.as_event() else {
            last_sender = None;
            last_minute_bucket = None;
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
        let fallback_sender_name = fallback_sender_name(&sender_id);

        let (sender_name, avatar_url) = match event.sender_profile() {
            TimelineDetails::Ready(profile) => {
                let name = profile
                    .display_name
                    .clone()
                    .filter(|name| !name.trim().is_empty())
                    .unwrap_or_else(|| fallback_sender_name.clone());
                let avatar = profile.avatar_url.as_ref().map(|mxc| mxc.to_string());
                (name, avatar)
            }
            _ => (fallback_sender_name, None),
        };

        let is_grouped =
            last_sender.as_ref() == Some(&sender_id) && last_minute_bucket == Some(minute_bucket);
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
                    arrow: None,
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
                        arrow: None,
                    })
                }
                TimelineItemContent::FailedToParseState { event_type, .. } => {
                    Some(RenderedTimelineItem::System {
                        id,
                        content: format!(
                            "{sender_name} sent a state event we could not parse: {event_type}."
                        ),
                        timestamp: formatted_timestamp.clone(),
                        arrow: None,
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
                        arrow: None,
                    }),
                    MsgLikeKind::Other(other) => Some(RenderedTimelineItem::System {
                        id,
                        content: format!(
                            "{sender_name} sent an event of type {}.",
                            other.event_type()
                        ),
                        timestamp: formatted_timestamp.clone(),
                        arrow: None,
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
                pending_calls.push(call_entry);
                last_sender = None;
                last_minute_bucket = None;
                continue;
            }

            flush_call_group(&mut rendered, &mut pending_calls, &mut pending_date_divider);

            let is_chat_item = matches!(
                rendered_item,
                RenderedTimelineItem::Message { .. } | RenderedTimelineItem::Image { .. }
            );

            if is_chat_item {
                last_sender = Some(sender_id);
                last_minute_bucket = Some(minute_bucket);
            } else {
                last_sender = None;
                last_minute_bucket = None;
            }

            rendered.push_back(rendered_item);
        }
    }

    flush_call_group(&mut rendered, &mut pending_calls, &mut pending_date_divider);

    tracing::info!("timeline: produced {} rendered items", rendered.len());
    rendered
}
