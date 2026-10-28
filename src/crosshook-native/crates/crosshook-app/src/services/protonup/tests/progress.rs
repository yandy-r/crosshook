use crosshook_core::protonup::progress::{Phase, ProgressEmitter};

use super::super::progress::{forward_progress, next_progress};

#[tokio::test]
async fn next_progress_returns_snapshots_then_none_when_closed() {
    let (emitter, mut rx) = ProgressEmitter::new("op");
    emitter.emit(Phase::Resolving, 0, None, None);
    drop(emitter);
    let first = next_progress(&mut rx).await.expect("snapshot");
    assert_eq!(first.op_id, "op");
    assert!(matches!(first.phase, Phase::Resolving));
    assert!(next_progress(&mut rx).await.is_none());
}

#[tokio::test]
async fn next_progress_continues_after_lagged() {
    let (emitter, mut rx) = ProgressEmitter::new("op");
    // The channel retains 64 snapshots; overflow it so the receiver lags.
    for done in 0..200u64 {
        emitter.emit(Phase::Downloading, done, Some(200), None);
    }
    emitter.emit(Phase::Done, 200, Some(200), None);
    drop(emitter);
    let mut last = None;
    while let Some(progress) = next_progress(&mut rx).await {
        last = Some(progress);
    }
    let last = last.expect("at least one snapshot survives lag");
    assert!(matches!(last.phase, Phase::Done));
}

#[tokio::test]
async fn forward_progress_relays_with_sanitized_messages_and_reports_terminal() {
    let (core, core_rx) = ProgressEmitter::new("op");
    let (out, mut out_rx) = ProgressEmitter::new("op");
    core.emit(
        Phase::Downloading,
        5,
        Some(10),
        Some("https://host.example/file".into()),
    );
    core.emit(Phase::Verifying, 10, Some(10), Some("verifying".into()));
    core.emit(Phase::Failed, 0, None, Some("failed at /tmp/x".into()));
    drop(core);

    assert!(forward_progress(core_rx, out).await);

    let first = next_progress(&mut out_rx).await.unwrap();
    assert!(matches!(first.phase, Phase::Downloading));
    assert_eq!((first.bytes_done, first.bytes_total), (5, Some(10)));
    assert_eq!(first.message, None);
    let second = next_progress(&mut out_rx).await.unwrap();
    assert!(matches!(second.phase, Phase::Verifying));
    assert_eq!(second.message.as_deref(), Some("verifying"));
    let third = next_progress(&mut out_rx).await.unwrap();
    assert!(matches!(third.phase, Phase::Failed));
    assert_eq!(third.message.as_deref(), Some("Proton install failed"));
    assert!(next_progress(&mut out_rx).await.is_none());
}

#[tokio::test]
async fn forward_progress_without_terminal_phase_returns_false() {
    let (core, core_rx) = ProgressEmitter::new("op");
    let (out, mut out_rx) = ProgressEmitter::new("op");
    core.emit(Phase::Resolving, 0, None, None);
    drop(core);
    assert!(!forward_progress(core_rx, out).await);
    assert!(matches!(
        next_progress(&mut out_rx).await.unwrap().phase,
        Phase::Resolving
    ));
}

#[tokio::test]
async fn forward_progress_continues_after_lagged_and_still_sees_terminal() {
    let (core, core_rx) = ProgressEmitter::new("op");
    let (out, mut out_rx) = ProgressEmitter::new("op");
    for done in 0..200u64 {
        core.emit(Phase::Downloading, done, None, None);
    }
    core.emit(Phase::Cancelled, 0, None, None);
    drop(core);
    // `out` retains only its own capacity, so drain concurrently.
    let drain = tokio::spawn(async move {
        let mut phases = Vec::new();
        while let Some(progress) = next_progress(&mut out_rx).await {
            phases.push(progress.phase);
        }
        phases
    });
    assert!(forward_progress(core_rx, out).await);
    let phases = drain.await.unwrap();
    assert!(matches!(phases.last(), Some(Phase::Cancelled)));
}
