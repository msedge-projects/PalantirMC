//! The launcher's first file picker: one path, from the reader's own disk.
//!
//! Until now this launcher has never asked Windows for a file. The import flow
//! *scans* the places other launchers keep their instances in, and that works
//! because there is one right answer to find and it is in a known place. A skin is
//! the opposite: it is wherever the reader saved it, so the launcher has to ask, and
//! asking means a modal dialog owned by this window.
//!
//! Three decisions are worth the space.
//!
//! * **`GetOpenFileNameW`, not `IFileOpenDialog`.** The modern picker is a COM
//!   interface: `CoInitializeEx`, a dozen `Set…` calls on an object, and a
//!   lifetime decision for every string that crosses it. The older call takes one
//!   struct and one `windows-sys` module — `Win32_UI_Controls_Dialogs`, which is one
//!   feature flag on a dependency this crate already links for the window code, so no
//!   new crate enters the build and `THIRD_PARTY_NOTICES.md` does not change (the
//!   same reasoning `native.rs` gives for its two Win32 calls). The cost is named
//!   rather than discovered: this is the pre-Vista dialog, not the one Windows 11
//!   draws for its own apps.
//! * **On the frame thread.** A native dialog is modal to the window it is given,
//!   and `GetOpenFileNameW` runs its own message loop while it is open. A dialog
//!   opened from a worker thread would be modal to a window owned by another thread,
//!   which is not modality at all; so the caller opens it where the window lives —
//!   see `shell::Shell::add_skin`, which reads the file and prepares it there and
//!   sends only the *upload* off the thread.
//! * **A ceiling on the file.** A skin texture is a 64x64 PNG: a few kilobytes, and
//!   never a megabyte. A reader who picks last night's video by mistake should be
//!   told, in one sentence, before this launcher hands a launcher-side file to a
//!   service that would answer with a 413 that says nothing about which file.
//!
//! Everything but the dialog call itself — the filter, the buffer it fills, the
//! ceiling, the read — carries no `cfg` and is tested everywhere. The dialog is not
//! tested and cannot be: it is a modal OS window, and a test that opened one would
//! be waiting for a mouse.

use std::path::{Path, PathBuf};

/// The largest file this hands on, in bytes.
///
/// One mebibyte is four orders of magnitude above the largest texture the format has
/// (a 64x64 PNG is a few kilobytes, and the 128x128 skins some servers accept are
/// still tens), and small enough that the bytes can travel inside a message without
/// anything having to think about it.
pub const MAX_SKIN_BYTES: u64 = 1 << 20;

/// The file filter the dialog shows: one entry, and the double NUL that ends it.
///
/// `GetOpenFileNameW` reads a filter as pairs of NUL-terminated strings — a label and
/// a pattern — and stops at an empty one, so the two trailing NULs are the terminator
/// rather than padding. Written out as one constant so a test can read it: a filter
/// that forgot one of them is a dialog with no file types in it at all.
const FILTER: &str = "PNG images (*.png)\0*.png\0\0";

/// How many UTF-16 units the dialog may write a path into.
///
/// Four thousand is fifteen times the classic `MAX_PATH` of 260 and above the 32,767
/// a long path can be only in the sense that it is a buffer a file dialog can fill:
/// anything longer is a path Windows itself would refuse to hand over.
const MAX_PATH_UNITS: usize = 4096;

/// The filter as the UTF-16 the dialog takes.
fn filter() -> Vec<u16> {
    FILTER.encode_utf16().collect()
}

/// The path a filled dialog buffer holds.
///
/// `GetOpenFileNameW` answers by writing the chosen path into the buffer it was given,
/// NUL-terminated, and it writes an empty string when the reader cancels — which is
/// why the two are one function: "what did the dialog leave behind" has exactly these
/// two answers, and no `cfg` on it means both can be tested without opening a window.
pub fn picked_path(buffer: &[u16]) -> Option<PathBuf> {
    let end = buffer.iter().position(|unit| *unit == 0).unwrap_or(buffer.len());
    if end == 0 {
        return None;
    }
    Some(PathBuf::from(String::from_utf16_lossy(&buffer[..end])))
}

/// Read a picked file, refusing one that is too big to be a skin.
///
/// The size is asked for before the bytes are: a reader who picked a 4 GB file should
/// not have this launcher try. Whether the bytes *are* a skin is not this function's
/// question — that is `skin::prepare`'s, and it answers it by decoding them.
pub fn read(path: &Path) -> Result<Vec<u8>, String> {
    let size = std::fs::metadata(path)
        .map_err(|error| format!("That file could not be opened: {error}"))?
        .len();
    if size > MAX_SKIN_BYTES {
        return Err(format!(
            "That file is {} kB, and a skin texture is at most {} kB.",
            size / 1024,
            MAX_SKIN_BYTES / 1024
        ));
    }
    std::fs::read(path).map_err(|error| format!("That file could not be read: {error}"))
}

/// Ask for a skin file: `Ok(Some(path))` when one was chosen, `Ok(None)` when the
/// reader closed the dialog, and `Err` when this machine has no dialog to open.
///
/// The three answers are the caller's to tell apart — a cancel is not a failure, and
/// a page that drew one as a failure would be apologising for nothing — which is why
/// this is not an `Option`.
#[cfg(windows)]
pub fn choose(title: &str) -> Result<Option<PathBuf>, String> {
    use windows_sys::Win32::UI::Controls::Dialogs::{
        GetOpenFileNameW, OFN_FILEMUSTEXIST, OFN_NOCHANGEDIR, OFN_PATHMUSTEXIST, OPENFILENAMEW,
    };

    let mut chosen = vec![0u16; MAX_PATH_UNITS];
    let filter = filter();
    let title = crate::native::wide(title);
    let extension = crate::native::wide("png");
    // SAFETY: `OPENFILENAMEW` is a plain C struct of integers and pointers, and Win32
    // expects the caller to hand it one that has been zeroed and then filled in: every
    // field this does not set is meant to be null or zero.
    let mut dialog: OPENFILENAMEW = unsafe { std::mem::zeroed() };
    dialog.lStructSize = std::mem::size_of::<OPENFILENAMEW>() as u32;
    // Owned by this launcher's window, so the dialog is modal to it and cannot end up
    // behind it. A failed walk for the handle is not fatal: an unowned dialog still
    // opens, it just is not modal.
    dialog.hwndOwner = crate::native::own_hwnd().unwrap_or(0);
    dialog.lpstrFilter = filter.as_ptr();
    dialog.lpstrFile = chosen.as_mut_ptr();
    dialog.nMaxFile = chosen.len() as u32;
    dialog.lpstrTitle = title.as_ptr();
    // The suffix the dialog appends when the reader types a name without one, so
    // "myskin" is saved as "myskin.png" rather than refused.
    dialog.lpstrDefExt = extension.as_ptr();
    // A file that is not there is not a texture; a folder that is not there is not
    // where the reader meant to look. `OFN_NOCHANGEDIR` is for this launcher rather
    // than for the dialog: it stops the dialog from changing the *process's* current
    // directory, which a launcher that resolves relative paths must not have happen
    // behind its back.
    dialog.Flags = OFN_FILEMUSTEXIST | OFN_PATHMUSTEXIST | OFN_NOCHANGEDIR;
    // SAFETY: every pointer in `dialog` points at a local that outlives this call --
    // `chosen` is the buffer the dialog writes into, and the filter, the title and the
    // suffix are all vectors alive until the end of the function. The window handle is
    // this process's own or null.
    let accepted = unsafe { GetOpenFileNameW(&mut dialog) };
    if accepted == 0 {
        // Zero means no file: either the reader cancelled or Windows refused to open
        // the dialog at all. `CommDlgExtendedError` is the only thing that tells the
        // two apart, and it is deliberately not asked -- both leave this launcher with
        // nothing to do, and a sentence about a dialog that would not open is a
        // sentence shown to a reader who just pressed Cancel.
        return Ok(None);
    }
    Ok(picked_path(&chosen))
}

/// Ask for a skin file. No picker off Windows, and that is an answer rather than a
/// cancel.
#[cfg(not(windows))]
pub fn choose(_title: &str) -> Result<Option<PathBuf>, String> {
    Err("This build has no file picker: adding a skin from a file is Windows-only.".to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_filter_is_one_entry_and_a_double_nul() {
        // The shape `GetOpenFileNameW` reads: a label, a pattern, then an empty string
        // to stop. A missing terminator is a dialog with no types in it, which is a
        // reader staring at a list of "All files" they cannot use.
        assert_eq!(FILTER, "PNG images (*.png)\0*.png\0\0");
        let units = filter();
        assert_eq!(units[units.len() - 1], 0);
        assert_eq!(units[units.len() - 2], 0);
        assert_eq!(units.iter().filter(|unit| **unit == 0).count(), 3, "two inner NULs and the terminator");
        // And it is the UTF-16 of the same text, not a lossy copy of it.
        assert_eq!(String::from_utf16(&units).expect("valid UTF-16"), FILTER);
    }

    #[test]
    fn a_cancel_is_an_empty_buffer_and_a_choice_is_read_to_its_nul() {
        assert_eq!(picked_path(&[0u16; 8]), None, "the dialog leaves the first unit a NUL");
        assert_eq!(picked_path(&[]), None, "and an empty buffer is the same answer");
        let mut buffer = vec![0u16; MAX_PATH_UNITS];
        for (index, unit) in "C:\\skins\\alex.png".encode_utf16().enumerate() {
            buffer[index] = unit;
        }
        assert_eq!(
            picked_path(&buffer),
            Some(PathBuf::from("C:\\skins\\alex.png")),
            "read to the NUL and no further"
        );
        // Whatever the dialog wrote, the rest of the buffer is not part of the path --
        // which is what makes the read a read rather than a whole-buffer decode.
        buffer[19] = b'X' as u16;
        assert_eq!(picked_path(&buffer), Some(PathBuf::from("C:\\skins\\alex.png")));
    }

    #[test]
    fn a_file_that_is_readable_is_read_and_one_over_the_ceiling_is_refused_by_size() {
        let directory = tempfile::tempdir().expect("a scratch directory");
        let small = directory.path().join("alex.png");
        std::fs::write(&small, b"a few bytes").expect("a written file");
        assert_eq!(read(&small).expect("the bytes"), b"a few bytes");
        // The ceiling is asked of the file's size before its bytes are read, so the
        // sentence arrives without a 4 GB allocation behind it.
        let big = directory.path().join("huge.png");
        let file = std::fs::File::create(&big).expect("a file");
        file.set_len(MAX_SKIN_BYTES + 1).expect("a size without the bytes");
        drop(file);
        let reason = read(&big).expect_err("too big to be a skin");
        assert_eq!(reason, "That file is 1024 kB, and a skin texture is at most 1024 kB.");
        // And the two ways a read fails are sentences rather than panics: a path that
        // is not there, and a directory.
        let missing = directory.path().join("not-here.png");
        assert!(read(&missing).expect_err("no such file").starts_with("That file could not be opened:"));
        assert!(read(directory.path()).expect_err("a directory").starts_with("That file could not be read:"));
    }

    #[test]
    fn the_ceiling_is_the_one_the_message_names() {
        // The two numbers in that sentence are computed from the constant, so this is
        // the one place they can be wrong together.
        assert_eq!(MAX_SKIN_BYTES, 1_048_576);
        assert_eq!(MAX_SKIN_BYTES / 1024, 1024);
    }
}
