use super::*;
use std::io::{PipeReader, PipeWriter, Write};

const SHORT: Duration = Duration::from_millis(150);

fn pipe() -> (PipeReader, PipeWriter) {
    std::io::pipe().expect("pipe")
}

#[test]
fn an_exact_go_line_is_accepted() {
    let (mut reader, mut writer) = pipe();
    writer.write_all(b"GO\n").unwrap();
    assert_eq!(read_go(&mut reader, Duration::from_secs(5)), Ok(()));
}

#[test]
fn a_go_line_split_across_writes_is_accepted() {
    let (mut reader, mut writer) = pipe();
    let feeder = std::thread::spawn(move || {
        writer.write_all(b"G").unwrap();
        std::thread::sleep(Duration::from_millis(40));
        writer.write_all(b"O\n").unwrap();
        writer
    });
    assert_eq!(read_go(&mut reader, Duration::from_secs(5)), Ok(()));
    drop(feeder.join().unwrap());
}

#[test]
fn anything_but_go_is_rejected() {
    for bytes in [
        &b"NO\n"[..],
        b"go\n",
        b"GO",
        b"GO\r\n",
        b"GO\nGO\n",
        b"GOGO\n",
        b"X",
        b"\n",
        b"GO \n",
        b"GOOD LUCK, EVERYONE",
    ] {
        let (mut reader, mut writer) = pipe();
        writer.write_all(bytes).unwrap();
        // Keep the write end open so a short prefix such as `GO` times out instead of reading EOF.
        let error = read_go(&mut reader, SHORT).unwrap_err();
        assert!(
            error.contains("other than the GO line") || error.contains("timed out"),
            "{bytes:?}: {error}"
        );
        assert!(!error.contains("NO") && !error.contains("LUCK"), "{error}");
    }
}

#[test]
fn a_wrong_first_byte_is_rejected_without_waiting() {
    let (mut reader, mut writer) = pipe();
    writer.write_all(b"X").unwrap();
    let started = Instant::now();
    let error = read_go(&mut reader, Duration::from_secs(30)).unwrap_err();
    assert!(error.contains("other than the GO line"), "{error}");
    assert!(started.elapsed() < Duration::from_secs(5));
}

#[test]
fn eof_is_rejected() {
    let (mut reader, writer) = pipe();
    drop(writer);
    assert_eq!(
        read_go(&mut reader, Duration::from_secs(5)),
        Err("stdin closed before the GO line".to_string())
    );
}

#[test]
fn eof_after_a_partial_line_is_rejected() {
    let (mut reader, mut writer) = pipe();
    writer.write_all(b"G").unwrap();
    drop(writer);
    assert_eq!(
        read_go(&mut reader, Duration::from_secs(5)),
        Err("stdin closed before the GO line".to_string())
    );
}

#[test]
fn silence_times_out() {
    let (mut reader, _writer) = pipe();
    let started = Instant::now();
    assert_eq!(
        read_go(&mut reader, SHORT),
        Err("timed out waiting for the GO line on stdin".to_string())
    );
    assert!(started.elapsed() >= SHORT);
}

#[tokio::test]
async fn the_thread_answer_reaches_the_async_caller() {
    let (mut reader, mut writer) = pipe();
    writer.write_all(b"GO\n").unwrap();
    let go = await_reader(move || read_go(&mut reader, Duration::from_secs(5))).await;
    assert_eq!(go, Ok(()));

    let (mut reader, writer) = pipe();
    drop(writer);
    let go = await_reader(move || read_go(&mut reader, Duration::from_secs(5))).await;
    assert_eq!(go, Err("stdin closed before the GO line".to_string()));
}

#[tokio::test]
async fn a_panicking_reader_is_reported_not_hung() {
    let go = await_reader(|| panic!("reader blew up")).await;
    assert_eq!(
        go,
        Err("the GO reader stopped without an answer".to_string())
    );
}
