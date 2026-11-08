use super::*;
use crosshook_core::protonup::progress::Phase;
use serde_json::json;

#[test]
fn progress_event_keeps_the_frozen_name_and_snake_case_fields() {
    let event = progress_event(&ProtonInstallProgress {
        op_id: "job-1".into(),
        phase: Phase::Downloading,
        bytes_done: 5,
        bytes_total: None,
        message: None,
    })
    .expect("serializes");
    assert_eq!(event.name, "protonup:install:progress");
    assert_eq!(
        event.value(),
        json!({
            "op_id": "job-1",
            "phase": "downloading",
            "bytes_done": 5,
            "bytes_total": null,
            "message": null
        })
    );
}
