//! Blocking file downloads with atomic writes and `sha256` verification.
//!
//! Prism parity: Prism's `NetJob`/`Download` pipeline writes to a temporary
//! file and renames over the target only on success (mirroring `QSaveFile`);
//! here downloads land in `<dest>.part` and are renamed to `dest`.
//!
//! [`download_bytes`] is the unit-testable core (it takes any [`Fetcher`],
//! so tests inject [`MapFetcher`] bytes); [`download_file`] wraps it with a
//! [`BlockingHttpFetcher`] timeout.
//!
//! [`download_many`] spreads many jobs over a small thread pool; it is
//! [`download_many_with_progress`] with a callback that ignores what it is
//! told. The progress spelling exists because a bulk phase of thousands of
//! small files is otherwise silent between starting and finishing, which is
//! indistinguishable from a hang.

use crate::meta::{BlockingHttpFetcher, Fetcher};
use std::ffi::OsString;
use std::path::{Path, PathBuf};
use std::sync::mpsc;
use std::time::Duration;

/// Compute the lowercase hex `sha256` digest of `data`.
///
/// Uses the `sha2` crate (pure Rust) and formats via `LowerHex`, so no manual
/// hex table is needed.
pub fn sha256_hex(data: &[u8]) -> String {
    use sha2::Digest;
    let hash = sha2::Sha256::digest(data);
    format!("{hash:x}")
}

/// Verify that the file at `path` matches the expected hex `sha256` digest.
///
/// Comparison is ASCII case-insensitive and surrounding whitespace on
/// `expected_hex` is ignored. Returns [`crate::Error::HashMismatch`] on a
/// digest mismatch and [`crate::Error::Format`] when `expected_hex` is not a
/// 64-character hex string.
pub fn verify_sha256(path: &Path, expected_hex: &str) -> Result<(), crate::Error> {
    let expected = expected_hex.trim().to_ascii_lowercase();
    if expected.len() != 64 || !expected.bytes().all(is_hex_digit) {
        return Err(crate::Error::format(path, format!("invalid sha256 hex '{expected_hex}'")));
    }
    let bytes =
        std::fs::read(path).map_err(|e| crate::Error::io(path, e))?;
    let actual = sha256_hex(&bytes);
    if actual != expected {
        return Err(crate::Error::hash_mismatch(path, expected, actual));
    }
    Ok(())
}

/// Download `url` via `fetcher` into `dest`, returning the byte count.
///
/// The body is written to `<dest>.part` first and renamed over `dest` only on
/// success; a leftover `.part` file is removed when the write or rename fails.
/// Parent directories are created as needed.
pub fn download_bytes(
    fetcher: &dyn Fetcher,
    url: &str,
    dest: &Path,
) -> Result<u64, crate::Error> {
    let bytes = fetcher.fetch(url)?;
    if let Some(parent) = dest.parent() {
        if !parent.as_os_str().is_empty() {
            std::fs::create_dir_all(parent)
                .map_err(|e| crate::Error::io(parent, e))?;
        }
    }
    let part = part_path(dest);
    let write_result = (|| -> std::io::Result<()> {
        std::fs::write(&part, &bytes)?;
        Ok(())
    })();
    if let Err(e) = write_result {
        let _ = std::fs::remove_file(&part);
        return Err(crate::Error::io(&part, e));
    }
    std::fs::rename(&part, dest).map_err(|e| {
        let _ = std::fs::remove_file(&part);
        crate::Error::io(dest, e)
    })?;
    u64_try_from_usize(bytes.len(), dest)
}

/// Blocking download of `url` into `dest` with a per-request `timeout`,
/// returning the byte count.
///
/// Thin wrapper over [`download_bytes`] with a [`BlockingHttpFetcher`]; the
/// atomic `.part` + rename semantics are identical.
pub fn download_file(url: &str, dest: &Path, timeout: Duration) -> Result<u64, crate::Error> {
    let fetcher = BlockingHttpFetcher::new(timeout);
    download_bytes(&fetcher, url, dest)
}

/// Download many `(url, dest)` jobs in parallel, returning one
/// `(url, result)` pair per job in the input order.
///
/// The chunking, thread count and per-job independence of
/// [`download_many_with_progress`], without the reporting: this is that call
/// with a progress callback that ignores what it is told.
pub fn download_many(
    fetcher: &(dyn Fetcher + Sync),
    jobs: &[(String, PathBuf)],
    threads: usize,
) -> Vec<(String, Result<u64, crate::Error>)> {
    download_many_with_progress(fetcher, jobs, threads, &mut |_, _| {})
}

/// Download many `(url, dest)` jobs in parallel, reporting each finished one.
///
/// `jobs` are split into `threads` contiguous chunks (`threads` is clamped to
/// a minimum of 1 and a maximum of `jobs.len()`; `threads == 0` therefore
/// behaves like 1), and each chunk is downloaded sequentially on its own
/// scoped worker thread via [`download_bytes`]. The scoped threads only
/// borrow `fetcher` and `jobs`, so no `'static` bounds or cloning are needed.
/// Each job is independent: a failing URL yields an `Err` for that entry only
/// and does not affect the other downloads — and it still counts as a finished
/// job below, so a phase's progress reaches its total even when files fail.
///
/// `progress` is called **on the calling thread**, once per finished job and
/// never from a worker, with the number of jobs finished so far and the bytes
/// those jobs have carried (a failed job carries none). That is what makes it
/// usable from a caller that has to log where a long phase is: the events are
/// serialised in completion order, so they need no lock and the callback can
/// borrow a plain `&mut dyn FnMut` logger. Unless a worker panicked — whose
/// unfinished chunk is reported as an error instead — the final call has
/// `jobs.len()` as its count.
pub fn download_many_with_progress(
    fetcher: &(dyn Fetcher + Sync),
    jobs: &[(String, PathBuf)],
    threads: usize,
    progress: &mut dyn FnMut(usize, u64),
) -> Vec<(String, Result<u64, crate::Error>)> {
    if jobs.is_empty() {
        return Vec::new();
    }
    let worker_count = threads.max(1).min(jobs.len());
    let chunk_size = jobs.len().div_ceil(worker_count);
    // Each chunk carries the index of its first job, so a worker can report
    // where what it just finished belongs without the caller sorting results.
    let chunks: Vec<(usize, &[(String, PathBuf)])> = jobs
        .chunks(chunk_size)
        .enumerate()
        .map(|(offset, chunk)| (offset * chunk_size, chunk))
        .collect();
    let mut results: Vec<Option<Result<u64, crate::Error>>> =
        (0..jobs.len()).map(|_| None).collect();
    std::thread::scope(|s| {
        let (finished_tx, finished_rx) = mpsc::channel::<(usize, Result<u64, crate::Error>)>();
        let handles: Vec<_> = chunks
            .iter()
            .map(|(offset, chunk)| {
                let offset = *offset;
                let finished_tx = finished_tx.clone();
                s.spawn(move || {
                    for (index, (url, dest)) in chunk.iter().enumerate() {
                        let result = download_bytes(fetcher, url, dest);
                        // A closed receiver means the caller stopped listening;
                        // there is nowhere left to send anything.
                        if finished_tx.send((offset + index, result)).is_err() {
                            return;
                        }
                    }
                })
            })
            .collect();
        // Dropping the original sender is what ends the loop below: it exits
        // once every worker has dropped its clone and the queue is empty.
        drop(finished_tx);
        let mut finished = 0usize;
        let mut bytes = 0u64;
        while let Ok((index, result)) = finished_rx.recv() {
            if let Ok(count) = &result {
                bytes += count;
            }
            results[index] = Some(result);
            finished += 1;
            progress(finished, bytes);
        }
        // Joined by hand rather than left to the scope, so a worker that
        // panicked costs its own chunk its files instead of panicking this
        // call: those entries stay `None` and become an error below.
        for handle in handles {
            let _ = handle.join();
        }
    });
    results
        .into_iter()
        .enumerate()
        .map(|(index, result)| {
            let (url, dest) = &jobs[index];
            (
                url.clone(),
                result.unwrap_or_else(|| {
                    Err(crate::Error::format(dest, "download worker panicked"))
                }),
            )
        })
        .collect()
}

/// Return the sibling temporary path `<dest>.part` for atomic downloads.
fn part_path(dest: &Path) -> PathBuf {
    let mut os: OsString = dest.as_os_str().to_owned();
    os.push(".part");
    PathBuf::from(os)
}

/// Convert a `usize` byte length to `u64` without panicking on exotic targets.
fn u64_try_from_usize(len: usize, dest: &Path) -> Result<u64, crate::Error> {
    u64::try_from(len)
        .map_err(|_| crate::Error::format(dest, format!("body too large ({len} bytes)")))
}

/// Return `true` for ASCII hex digits (`0-9`, `a-f`, `A-F`).
fn is_hex_digit(b: u8) -> bool {
    matches!(b, b'0'..=b'9' | b'a'..=b'f' | b'A'..=b'F')
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::meta::MapFetcher;

    #[test]
    fn sha256_hex_matches_known_vectors() {
        // echo -n "" | sha256sum
        assert_eq!(
            sha256_hex(b""),
            "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855"
        );
        // echo -n "abc" | sha256sum
        assert_eq!(
            sha256_hex(b"abc"),
            "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
        );
    }

    #[test]
    fn sha256_hex_is_lowercase_hex() {
        let h = sha256_hex(b"prism");
        assert_eq!(h.len(), 64);
        assert!(h.bytes().all(|b| matches!(b, b'0'..=b'9' | b'a'..=b'f')));
    }

    #[test]
    fn verify_sha256_accepts_correct_digest() {
        let tmp = tempfile::tempdir().unwrap();
        let path = tmp.path().join("a.bin");
        std::fs::write(&path, b"abc").unwrap();
        verify_sha256(&path, "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad")
            .unwrap();
    }

    #[test]
    fn verify_sha256_accepts_uppercase_and_whitespace() {
        let tmp = tempfile::tempdir().unwrap();
        let path = tmp.path().join("a.bin");
        std::fs::write(&path, b"abc").unwrap();
        verify_sha256(&path, "  BA7816BF8F01CFEA414140DE5DAE2223B00361A396177A9CB410FF61F20015AD\n")
            .unwrap();
    }

    #[test]
    fn verify_sha256_rejects_mismatch() {
        let tmp = tempfile::tempdir().unwrap();
        let path = tmp.path().join("a.bin");
        std::fs::write(&path, b"abc").unwrap();
        let err = verify_sha256(
            &path,
            "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855",
        )
        .unwrap_err();
        match err {
            crate::Error::HashMismatch { expected, actual, .. } => {
                assert!(expected.starts_with("e3b0"));
                assert_eq!(actual, sha256_hex(b"abc"));
            }
            _ => panic!("expected HashMismatch"),
        }
    }

    #[test]
    fn verify_sha256_rejects_invalid_hex() {
        let tmp = tempfile::tempdir().unwrap();
        let path = tmp.path().join("a.bin");
        std::fs::write(&path, b"abc").unwrap();
        let err = verify_sha256(&path, "not-hex").unwrap_err();
        match err {
            crate::Error::Format { .. } => {}
            _ => panic!("expected Format"),
        }
    }

    #[test]
    fn verify_sha256_missing_file_is_io_error() {
        let tmp = tempfile::tempdir().unwrap();
        let err = verify_sha256(
            &tmp.path().join("missing.bin"),
            "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad",
        )
        .unwrap_err();
        match err {
            crate::Error::Io { .. } => {}
            _ => panic!("expected Io"),
        }
    }

    #[test]
    fn download_bytes_writes_atomically_and_returns_count() {
        let tmp = tempfile::tempdir().unwrap();
        let dest = tmp.path().join("libs").join("a.jar");
        let mut f = MapFetcher::new();
        f.insert("https://x/a.jar", b"payload".to_vec());
        let n = download_bytes(&f, "https://x/a.jar", &dest).unwrap();
        assert_eq!(n, 7);
        assert_eq!(std::fs::read(&dest).unwrap(), b"payload");
        // no .part leftovers
        assert!(!part_path(&dest).exists());
    }

    #[test]
    fn download_bytes_overwrites_existing_target() {
        let tmp = tempfile::tempdir().unwrap();
        let dest = tmp.path().join("a.jar");
        std::fs::write(&dest, b"old").unwrap();
        let mut f = MapFetcher::new();
        f.insert("https://x/a.jar", b"new-bytes".to_vec());
        let n = download_bytes(&f, "https://x/a.jar", &dest).unwrap();
        assert_eq!(n, 9);
        assert_eq!(std::fs::read(&dest).unwrap(), b"new-bytes");
    }

    #[test]
    fn download_bytes_propagates_fetcher_error_and_writes_nothing() {
        let tmp = tempfile::tempdir().unwrap();
        let dest = tmp.path().join("a.jar");
        let f = MapFetcher::new();
        let err = download_bytes(&f, "https://x/missing.jar", &dest).unwrap_err();
        match err {
            crate::Error::Http { .. } => {}
            _ => panic!("expected Http"),
        }
        assert!(!dest.exists());
        assert!(!part_path(&dest).exists());
    }

    #[test]
    fn download_file_reports_http_error_offline() {
        // Unroutable address + short timeout: must fail fast without network.
        let tmp = tempfile::tempdir().unwrap();
        let dest = tmp.path().join("a.jar");
        let err = download_file(
            "http://192.0.2.1/a.jar",
            &dest,
            Duration::from_millis(800),
        )
        .unwrap_err();
        match err {
            crate::Error::Http { url, .. } => assert_eq!(url, "http://192.0.2.1/a.jar"),
            _ => panic!("expected Http"),
        }
        assert!(!dest.exists());
    }

    #[test]
    fn download_many_downloads_all_files_with_multiple_threads() {
        let tmp = tempfile::tempdir().unwrap();
        let mut f = MapFetcher::new();
        let mut jobs: Vec<(String, PathBuf)> = Vec::new();
        for i in 0..8usize {
            let url = format!("https://x/file{i}.bin");
            f.insert(url.clone(), format!("payload-{i}").into_bytes());
            jobs.push((url, tmp.path().join(format!("file{i}.bin"))));
        }
        let results = download_many(&f, &jobs, 4);
        assert_eq!(results.len(), jobs.len());
        for (i, ((url, result), (expected_url, dest))) in
            results.iter().zip(jobs.iter()).enumerate()
        {
            assert_eq!(url, expected_url);
            let expected_body = format!("payload-{i}").into_bytes();
            assert_eq!(*result.as_ref().unwrap(), expected_body.len() as u64);
            assert_eq!(std::fs::read(dest).unwrap(), expected_body);
        }
    }

    #[test]
    fn download_many_zero_threads_behaves_like_one() {
        let tmp = tempfile::tempdir().unwrap();
        let mut f = MapFetcher::new();
        f.insert("https://x/a.bin", b"aaa".to_vec());
        f.insert("https://x/b.bin", b"bb".to_vec());
        let jobs = vec![
            ("https://x/a.bin".to_string(), tmp.path().join("a.bin")),
            ("https://x/b.bin".to_string(), tmp.path().join("b.bin")),
        ];
        let results = download_many(&f, &jobs, 0);
        assert_eq!(results.len(), 2);
        assert_eq!(*results[0].1.as_ref().unwrap(), 3);
        assert_eq!(*results[1].1.as_ref().unwrap(), 2);
        assert_eq!(std::fs::read(&jobs[0].1).unwrap(), b"aaa");
        assert_eq!(std::fs::read(&jobs[1].1).unwrap(), b"bb");
    }

    #[test]
    fn download_many_failure_does_not_poison_others() {
        let tmp = tempfile::tempdir().unwrap();
        let mut f = MapFetcher::new();
        f.insert("https://x/good.bin", b"good".to_vec());
        let jobs = vec![
            ("https://x/good.bin".to_string(), tmp.path().join("good.bin")),
            ("https://x/missing.bin".to_string(), tmp.path().join("missing.bin")),
        ];
        let results = download_many(&f, &jobs, 2);
        assert_eq!(results.len(), 2);
        assert_eq!(results[0].0, "https://x/good.bin");
        assert!(results[0].1.is_ok());
        assert_eq!(results[1].0, "https://x/missing.bin");
        assert!(matches!(results[1].1, Err(crate::Error::Http { .. })));
        assert_eq!(std::fs::read(&jobs[0].1).unwrap(), b"good");
        assert!(!jobs[1].1.exists());
    }

    #[test]
    fn download_many_empty_jobs_returns_empty() {
        let f = MapFetcher::new();
        let results = download_many(&f, &[], 4);
        assert!(results.is_empty());
    }

    #[test]
    fn download_many_with_progress_counts_every_finished_job_including_failures() {
        let tmp = tempfile::tempdir().unwrap();
        let mut f = MapFetcher::new();
        f.insert("https://x/a.bin", b"aaa".to_vec()); // 3 bytes
        f.insert("https://x/c.bin", b"ccccc".to_vec()); // 5 bytes
        let jobs = vec![
            ("https://x/a.bin".to_string(), tmp.path().join("a.bin")),
            // No body for this one: it fails, and still counts as finished.
            ("https://x/missing.bin".to_string(), tmp.path().join("missing.bin")),
            ("https://x/c.bin".to_string(), tmp.path().join("c.bin")),
        ];

        let mut events: Vec<(usize, u64)> = Vec::new();
        let results = download_many_with_progress(&f, &jobs, 2, &mut |finished, bytes| {
            events.push((finished, bytes));
        });

        assert_eq!(events.len(), 3, "one event per job: {events:?}");
        assert_eq!(
            events.iter().map(|(finished, _)| *finished).collect::<Vec<_>>(),
            vec![1, 2, 3],
            "the count rises by one per job, in completion order: {events:?}"
        );
        assert!(
            events.windows(2).all(|pair| pair[0].1 <= pair[1].1),
            "bytes only ever add up: {events:?}"
        );
        assert_eq!(events.last().unwrap().1, 8, "3 + 5 bytes, the failure adds none");
        assert_eq!(results.len(), 3);
        assert!(results[1].1.is_err(), "the missing file still reports its error");
    }

    #[test]
    fn download_many_with_progress_with_no_jobs_reports_nothing() {
        let f = MapFetcher::new();
        let mut calls = 0usize;
        let results = download_many_with_progress(&f, &[], 4, &mut |_, _| calls += 1);
        assert!(results.is_empty());
        assert_eq!(calls, 0, "there is no progress to report before there is work");
    }

    #[test]
    fn download_many_returns_what_download_many_with_progress_returns() {
        let tmp = tempfile::tempdir().unwrap();
        let mut f = MapFetcher::new();
        f.insert("https://x/a.bin", b"aaa".to_vec());
        let jobs = vec![("https://x/a.bin".to_string(), tmp.path().join("a.bin"))];
        let plain = download_many(&f, &jobs, 2);
        let reported = download_many_with_progress(&f, &jobs, 2, &mut |_, _| {});
        assert_eq!(plain.len(), reported.len());
        assert!(plain.iter().all(|(_, result)| result.is_ok()));
        assert!(reported.iter().all(|(_, result)| result.is_ok()));
    }
}
