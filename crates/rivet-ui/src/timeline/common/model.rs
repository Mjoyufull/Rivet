mod formatting;
mod stream;

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
        source: matrix_sdk::ruma::events::room::MediaSource,
        mimetype: Option<String>,
        caption: Option<String>,
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
        arrow: Option<String>,
    },
    CallGroup {
        id: String,
        sender_name: String,
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

    lazy_static::lazy_static! {
        static ref URL_REGEX: regex::Regex = regex::Regex::new(r"(?i)\b((?:https?://|www\d{0,3}[.]|[a-z0-9.\-]+[.][a-z]{2,4}/)(?:[^\s()<>]+|\(([^\s()<>]+|(\([^\s()<>]+\)))*\))+(?:\(([^\s()<>]+|(\([^\s()<>]+\)))*\)|[^\s`!()\[\]{};:'.,<>?«»XY<>?«»“”‘’]))").unwrap();
    }

    let body_string = if URL_REGEX.is_match(body) {
        URL_REGEX.replace_all(body, "[$0]($0)").to_string()
    } else {
        body.to_string()
    };

    if should_use_rich_text(&body_string) {
        RenderedBody::Markdown(body_string)
    } else {
        RenderedBody::Plain(body_string)
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
) -> (String, Option<String>) {
    let target_display_name = membership_change
        .display_name()
        .filter(|name| !name.trim().is_empty())
        .unwrap_or_else(|| fallback_sender_name(membership_change.user_id().as_str()));

    let change = effective_membership_change(membership_change, sender_id);
    let arrow = match change {
        MembershipChange::Joined
        | MembershipChange::Invited
        | MembershipChange::InvitationAccepted
        | MembershipChange::Knocked
        | MembershipChange::KnockAccepted => Some("icons/log-in.svg".to_string()),
        MembershipChange::Left
        | MembershipChange::Banned
        | MembershipChange::KickedAndBanned
        | MembershipChange::Kicked
        | MembershipChange::InvitationRejected
        | MembershipChange::InvitationRevoked
        | MembershipChange::KnockRetracted
        | MembershipChange::KnockDenied => Some("icons/log-out.svg".to_string()),
        _ => None,
    };

    let content = match change {
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
    };
    (content, arrow)
}

fn format_profile_change(
    profile_change: &MemberProfileChange,
    sender_name: &str,
) -> (String, Option<String>) {
    let content = if let Some(displayname) = profile_change.displayname_change() {
        if let Some(previous_name) = &displayname.old {
            if let Some(new_name) = &displayname.new {
                format!("{previous_name} changed their display name to {new_name}.")
            } else {
                format!("{previous_name} removed their display name.")
            }
        } else if let Some(new_name) = &displayname.new {
            format!(
                "{} set their display name to {new_name}.",
                fallback_sender_name(profile_change.user_id().as_str())
            )
        } else {
            format!("{sender_name} updated their profile.")
        }
    } else if let Some(avatar_url) = profile_change.avatar_url_change() {
        if avatar_url.old.is_none() {
            format!("{sender_name} set their avatar.")
        } else if avatar_url.new.is_none() {
            format!("{sender_name} removed their avatar.")
        } else {
            format!("{sender_name} changed their avatar.")
        }
    } else {
        format!("{sender_name} updated their profile.")
    };
    (content, None)
}

fn format_other_state(
    other_state: &OtherState,
    sender_name: &str,
) -> Option<(String, Option<String>)> {
    let (content, arrow) = match other_state.content() {
        AnyOtherFullStateEventContent::RoomCreate(_) => (
            format!("{sender_name} created the room."),
            Some("icons/log-in.svg".to_string()),
        ),
        AnyOtherFullStateEventContent::RoomEncryption(_) => (
            "This room is encrypted from this point on.".to_string(),
            None,
        ),
        AnyOtherFullStateEventContent::RoomName(_) => {
            (format!("{sender_name} changed the room name."), None)
        }
        AnyOtherFullStateEventContent::RoomTopic(_) => {
            (format!("{sender_name} changed the room topic."), None)
        }
        AnyOtherFullStateEventContent::RoomAvatar(_) => {
            (format!("{sender_name} changed the room avatar."), None)
        }
        AnyOtherFullStateEventContent::RoomAliases(_) => {
            (format!("{sender_name} updated the room aliases."), None)
        }
        AnyOtherFullStateEventContent::RoomCanonicalAlias(_) => {
            (format!("{sender_name} changed the room alias."), None)
        }
        AnyOtherFullStateEventContent::RoomGuestAccess(_) => {
            (format!("{sender_name} changed guest access."), None)
        }
        AnyOtherFullStateEventContent::RoomHistoryVisibility(_) => {
            (format!("{sender_name} changed history visibility."), None)
        }
        AnyOtherFullStateEventContent::RoomJoinRules(_) => {
            (format!("{sender_name} changed join rules."), None)
        }
        AnyOtherFullStateEventContent::RoomPinnedEvents(_) => {
            (format!("{sender_name} updated pinned messages."), None)
        }
        AnyOtherFullStateEventContent::RoomPowerLevels(_) => {
            (format!("{sender_name} updated room permissions."), None)
        }
        AnyOtherFullStateEventContent::RoomServerAcl(_) => {
            (format!("{sender_name} updated the server ACL."), None)
        }
        AnyOtherFullStateEventContent::RoomThirdPartyInvite(_) => {
            (format!("{sender_name} created a third-party invite."), None)
        }
        AnyOtherFullStateEventContent::RoomTombstone(_) => (
            "This room has been replaced with a newer room.".to_string(),
            None,
        ),
        AnyOtherFullStateEventContent::SpaceChild(_)
        | AnyOtherFullStateEventContent::SpaceParent(_) => {
            (format!("{sender_name} updated the space hierarchy."), None)
        }
        AnyOtherFullStateEventContent::PolicyRuleRoom(_)
        | AnyOtherFullStateEventContent::PolicyRuleServer(_)
        | AnyOtherFullStateEventContent::PolicyRuleUser(_) => {
            (format!("{sender_name} updated a room policy."), None)
        }
        AnyOtherFullStateEventContent::_Custom { event_type } => {
            if event_type == "org.matrix.msc3401.call.member" {
                return None;
            }
            (
                format!("{sender_name} sent a state event: {event_type}."),
                None,
            )
        }
    };
    Some((content, arrow))
}

pub fn room_media_source_url(source: &matrix_sdk::ruma::events::room::MediaSource) -> String {
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

fn optional_caption(body: &str) -> Option<String> {
    (!body.trim().is_empty()).then(|| body.to_string())
}

fn optional_caption_str(body: &str) -> Option<String> {
    (!body.trim().is_empty()).then(|| body.to_string())
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

        stream::install_scroll_handler(&model, cx);
        model
    }

    pub fn load_more_history(&mut self, model_handle: Entity<Self>, cx: &mut App) {
        stream::load_more_history(self, model_handle, cx);
    }

    pub fn init(model: Entity<Self>, cx: &mut App) {
        stream::init(model, cx);
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
}
