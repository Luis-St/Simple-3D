//! A file dialog runs without holding up the frame.

use super::*;
use std::path::{Path, PathBuf};

#[test]
pub(crate) fn a_file_dialog_does_not_hold_up_the_frame_and_never_answers_silently() {
    // The dialog used to be called straight from `App::update` and waited on
    // the calling thread, so a portal that was slow, absent or confused took
    // the whole application down with it -- the last frame stayed on screen
    // with its hover states frozen, and the process had to be killed. It
    // waits on a thread now and is picked up by `poll_file_prompt`.
    //
    // Driven through a channel of this test's own rather than a real
    // dialog: what is under test is that the answer arrives on a later
    // frame and that no outcome is silent.
    let mut app = app_in(temp_config_dir("file-prompt"));
    let (tx, rx) = std::sync::mpsc::channel();
    let chosen = std::rc::Rc::new(std::cell::RefCell::new(None));
    let sink = chosen.clone();
    app.file_prompt = Some(FilePrompt {
        what: "Save as",
        answer: rx,
        then: Box::new(move |_app, path| *sink.borrow_mut() = Some(path)),
        started: std::time::Instant::now(),
    });

    // Nothing yet, and nothing blocking: the frame goes on without an answer.
    app.poll_file_prompt();
    assert!(app.file_prompt.is_some(), "the prompt was given up on before it answered");
    assert!(chosen.borrow().is_none());

    tx.send(Some(PathBuf::from("/tmp/simple3d-test.simple3d"))).expect("the prompt is listening");
    app.poll_file_prompt();
    assert!(app.file_prompt.is_none(), "the answered prompt was not cleared");
    assert_eq!(chosen.borrow().as_deref(), Some(Path::new("/tmp/simple3d-test.simple3d")));

    // A dialog that answers with nothing says so. Silence there was the
    // other half of the bug: Ctrl+S on an unsaved document produced no
    // dialog and no message, which is indistinguishable from a cancel.
    let (tx, rx) = std::sync::mpsc::channel();
    app.file_prompt = Some(FilePrompt {
        what: "Save as",
        answer: rx,
        then: Box::new(|_app, _path| panic!("a cancelled dialog must not act")),
        started: std::time::Instant::now(),
    });
    tx.send(None).expect("the prompt is listening");
    app.poll_file_prompt();
    assert!(app.file_prompt.is_none());
    assert!(app.status_text().contains("cancelled"), "a cancelled dialog said nothing: {}", app.status_text());
}
