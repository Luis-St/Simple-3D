//! A file dialog runs without holding up the frame.

use super::*;
use std::path::{Path, PathBuf};

#[test]
pub(crate) fn a_file_dialog_does_not_hold_up_the_frame_and_never_answers_silently() {
    // Regression: the dialog blocked `App::update`, so a slow or absent portal froze the app. It waits on
    // a thread now, picked up by `poll_file_prompt`; a test channel stands in for a real dialog.
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

    // Nothing yet, and nothing blocking.
    app.poll_file_prompt();
    assert!(app.file_prompt.is_some(), "the prompt was given up on before it answered");
    assert!(chosen.borrow().is_none());

    tx.send(Some(PathBuf::from("/tmp/simple3d-test.simple3d"))).expect("the prompt is listening");
    app.poll_file_prompt();
    assert!(app.file_prompt.is_none(), "the answered prompt was not cleared");
    assert_eq!(chosen.borrow().as_deref(), Some(Path::new("/tmp/simple3d-test.simple3d")));

    // An empty answer says so: Ctrl+S on an unsaved document used to be silent, like a cancel.
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
