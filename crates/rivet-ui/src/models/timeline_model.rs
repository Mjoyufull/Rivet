use chrono::{DateTime, Local};
use futures::StreamExt;
use gpui::*;
use matrix_sdk::Room as MatrixRoom;
use matrix_sdk::ruma::events::{
    FullStateEventContent,
    room::{
        member::MembershipState,
        message::{FormattedBody, MessageFormat, MessageType, RoomMessageEventContent},
    },
    sticker::StickerMediaSource,
};
use matrix_sdk_ui::eyeball_im::{Vector, VectorDiff};
use matrix_sdk_ui::timeline::{
    AnyOtherFullStateEventContent, MemberProfileChange, MembershipChange, MsgLikeKind, OtherState,
    RoomMembershipChange, Timeline, TimelineDetails, TimelineItem, TimelineItemContent,
    VirtualTimelineItem,
};
use std::sync::Arc;

const INITIAL_HISTORY_BATCH_SIZE: u16 = 60;
const HISTORY_BATCH_SIZE: u16 = 80;
const HISTORY_PREFETCH_THRESHOLD: usize = 12;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum ChatStyle {
    #[default]
    Modern,
    Bubble,
}

#[derive(Clone, Debug)]
pub enum RenderedBody {
    Plain(String),
    Markdown(String),
    Html(String),
}

#[derive(Clone, Debug)]
pub struct RenderedReplyPreview {
    pub event_id: String,
    pub sender_name: String,
    pub body: String,
}

#[derive(Clone, Debug)]
pub struct RenderedCallEntry {
    pub id: String,
    pub content: String,
    pub timestamp: String,
}

#[derive(Clone, Debug)]
pub enum RenderedTimelineItem {
    Message {
        id: String,
        sender_id: String,
        sender_name: String,
        body: RenderedBody,
        timestamp: String,
        is_own: bool,
        is_grouped: bool,
        avatar_url: Option<String>,
        edited: bool,
        is_notice: bool,
        reply_to: Option<RenderedReplyPreview>,
    },
    Image {
        id: String,
        sender_id: String,
        sender_name: String,
        url: String,
        caption: Option<RenderedBody>,
        timestamp: String,
        is_own: bool,
        is_grouped: bool,
        avatar_url: Option<String>,
        edited: bool,
        reply_to: Option<RenderedReplyPreview>,
    },
    System {
        id: String,
        content: String,
        timestamp: String,
    },
    CallGroup {
        id: String,
        sender_name: String,
        avatar_url: Option<String>,
        label: String,
        entries: Vec<RenderedCallEntry>,
    },
    Separator(String),
}

pub struct TimelineModel {
    pub room: MatrixRoom,
    pub timeline: Arc<Timeline>,
    pub items: Vector<Arc<TimelineItem>>,
    pub rendered_items: Vector<RenderedTimelineItem>,
    pub list_state: ListState,
    pub homeserver_url: String,
    pub chat_style: ChatStyle,
    pub loading_history: bool,
    pub hit_timeline_start: bool,
}

impl EventEmitter<()> for TimelineModel {}

fn fallback_sender_name(sender_id: &str) -> String {
    sender_id
        .trim_start_matches('@')
        .split(':')
        .next()
        .unwrap_or(sender_id)
        .to_string()
}

fn timeline_item_id(
    item: &Arc<TimelineItem>,
    event: Option<&matrix_sdk_ui::timeline::EventTimelineItem>,
) -> String {
    event
        .and_then(|event| event.event_id().map(|event_id| event_id.to_string()))
        .unwrap_or_else(|| item.unique_id().0.clone())
}

fn format_message_timestamp(timestamp: DateTime<Local>, now: DateTime<Local>) -> String {
    let today = now.date_naive();
    let message_day = timestamp.date_naive();

    if message_day == today {
        timestamp.format("%-I:%M %p").to_string()
    } else if today
        .checked_sub_days(chrono::Days::new(1))
        .is_some_and(|yesterday| yesterday == message_day)
    {
        format!("Yesterday at {}", timestamp.format("%-I:%M %p"))
    } else {
        timestamp.format("%-m/%-d/%y, %-I:%M %p").to_string()
    }
}

fn format_date_divider(timestamp: DateTime<Local>) -> String {
    let today = Local::now().date_naive();
    let message_day = timestamp.date_naive();

    if message_day == today {
        "Today".to_string()
    } else if today
        .checked_sub_days(chrono::Days::new(1))
        .is_some_and(|yesterday| yesterday == message_day)
    {
        "Yesterday".to_string()
    } else {
        timestamp.format("%B %-d, %Y").to_string()
    }
}

fn should_use_rich_text(body: &str) -> bool {
    body.contains('\n')
        || body.contains("```")
        || body.contains("https://")
        || body.contains("http://")
        || body.contains("www.")
        || body.contains('`')
}

fn rendered_body_from_text(body: &str, formatted: Option<&FormattedBody>) -> RenderedBody {
    if let Some(formatted) = formatted {
        if formatted.format == MessageFormat::Html {
            return RenderedBody::Html(formatted.body.clone());
        }
    }

    if should_use_rich_text(body) {
        RenderedBody::Markdown(body.to_string())
    } else {
        RenderedBody::Plain(body.to_string())
    }
}

fn effective_membership_change(
    membership_change: &RoomMembershipChange,
    sender_id: &str,
) -> MembershipChange {
    match membership_change.change().unwrap_or(MembershipChange::None) {
        MembershipChange::Joined
        | MembershipChange::Left
        | MembershipChange::Banned
        | MembershipChange::Unbanned
        | MembershipChange::Kicked
        | MembershipChange::Invited
        | MembershipChange::KickedAndBanned
        | MembershipChange::InvitationAccepted
        | MembershipChange::InvitationRejected
        | MembershipChange::InvitationRevoked
        | MembershipChange::Knocked
        | MembershipChange::KnockAccepted
        | MembershipChange::KnockRetracted
        | MembershipChange::KnockDenied => membership_change.change().unwrap(),
        _ => {
            let membership = match membership_change.content() {
                FullStateEventContent::Original { content, .. } => &content.membership,
                FullStateEventContent::Redacted(content) => &content.membership,
            };

            match membership {
                MembershipState::Ban => MembershipChange::Banned,
                MembershipState::Invite => MembershipChange::Invited,
                MembershipState::Join => MembershipChange::Joined,
                MembershipState::Knock => MembershipChange::Knocked,
                MembershipState::Leave => {
                    if membership_change.user_id().as_str() == sender_id {
                        MembershipChange::Left
                    } else {
                        MembershipChange::Kicked
                    }
                }
                _ => MembershipChange::NotImplemented,
            }
        }
    }
}

fn format_membership_change(
    membership_change: &RoomMembershipChange,
    sender_name: &str,
    sender_id: &str,
) -> String {
    let target_display_name = membership_change
        .display_name()
        .filter(|name| !name.trim().is_empty())
        .unwrap_or_else(|| fallback_sender_name(membership_change.user_id().as_str()));

    match effective_membership_change(membership_change, sender_id) {
        MembershipChange::Joined => format!("{target_display_name} joined the room."),
        MembershipChange::Left => format!("{target_display_name} left the room."),
        MembershipChange::Banned | MembershipChange::KickedAndBanned => {
            format!("{sender_name} banned {target_display_name}.")
        }
        MembershipChange::Unbanned => format!("{sender_name} unbanned {target_display_name}."),
        MembershipChange::Kicked => format!("{sender_name} kicked {target_display_name}."),
        MembershipChange::Invited | MembershipChange::KnockAccepted => {
            format!("{sender_name} invited {target_display_name}.")
        }
        MembershipChange::InvitationAccepted => {
            format!("{target_display_name} accepted the invite.")
        }
        MembershipChange::InvitationRejected => {
            format!("{target_display_name} declined the invite.")
        }
        MembershipChange::InvitationRevoked => {
            format!("{sender_name} revoked the invitation for {target_display_name}.")
        }
        MembershipChange::Knocked => {
            format!("{target_display_name} requested to join the room.")
        }
        MembershipChange::KnockRetracted => {
            format!("{target_display_name} retracted their join request.")
        }
        MembershipChange::KnockDenied => {
            format!("{sender_name} denied {target_display_name}'s join request.")
        }
        MembershipChange::None | MembershipChange::Error | MembershipChange::NotImplemented => {
            format!("{target_display_name}'s membership changed.")
        }
    }
}

fn format_profile_change(profile_change: &MemberProfileChange, sender_name: &str) -> String {
    if let Some(displayname) = profile_change.displayname_change() {
        if let Some(previous_name) = &displayname.old {
            if let Some(new_name) = &displayname.new {
                return format!("{previous_name} changed their display name to {new_name}.");
            }

            return format!("{previous_name} removed their display name.");
        }

        if let Some(new_name) = &displayname.new {
            return format!(
                "{} set their display name to {new_name}.",
                fallback_sender_name(profile_change.user_id().as_str())
            );
        }
    }

    if let Some(avatar_url) = profile_change.avatar_url_change() {
        if avatar_url.old.is_none() {
            return format!("{sender_name} set their avatar.");
        }
        if avatar_url.new.is_none() {
            return format!("{sender_name} removed their avatar.");
        }
        return format!("{sender_name} changed their avatar.");
    }

    format!("{sender_name} updated their profile.")
}

fn format_other_state(other_state: &OtherState, sender_name: &str) -> String {
    match other_state.content() {
        AnyOtherFullStateEventContent::RoomCreate(_) => {
            format!("{sender_name} created the room.")
        }
        AnyOtherFullStateEventContent::RoomEncryption(_) => {
            "This room is encrypted from this point on.".to_string()
        }
        AnyOtherFullStateEventContent::RoomName(_) => {
            format!("{sender_name} changed the room name.")
        }
        AnyOtherFullStateEventContent::RoomTopic(_) => {
            format!("{sender_name} changed the room topic.")
        }
        AnyOtherFullStateEventContent::RoomAvatar(_) => {
            format!("{sender_name} changed the room avatar.")
        }
        AnyOtherFullStateEventContent::RoomAliases(_) => {
            format!("{sender_name} updated the room aliases.")
        }
        AnyOtherFullStateEventContent::RoomCanonicalAlias(_) => {
            format!("{sender_name} changed the room alias.")
        }
        AnyOtherFullStateEventContent::RoomGuestAccess(_) => {
            format!("{sender_name} changed guest access.")
        }
        AnyOtherFullStateEventContent::RoomHistoryVisibility(_) => {
            format!("{sender_name} changed history visibility.")
        }
        AnyOtherFullStateEventContent::RoomJoinRules(_) => {
            format!("{sender_name} changed join rules.")
        }
        AnyOtherFullStateEventContent::RoomPinnedEvents(_) => {
            format!("{sender_name} updated pinned messages.")
        }
        AnyOtherFullStateEventContent::RoomPowerLevels(_) => {
            format!("{sender_name} updated room permissions.")
        }
        AnyOtherFullStateEventContent::RoomServerAcl(_) => {
            format!("{sender_name} updated the server ACL.")
        }
        AnyOtherFullStateEventContent::RoomThirdPartyInvite(_) => {
            format!("{sender_name} created a third-party invite.")
        }
        AnyOtherFullStateEventContent::RoomTombstone(_) => {
            "This room has been replaced with a newer room.".to_string()
        }
        AnyOtherFullStateEventContent::SpaceChild(_)
        | AnyOtherFullStateEventContent::SpaceParent(_) => {
            format!("{sender_name} updated the space hierarchy.")
        }
        AnyOtherFullStateEventContent::PolicyRuleRoom(_)
        | AnyOtherFullStateEventContent::PolicyRuleServer(_)
        | AnyOtherFullStateEventContent::PolicyRuleUser(_) => {
            format!("{sender_name} updated a room policy.")
        }
        AnyOtherFullStateEventContent::_Custom { event_type } => {
            if event_type == "org.matrix.msc3401.call.member" {
                return format!("{sender_name} updated call membership.");
            }
            format!("{sender_name} sent a state event: {event_type}.")
        }
    }
}

fn room_media_source_url(source: &matrix_sdk::ruma::events::room::MediaSource) -> String {
    match source {
        matrix_sdk::ruma::events::room::MediaSource::Plain(url) => url.to_string(),
        matrix_sdk::ruma::events::room::MediaSource::Encrypted(file) => file.url.to_string(),
    }
}

fn attachment_body(kind: &str, body: &str) -> RenderedBody {
    if body.trim().is_empty() {
        RenderedBody::Plain(kind.to_string())
    } else {
        RenderedBody::Plain(format!("{kind}: {body}"))
    }
}

fn optional_caption(body: &str) -> Option<RenderedBody> {
    (!body.trim().is_empty()).then(|| rendered_body_from_text(body, None))
}

fn embedded_sender_name(
    sender_id: &str,
    profile: &TimelineDetails<matrix_sdk_ui::timeline::Profile>,
) -> String {
    match profile {
        TimelineDetails::Ready(profile) => profile
            .display_name
            .clone()
            .filter(|name| !name.trim().is_empty())
            .unwrap_or_else(|| fallback_sender_name(sender_id)),
        _ => fallback_sender_name(sender_id),
    }
}

fn embedded_preview_body(content: &TimelineItemContent) -> String {
    if let Some(message) = content.as_message() {
        return message
            .body()
            .lines()
            .find(|line| !line.trim().is_empty())
            .unwrap_or("Message")
            .to_string();
    }

    if content.as_sticker().is_some() {
        return "Sticker".to_string();
    }

    if content.as_unable_to_decrypt().is_some() {
        return "Unable to decrypt message".to_string();
    }

    match content {
        TimelineItemContent::MembershipChange(_) => "Membership event".to_string(),
        TimelineItemContent::ProfileChange(_) => "Profile update".to_string(),
        TimelineItemContent::OtherState(_) => "State event".to_string(),
        TimelineItemContent::FailedToParseMessageLike { .. }
        | TimelineItemContent::FailedToParseState { .. } => "Unsupported event".to_string(),
        TimelineItemContent::CallInvite | TimelineItemContent::RtcNotification => {
            "Call activity".to_string()
        }
        TimelineItemContent::MsgLike(msglike) => match &msglike.kind {
            MsgLikeKind::Poll(_) => "Poll".to_string(),
            MsgLikeKind::Redacted => "Message removed".to_string(),
            MsgLikeKind::Other(_) => "Event".to_string(),
            MsgLikeKind::Message(_) | MsgLikeKind::Sticker(_) | MsgLikeKind::UnableToDecrypt(_) => {
                "Message".to_string()
            }
        },
    }
}

fn render_reply_preview(
    reply_details: Option<matrix_sdk_ui::timeline::InReplyToDetails>,
) -> Option<RenderedReplyPreview> {
    let reply_details = reply_details?;

    let (sender_name, body) = match reply_details.event {
        TimelineDetails::Ready(event) => {
            let sender_id = event.sender.to_string();
            (
                embedded_sender_name(&sender_id, &event.sender_profile),
                embedded_preview_body(&event.content),
            )
        }
        TimelineDetails::Pending => ("Loading".to_string(), "Loading reply...".to_string()),
        TimelineDetails::Unavailable => (
            "Unknown".to_string(),
            "Original message is not loaded".to_string(),
        ),
        TimelineDetails::Error(_) => (
            "Unknown".to_string(),
            "Could not load the original message".to_string(),
        ),
    };

    Some(RenderedReplyPreview {
        event_id: reply_details.event_id.to_string(),
        sender_name,
        body,
    })
}

#[derive(Clone, Debug)]
struct PendingCallEntry {
    id: String,
    sender_name: String,
    avatar_url: Option<String>,
    content: String,
    timestamp: String,
}

fn flush_call_group(
    rendered: &mut Vector<RenderedTimelineItem>,
    pending_calls: &mut Vec<PendingCallEntry>,
) {
    if pending_calls.is_empty() {
        return;
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
            id: entry.id.clone(),
            content: entry.content.clone(),
            timestamp: entry.timestamp.clone(),
        })
        .collect::<Vec<_>>();

    rendered.push_back(RenderedTimelineItem::CallGroup {
        id: format!("call-group-{}", first.id),
        sender_name: first.sender_name.clone(),
        avatar_url: first.avatar_url.clone(),
        label,
        entries,
    });

    pending_calls.clear();
}

impl TimelineModel {
    pub fn new(
        room: MatrixRoom,
        timeline: Arc<Timeline>,
        homeserver_url: String,
        cx: &mut App,
    ) -> Entity<Self> {
        let list_state = ListState::new(0, ListAlignment::Bottom, px(1000.0));

        let model = cx.new(|_| Self {
            room,
            timeline,
            items: Vector::new(),
            rendered_items: Vector::new(),
            list_state,
            homeserver_url,
            chat_style: ChatStyle::default(),
            loading_history: false,
            hit_timeline_start: false,
        });

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

        model
    }

    pub fn load_more_history(&mut self, model_handle: Entity<Self>, cx: &mut App) {
        if self.loading_history || self.hit_timeline_start {
            return;
        }

        self.loading_history = true;
        cx.notify(model_handle.entity_id());

        let timeline = self.timeline.clone();
        let weak_model = model_handle.downgrade();

        cx.spawn(|cx: &mut AsyncApp| {
            let mut cx = cx.clone();
            async move {
                let result = timeline.paginate_backwards(HISTORY_BATCH_SIZE).await;

                if let Some(model) = weak_model.upgrade() {
                    let _ = model.update(&mut cx, |this, cx: &mut Context<Self>| {
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

    pub fn init(model: Entity<Self>, cx: &mut App) {
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
                let processed = Self::process_items_vector(&items, &homeserver_url).await;

                let _ = async_cx.update(|cx: &mut App| {
                    let _ = weak_model.update(cx, |this, cx: &mut Context<Self>| {
                        this.items = items;
                        this.rendered_items = processed;
                        this.update_list_state(cx);
                    });
                });

                // Trigger one larger initial back-pagination after first paint data is set.
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
                    // Get current items to apply diffs
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
                        // Apply diff to local items
                        match diff {
                            VectorDiff::Append { values } => {
                                current_items.append(values);
                            }
                            VectorDiff::Clear => {
                                current_items.clear();
                            }
                            VectorDiff::PushFront { value } => {
                                current_items.push_front(value);
                            }
                            VectorDiff::PushBack { value } => {
                                current_items.push_back(value);
                            }
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
                            VectorDiff::Truncate { length } => {
                                current_items.truncate(length);
                            }
                            VectorDiff::Reset { values } => {
                                current_items = values;
                            }
                        }
                    }

                    let homeserver_url = async_cx
                        .update(|cx: &mut App| {
                            weak_model
                                .update(cx, |this, _| this.homeserver_url.clone())
                                .unwrap_or_default()
                        })
                        .unwrap_or_default();
                    // Process updated items once per chunk
                    let processed =
                        Self::process_items_vector(&current_items, &homeserver_url).await;
                    let items_to_save = current_items;

                    // Update model once after processing all diffs in the chunk
                    let _ = async_cx.update(|cx: &mut App| {
                        let _ = weak_model.update(cx, |this, cx| {
                            this.items = items_to_save;
                            this.rendered_items = processed;
                            this.update_list_state(cx);
                        });
                    });
                }
            })
            .detach();
    }

    async fn process_items_vector(
        items: &Vector<Arc<TimelineItem>>,
        _homeserver_url: &str,
    ) -> Vector<RenderedTimelineItem> {
        tracing::info!("timeline: processing {} raw items", items.len());
        let mut rendered = Vector::new();
        let now = Local::now();
        let mut last_sender: Option<String> = None;
        let mut last_minute_bucket: Option<i64> = None;
        let mut pending_calls = Vec::new();

        for item in items.iter() {
            if let Some(virtual_item) = item.as_virtual() {
                flush_call_group(&mut rendered, &mut pending_calls);
                last_sender = None;
                last_minute_bucket = None;

                match virtual_item {
                    VirtualTimelineItem::DateDivider(timestamp) => {
                        let dt: DateTime<Local> = timestamp
                            .to_system_time()
                            .unwrap_or(std::time::SystemTime::now())
                            .into();
                        rendered
                            .push_back(RenderedTimelineItem::Separator(format_date_divider(dt)));
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

            let is_grouped = last_sender.as_ref() == Some(&sender_id)
                && last_minute_bucket == Some(minute_bucket);
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
                        url: room_media_source_url(&image.source),
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
                    MessageType::VerificationRequest(request) => {
                        Some(RenderedTimelineItem::Message {
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
                        })
                    }
                    MessageType::ServerNotice(server_notice) => {
                        Some(RenderedTimelineItem::System {
                            id,
                            content: server_notice.body.clone(),
                            timestamp: formatted_timestamp.clone(),
                        })
                    }
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
                        url: url.to_string(),
                        caption: optional_caption(&sticker.content().body),
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
                        Some(RenderedTimelineItem::System {
                            id,
                            content: format_membership_change(change, &sender_name, &sender_id),
                            timestamp: formatted_timestamp.clone(),
                        })
                    }
                    TimelineItemContent::ProfileChange(change) => {
                        Some(RenderedTimelineItem::System {
                            id,
                            content: format_profile_change(change, &sender_name),
                            timestamp: formatted_timestamp.clone(),
                        })
                    }
                    TimelineItemContent::OtherState(other_state) => {
                        Some(RenderedTimelineItem::System {
                            id,
                            content: format_other_state(other_state, &sender_name),
                            timestamp: formatted_timestamp.clone(),
                        })
                    }
                    TimelineItemContent::FailedToParseMessageLike { event_type, .. } => {
                        Some(RenderedTimelineItem::System {
                            id,
                            content: format!(
                                "{sender_name} sent an event we could not parse: {event_type}."
                            ),
                            timestamp: formatted_timestamp.clone(),
                        })
                    }
                    TimelineItemContent::FailedToParseState { event_type, .. } => {
                        Some(RenderedTimelineItem::System {
                            id,
                            content: format!(
                                "{sender_name} sent a state event we could not parse: {event_type}."
                            ),
                            timestamp: formatted_timestamp.clone(),
                        })
                    }
                    TimelineItemContent::CallInvite => Some(RenderedTimelineItem::System {
                        id,
                        content: "Call activity".to_string(),
                        timestamp: formatted_timestamp.clone(),
                    }),
                    TimelineItemContent::RtcNotification => Some(RenderedTimelineItem::System {
                        id,
                        content: "Call activity".to_string(),
                        timestamp: formatted_timestamp.clone(),
                    }),
                    TimelineItemContent::MsgLike(msglike) => match &msglike.kind {
                        MsgLikeKind::Poll(poll) => Some(RenderedTimelineItem::Message {
                            id,
                            sender_id: sender_id.clone(),
                            sender_name: sender_name.clone(),
                            body: rendered_body_from_text(
                                &poll.fallback_text().unwrap_or_else(|| {
                                    format!("Poll: {}", poll.results().question)
                                }),
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
                        }),
                        MsgLikeKind::Other(other) => Some(RenderedTimelineItem::System {
                            id,
                            content: format!(
                                "{sender_name} sent an event of type {}.",
                                other.event_type()
                            ),
                            timestamp: formatted_timestamp.clone(),
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
                    } if content == "Call activity"
                        || content.contains("started a call")
                        || content.contains("call notification") =>
                    {
                        Some(PendingCallEntry {
                            id: id.clone(),
                            sender_name: sender_name.clone(),
                            avatar_url: avatar_url.clone(),
                            content: content.clone(),
                            timestamp: timestamp.clone(),
                        })
                    }
                    RenderedTimelineItem::System {
                        id,
                        content,
                        timestamp,
                    } if content.contains("org.matrix.msc3401.call.member")
                        || content.contains("call.member") =>
                    {
                        Some(PendingCallEntry {
                            id: id.clone(),
                            sender_name: sender_name.clone(),
                            avatar_url: avatar_url.clone(),
                            content: "Call member update".to_string(),
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

                flush_call_group(&mut rendered, &mut pending_calls);

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

        flush_call_group(&mut rendered, &mut pending_calls);

        tracing::info!("timeline: produced {} rendered items", rendered.len());
        rendered
    }

    fn update_list_state(&mut self, cx: &mut Context<Self>) {
        let new_count = self.rendered_items.len();
        let old_count = self.list_state.item_count();
        tracing::info!(
            "timeline: syncing list state old_count={} new_count={} loading_history={}",
            old_count,
            new_count,
            self.loading_history
        );

        if old_count == new_count {
            cx.notify();
            return;
        }

        if new_count > old_count {
            let delta = new_count - old_count;
            if self.loading_history {
                self.list_state.splice(0..0, delta);
            } else {
                self.list_state.splice(old_count..old_count, delta);
            }
        } else {
            let delta = old_count - new_count;
            if self.loading_history {
                self.list_state.splice(0..delta, 0);
            } else {
                self.list_state.splice(new_count..old_count, 0);
            }
        }

        cx.notify();
    }

    pub fn send(timeline: Arc<Timeline>, content: String, _cx: &mut App) {
        if content.trim().is_empty() {
            return;
        }

        tokio::spawn(async move {
            let content = RoomMessageEventContent::text_plain(content);
            if let Err(e) = timeline.send(content.into()).await {
                tracing::error!("Failed to send message: {:?}", e);
            }
        });
    }

    pub fn send_message(&self, content: String, _cx: &mut App) {
        Self::send(self.timeline.clone(), content, _cx);
    }
}
