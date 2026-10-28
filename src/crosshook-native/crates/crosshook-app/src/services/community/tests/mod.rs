mod cancellation;
mod helpers;
mod imports;
mod runner;
mod sync;
mod taps;

#[test]
fn community_service_is_send_sync() {
    fn assert_send_sync<T: Send + Sync>() {}
    assert_send_sync::<super::CommunityService>();
}
