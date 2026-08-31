//! CLI integration tests for `gap send` / `gap receive`.
#![allow(
    unused_crate_dependencies,
    reason = "integration tests share the package graph; only duct, tempfile, and iroh-blobs are used"
)]

#[cfg(test)]
mod tests {
    use std::io::{self, Read};
    use std::panic::{AssertUnwindSafe, catch_unwind, resume_unwind};
    use std::path::{Path, PathBuf};
    use std::str::FromStr;
    use std::sync::{Arc, Mutex, mpsc};
    use std::time::Duration;

    use iroh_blobs::ticket::BlobTicket;

    const FILE_TEST_TIMEOUT: Duration = Duration::from_secs(60);
    const DIR_TEST_TIMEOUT: Duration = Duration::from_secs(120);

    const fn gap_bin() -> &'static str {
        env!("CARGO_BIN_EXE_gap")
    }

    fn always_err(msg: &'static str) -> Result<(), &'static str> {
        Err(msg)
    }

    fn test_fail(msg: &'static str) -> ! {
        loop {
            always_err(msg).expect(msg);
        }
    }

    /// Read `n` ASCII lines from `reader`, including the newlines.
    fn read_ascii_lines(mut n: usize, reader: &mut impl Read) -> io::Result<Vec<u8>> {
        let mut buf = [0u8; 1];
        let mut res = Vec::new();
        loop {
            if reader.read(&mut buf)? != 1 {
                break;
            }
            let [ch] = buf;
            res.push(ch);
            if ch != b'\n' {
                continue;
            }
            if n > 1 {
                n -= 1;
            } else {
                break;
            }
        }
        Ok(res)
    }

    type Kill = Box<dyn FnOnce() + Send>;

    #[derive(Clone)]
    struct TimeoutCtl {
        kills: Arc<Mutex<Vec<Kill>>>,
    }

    impl TimeoutCtl {
        fn new() -> Self {
            Self {
                kills: Arc::new(Mutex::new(Vec::new())),
            }
        }

        fn on_timeout(&self, kill: Kill) {
            self.kills.lock().expect("timeout kills mutex").push(kill);
        }

        fn fire(&self) {
            let kills = std::mem::take(&mut *self.kills.lock().expect("timeout kills mutex"));
            for kill in kills {
                kill();
            }
        }
    }

    fn timeout_test(limit: Duration, body: impl FnOnce(TimeoutCtl) + Send + 'static) {
        let ctl = TimeoutCtl::new();
        let ctl_timeout = ctl.clone();
        let (tx, rx) = mpsc::channel();
        std::thread::spawn(move || {
            let outcome = catch_unwind(AssertUnwindSafe(|| body(ctl)));
            drop(tx.send(outcome));
        });
        match rx.recv_timeout(limit) {
            Ok(Ok(())) => {}
            Ok(Err(payload)) => resume_unwind(payload),
            Err(mpsc::RecvTimeoutError::Timeout) => {
                ctl_timeout.fire();
                test_fail("cli test exceeded timeout; --relay disabled loopback did not finish");
            }
            Err(mpsc::RecvTimeoutError::Disconnected) => {
                test_fail("cli test worker dropped the channel without finishing");
            }
        }
    }

    const fn send_args(path: &str) -> [&str; 7] {
        [
            "send",
            path,
            "--relay",
            "disabled",
            "--no-progress",
            "--magic-ipv4-addr",
            "127.0.0.1:0",
        ]
    }

    const fn recv_args(ticket: &str) -> [&str; 7] {
        [
            "receive",
            ticket,
            "--relay",
            "disabled",
            "--no-progress",
            "--magic-ipv4-addr",
            "127.0.0.1:0",
        ]
    }

    fn ticket_from_send_stdout(output: &[u8]) -> BlobTicket {
        let text = String::from_utf8(output.to_vec()).expect("send stdout utf8");
        let ticket = text
            .split_ascii_whitespace()
            .next_back()
            .expect("ticket token on send stdout");
        BlobTicket::from_str(ticket).expect("parse BlobTicket")
    }

    fn register_kill(ctl: &TimeoutCtl, kill: impl FnOnce() + Send + 'static) {
        ctl.on_timeout(Box::new(kill));
    }

    fn create_file(base: &Path, i: usize, j: usize, k: usize) -> (PathBuf, Vec<u8>) {
        let name = base
            .join(format!("dir-{i}"))
            .join(format!("subdir-{j}"))
            .join(format!("file-{k}"));
        let len = i * 100 + j * 10 + k;
        let data = vec![0u8; len];
        (name, data)
    }

    fn all_coords() -> impl Iterator<Item = (usize, usize, usize)> {
        (0..5).flat_map(|i| (0..5).flat_map(move |j| (0..5).map(move |k| (i, j, k))))
    }

    fn write_src_tree(src_data_dir: &Path) {
        for (i, j, k) in all_coords() {
            let (name, data) = create_file(src_data_dir, i, j, k);
            std::fs::create_dir_all(name.parent().expect("parent")).expect("mkdir");
            std::fs::write(&name, &data).expect("write dir file");
        }
    }

    fn check_tgt_tree(tgt_data_dir: &Path) {
        for (i, j, k) in all_coords() {
            let (name, data) = create_file(tgt_data_dir, i, j, k);
            let tgt_data = std::fs::read(&name).expect("read received dir file");
            assert_eq!(
                tgt_data, data,
                "dir file {i}/{j}/{k} bytes differ from source"
            );
        }
    }

    #[test]
    fn send_recv_file() {
        timeout_test(FILE_TEST_TIMEOUT, |ctl| {
            let name = "somefile.bin";
            let data = vec![0u8; 100];
            let src_dir = tempfile::tempdir().expect("src tempdir");
            let tgt_dir = tempfile::tempdir().expect("tgt tempdir");
            let src_file = src_dir.path().join(name);
            std::fs::write(&src_file, &data).expect("write source file");
            let path = src_file.to_str().expect("src path utf8");
            let send = Arc::new(
                duct::cmd(gap_bin(), send_args(path))
                    .dir(src_dir.path())
                    .env_remove("RUST_LOG")
                    .reader()
                    .expect("spawn send"),
            );
            {
                let send = Arc::clone(&send);
                register_kill(&ctl, move || drop(send.kill()));
            }
            let send_out = read_ascii_lines(3, &mut &*send).expect("read send stdout");
            let ticket = ticket_from_send_stdout(&send_out);
            let recv = Arc::new(
                duct::cmd(gap_bin(), recv_args(&ticket.to_string()))
                    .dir(tgt_dir.path())
                    .env_remove("RUST_LOG")
                    .start()
                    .expect("spawn receive"),
            );
            {
                let recv = Arc::clone(&recv);
                register_kill(&ctl, move || drop(recv.kill()));
            }
            let recv_out = recv.wait().expect("receive wait");
            assert!(recv_out.status.success(), "receive failed: {recv_out:?}");
            let tgt_file = tgt_dir.path().join(name);
            let tgt_data = std::fs::read(tgt_file).expect("read received file");
            assert_eq!(tgt_data, data, "received file bytes differ from source");
        });
    }

    #[test]
    fn receive_closes_endpoint_no_iroh_socket_error() {
        timeout_test(FILE_TEST_TIMEOUT, |ctl| {
            let name = "graceful-close.bin";
            let data = vec![0xabu8; 64];
            let src_dir = tempfile::tempdir().expect("src tempdir");
            let tgt_dir = tempfile::tempdir().expect("tgt tempdir");
            let src_file = src_dir.path().join(name);
            std::fs::write(&src_file, &data).expect("write source file");
            let path = src_file.to_str().expect("src path utf8");
            let send = Arc::new(
                duct::cmd(gap_bin(), send_args(path))
                    .dir(src_dir.path())
                    .env_remove("RUST_LOG")
                    .reader()
                    .expect("spawn send"),
            );
            {
                let send = Arc::clone(&send);
                register_kill(&ctl, move || drop(send.kill()));
            }
            let send_out = read_ascii_lines(3, &mut &*send).expect("read send stdout");
            let ticket = ticket_from_send_stdout(&send_out);
            let recv = Arc::new(
                duct::cmd(gap_bin(), recv_args(&ticket.to_string()))
                    .dir(tgt_dir.path())
                    .env("RUST_LOG", "iroh::socket=error")
                    .stdout_capture()
                    .stderr_capture()
                    .start()
                    .expect("spawn receive"),
            );
            {
                let recv = Arc::clone(&recv);
                register_kill(&ctl, move || drop(recv.kill()));
            }
            let recv_out = recv.wait().expect("receive wait");
            assert!(recv_out.status.success(), "receive failed: {recv_out:?}");
            let stderr = String::from_utf8_lossy(&recv_out.stderr);
            assert!(
                !stderr.contains("Endpoint dropped"),
                "unexpected iroh shutdown log on stderr: {stderr}"
            );
            assert!(
                !stderr.contains("Aborting ungracefully"),
                "unexpected iroh shutdown log on stderr: {stderr}"
            );
            let tgt_file = tgt_dir.path().join(name);
            assert_eq!(
                std::fs::read(&tgt_file).expect("read received file"),
                data,
                "received file bytes differ from source"
            );
        });
    }

    #[test]
    fn send_recv_dir() {
        timeout_test(DIR_TEST_TIMEOUT, |ctl| {
            let src_dir = tempfile::tempdir().expect("src tempdir");
            let tgt_dir = tempfile::tempdir().expect("tgt tempdir");
            let src_data_dir = src_dir.path().join("data");
            let tgt_data_dir = tgt_dir.path().join("data");
            write_src_tree(&src_data_dir);
            let path = src_data_dir.to_str().expect("src data path utf8");
            let send = Arc::new(
                duct::cmd(gap_bin(), send_args(path))
                    .dir(src_dir.path())
                    .env_remove("RUST_LOG")
                    .reader()
                    .expect("spawn send"),
            );
            {
                let send = Arc::clone(&send);
                register_kill(&ctl, move || drop(send.kill()));
            }
            let send_out = read_ascii_lines(3, &mut &*send).expect("read send stdout");
            let ticket = ticket_from_send_stdout(&send_out);
            let recv = Arc::new(
                duct::cmd(gap_bin(), recv_args(&ticket.to_string()))
                    .dir(tgt_dir.path())
                    .env_remove("RUST_LOG")
                    .start()
                    .expect("spawn receive"),
            );
            {
                let recv = Arc::clone(&recv);
                register_kill(&ctl, move || drop(recv.kill()));
            }
            let recv_out = recv.wait().expect("receive wait");
            assert!(recv_out.status.success(), "receive failed: {recv_out:?}");
            check_tgt_tree(&tgt_data_dir);
        });
    }
}
