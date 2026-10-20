use serenity::all::ChannelId;
use serenity::all::Message;
use serenity::all::MessageId;
use serenity::all::User;
use serenity::all::UserId;
use std::collections::VecDeque;
use std::sync::Mutex;
use std::sync::atomic::AtomicUsize;
use std::sync::atomic::Ordering;
use teamy_discord::discord::mention_identity::MentionIdentity;
use teamy_discord::discord::message_list_request::MessageListRequest;
use teamy_discord::discord::message_mentions::scan;
use teamy_discord::discord::message_page_source::MessagePageSource;

struct MockHttp {
    pages: Mutex<VecDeque<Vec<Message>>>,
    requests: Mutex<Vec<(u64, Option<u64>, u8)>>,
    identity_calls: AtomicUsize,
    bot: bool,
}
impl MockHttp {
    fn new(pages: Vec<Vec<Message>>) -> Self {
        Self {
            pages: Mutex::new(pages.into()),
            requests: Mutex::new(Vec::new()),
            identity_calls: AtomicUsize::new(0),
            bot: true,
        }
    }
}
impl MessagePageSource for MockHttp {
    fn current_user(&self) -> impl Future<Output = eyre::Result<MentionIdentity>> + Send {
        self.identity_calls.fetch_add(1, Ordering::SeqCst);
        std::future::ready(Ok(MentionIdentity {
            id: 42,
            username: "SyntheticBot".into(),
            bot: self.bot,
        }))
    }
    fn page(
        &self,
        channel: u64,
        before: Option<u64>,
        limit: u8,
    ) -> impl Future<Output = eyre::Result<Vec<Message>>> + Send {
        self.requests.lock().unwrap().push((channel, before, limit));
        // Round-trip real HTTP message JSON, rather than substituting a simplified message model.
        let result = self
            .pages
            .lock()
            .unwrap()
            .pop_front()
            .ok_or_else(|| eyre::eyre!("Synthetic HTTP failure"))
            .and_then(|page| Ok(serde_json::from_slice(&serde_json::to_vec(&page)?)?));
        std::future::ready(result)
    }
}
fn message(id: u64, mention: Option<u64>) -> Message {
    let mut message = Message::default();
    message.id = MessageId::new(id);
    message.channel_id = ChannelId::new(20);
    if let Some(id) = mention {
        let mut user = User::default();
        user.id = UserId::new(id);
        message.mentions.push(user);
    }
    message
}
#[tokio::test]
async fn me_uses_authenticated_bot_and_only_structured_user_mentions() {
    let mut content_only = message(104, None);
    content_only.content = "<@42>".into();
    let mut everyone = message(102, None);
    everyone.mention_everyone = true;
    let http = MockHttp::new(vec![vec![
        message(105, Some(43)),
        content_only,
        message(103, Some(42)),
        everyone,
        message(101, Some(42)),
    ]]);
    let request = MessageListRequest::new(20, Some(2), None, Some("me"), Some(10)).unwrap();
    let report = scan(&http, &request).await.unwrap();
    assert_eq!(
        report
            .messages
            .iter()
            .map(|m| m.id.get())
            .collect::<Vec<_>>(),
        [103, 101]
    );
    assert_eq!(report.mentions_user_id, 42);
    assert_eq!(report.authenticated_bot.unwrap().username, "SyntheticBot");
    assert_eq!(report.stop_reason, "result-limit");
    assert_eq!(http.identity_calls.load(Ordering::SeqCst), 1);
}
#[tokio::test]
async fn numeric_filter_pages_with_fixed_ceiling_and_monotonic_cursor() {
    let first = (101..=200).rev().map(|id| message(id, None)).collect();
    let second = (94..=100)
        .rev()
        .map(|id| message(id, (id == 95).then_some(42)))
        .collect();
    let http = MockHttp::new(vec![first, second]);
    let request = MessageListRequest::new(20, Some(10), None, Some("42"), Some(107)).unwrap();
    let report = scan(&http, &request).await.unwrap();
    assert_eq!(report.fetched, 107);
    assert_eq!(report.scanned, 107);
    assert_eq!(report.returned, 1);
    assert_eq!(report.stop_reason, "scan-limit");
    assert_eq!(report.next_before, Some(94));
    assert_eq!(
        *http.requests.lock().unwrap(),
        [(20, None, 100), (20, Some(101), 7)]
    );
    assert_eq!(http.identity_calls.load(Ordering::SeqCst), 0);
}
#[tokio::test]
async fn result_limit_reports_examined_cursor_and_actual_fetched_count() {
    let http = MockHttp::new(vec![vec![
        message(4, None),
        message(3, Some(42)),
        message(2, Some(42)),
        message(1, None),
    ]]);
    let request = MessageListRequest::new(20, Some(1), None, Some("42"), Some(100)).unwrap();
    let report = scan(&http, &request).await.unwrap();
    assert_eq!(report.returned, 1);
    assert_eq!(report.scanned, 2);
    assert_eq!(report.fetched, 4);
    assert_eq!(report.next_before, Some(3));
    assert_eq!(report.stop_reason, "result-limit");
}
#[tokio::test]
async fn empty_page_is_visible_scope_only_and_user_account_never_substitutes_owner() {
    let http = MockHttp::new(vec![vec![]]);
    let request = MessageListRequest::new(20, Some(10), None, Some("42"), None).unwrap();
    let report = scan(&http, &request).await.unwrap();
    assert_eq!(report.stop_reason, "no-more-visible-messages");
    assert_eq!(report.scanned, 0);
    let mut http = MockHttp::new(vec![]);
    http.bot = false;
    let request = MessageListRequest::new(20, None, None, Some("me"), None).unwrap();
    assert!(
        scan(&http, &request)
            .await
            .unwrap_err()
            .to_string()
            .contains("authenticated bot account")
    );
    assert!(http.requests.lock().unwrap().is_empty());
}
#[tokio::test]
async fn invalid_pages_and_upstream_failure_stop_without_retry_or_loop() {
    for page in [
        vec![message(4, None), message(4, None)],
        vec![message(10, None)],
    ] {
        let http = MockHttp::new(vec![page]);
        let request = MessageListRequest::new(20, None, Some(10), Some("42"), None).unwrap();
        let _ = scan(&http, &request).await.unwrap_err();
        assert_eq!(http.requests.lock().unwrap().len(), 1);
    }
    let http = MockHttp::new(vec![]);
    let request = MessageListRequest::new(20, None, None, Some("42"), None).unwrap();
    let _ = scan(&http, &request).await.unwrap_err();
    assert_eq!(http.requests.lock().unwrap().len(), 1);
}
#[test]
fn typed_request_rejects_invalid_filters_and_unbounded_scans_without_echoing_values() {
    for target in ["0", "@me", "owner", "PRIVATE_SENTINEL"] {
        let error = MessageListRequest::new(20, None, None, Some(target), None).unwrap_err();
        assert!(error.to_string().contains("Invalid --mentions"));
        assert!(!error.to_string().contains("PRIVATE_SENTINEL"));
    }
    for limit in [0, 1001] {
        let _ = MessageListRequest::new(20, None, None, Some("me"), Some(limit)).unwrap_err();
    }
    let _ = MessageListRequest::new(20, None, None, None, Some(100)).unwrap_err();
}
