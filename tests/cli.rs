use std::{
    fs,
    io::{BufRead, BufReader, Read},
    net::UdpSocket,
    path::PathBuf,
    process::{Child, Command, Output, Stdio},
    sync::{
        atomic::{AtomicU64, Ordering},
        mpsc,
    },
    thread,
    time::{Duration, Instant},
};
static NEXT: AtomicU64 = AtomicU64::new(0);
struct Directory(PathBuf);
impl Directory {
    fn new() -> Self {
        let path = std::env::temp_dir().join(format!(
            "gdl90-cli-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir(&path).unwrap();
        Self(path)
    }
}
impl Drop for Directory {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}
struct Process(Option<Child>);
impl Process {
    fn spawn(args: &[&str]) -> Self {
        Self(Some(
            Command::new(env!("CARGO_BIN_EXE_gdl90"))
                .args(args)
                .stdin(Stdio::null())
                .stdout(Stdio::piped())
                .stderr(Stdio::piped())
                .spawn()
                .unwrap(),
        ))
    }
    fn finish(mut self) -> Output {
        let deadline = Instant::now() + Duration::from_secs(10);
        while self.0.as_mut().unwrap().try_wait().unwrap().is_none() {
            assert!(
                Instant::now() < deadline,
                "CLI did not finish before deadline"
            );
            thread::sleep(Duration::from_millis(10));
        }
        self.0.take().unwrap().wait_with_output().unwrap()
    }
}
impl Drop for Process {
    fn drop(&mut self) {
        if let Some(child) = &mut self.0 {
            let _ = child.kill();
            let _ = child.wait();
        }
    }
}
#[test]
fn invalid_arguments_fail_before_creating_output() {
    let dir = Directory::new();
    let output = dir.0.join("must-not-exist.txt");
    for args in [
        vec!["unknown"],
        vec!["--help", "extra"],
        vec!["support-status", "--typo"],
        vec![
            "capture",
            "127.0.0.1:0",
            output.to_str().unwrap(),
            "1",
            "extra",
        ],
    ] {
        assert!(!Process::spawn(&args).finish().status.success());
    }
    assert!(!output.exists());
    assert!(Process::spawn(&["--help"]).finish().status.success());
}
#[test]
fn replay_rejects_unreasonable_delay_before_sending() {
    let dir = Directory::new();
    let input = dir.0.join("input.txt");
    fs::write(&input, "@18446744073709551615 7E04000040007E\n").unwrap();
    let receiver = UdpSocket::bind("127.0.0.1:0").unwrap();
    receiver
        .set_read_timeout(Some(Duration::from_millis(100)))
        .unwrap();
    let target = receiver.local_addr().unwrap().to_string();
    assert!(
        !Process::spawn(&["replay-file", input.to_str().unwrap(), &target])
            .finish()
            .status
            .success()
    );
    assert!(receiver.recv(&mut [0; 64]).is_err());
}
#[test]
fn capture_survives_oversize_and_empty_datagrams_then_records_valid_packet() {
    let dir = Directory::new();
    let path = dir.0.join("capture.txt");
    let mut child = Process::spawn(&["capture", "127.0.0.1:0", path.to_str().unwrap(), "1"]);
    let stdout = child.0.as_mut().unwrap().stdout.take().unwrap();
    let (tx, rx) = mpsc::channel();
    let reader = thread::spawn(move || {
        let mut reader = BufReader::new(stdout);
        let mut first = String::new();
        reader.read_line(&mut first).unwrap();
        tx.send(first).unwrap();
        let mut rest = String::new();
        reader.read_to_string(&mut rest).unwrap();
        rest
    });
    let ready = rx
        .recv_timeout(Duration::from_secs(5))
        .expect("capture did not announce bound socket");
    let addr = ready.split_whitespace().nth(2).unwrap();
    let socket = UdpSocket::bind("127.0.0.1:0").unwrap();
    socket.send_to(&[0; 4096], addr).unwrap();
    socket.send_to(&[], addr).unwrap();
    let valid = gdl90::frame::encode_frame(&[4, 0, 0]);
    socket.send_to(&valid, addr).unwrap();
    let output = child.finish();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    reader.join().unwrap();
    let records = gdl90::session::read_datagram_file(&path).unwrap();
    assert_eq!(records.len(), 1);
    assert_eq!(records[0].bytes, valid);
    assert_eq!(records[0].delay_ms, Some(0));
}
#[test]
fn closed_stdout_does_not_panic() {
    let mut process = Process::spawn(&["support-status"]);
    drop(process.0.as_mut().unwrap().stdout.take());
    let result = process.finish();
    assert!(
        result.status.success(),
        "{}",
        String::from_utf8_lossy(&result.stderr)
    );
    assert!(!String::from_utf8_lossy(&result.stderr).contains("panicked"));
}
