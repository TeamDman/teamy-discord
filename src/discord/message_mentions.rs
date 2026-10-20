use super::mention_target::MentionTarget;
use super::message_list_error::MessageListError;
use super::message_list_report::MessageListReport;
use super::message_list_request::MessageListRequest;
use super::message_page_source::MessagePageSource;

/// Bounded read-only scan, newest first. User mentions come from Discord's structured mentions array.
/// # Errors
/// Stops on upstream errors, wrong bot identity or a page that cannot advance its cursor.
pub async fn scan(
    source: &impl MessagePageSource,
    request: &MessageListRequest,
) -> eyre::Result<MessageListReport> {
    let target = request.mentions.ok_or(MessageListError::InvalidMentions)?;
    let identity = if target == MentionTarget::AuthenticatedBot {
        let identity = source.current_user().await?;
        eyre::ensure!(identity.bot && identity.id != 0, MessageListError::NotBot);
        Some(identity)
    } else {
        None
    };
    let user_id = match target {
        MentionTarget::AuthenticatedBot => identity.as_ref().ok_or(MessageListError::NotBot)?.id,
        MentionTarget::User(id) => id,
    };
    let mut report = MessageListReport {
        messages: Vec::new(),
        mentions_user_id: user_id,
        authenticated_bot: identity,
        limit: request.limit,
        scan_limit: request.scan_limit,
        fetched: 0,
        scanned: 0,
        returned: 0,
        stop_reason: "scan-limit",
        next_before: request.before,
    };
    while report.fetched < usize::from(request.scan_limit) {
        let remaining = usize::from(request.scan_limit) - report.fetched;
        let page_size = u8::try_from(remaining.min(100))?;
        let mut page = source
            .page(request.channel_id, report.next_before, page_size)
            .await?;
        eyre::ensure!(
            page.len() <= usize::from(page_size),
            MessageListError::InvalidPage
        );
        page.sort_by_key(|message| std::cmp::Reverse(message.id));
        let short = page.len() < usize::from(page_size);
        let mut seen = std::collections::HashSet::new();
        for message in &page {
            eyre::ensure!(
                message.channel_id.get() == request.channel_id
                    && seen.insert(message.id)
                    && report
                        .next_before
                        .is_none_or(|before| message.id.get() < before),
                MessageListError::InvalidPage
            );
        }
        report.fetched += page.len();
        for message in page {
            report.scanned += 1;
            report.next_before = Some(message.id.get());
            if message.mentions.iter().any(|user| user.id.get() == user_id) {
                report.messages.push(message);
            }
            if report.messages.len() == usize::from(request.limit) {
                report.returned = report.messages.len();
                report.stop_reason = "result-limit";
                return Ok(report);
            }
        }
        if short {
            report.stop_reason = "no-more-visible-messages";
            break;
        }
    }
    report.returned = report.messages.len();
    Ok(report)
}
