//! The crash note: what the panic hook writes beside the save. The hook is the process's, so it is set only for
//! this test's own panics: it writes a note unless the panic is caught, and no other test here panics
//! outside `catch_panic`.

use std::path::PathBuf;
use std::rc::Rc;

use sts2_core::Character;
use sts2_tui::app::App;
use sts2_tui::session::{self, Dirs, Session};
use sts2_tui::text::Localization;

#[test]
fn a_panic_leaves_a_note_beside_the_save_unless_it_is_caught() {
    let dir = PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join("crash");
    let _ = std::fs::remove_dir_all(&dir);
    let dirs = Dirs::at(&dir);
    let app = App::with_text(
        Character::Silent,
        "CRASH",
        3,
        Rc::new(Localization::empty()),
    )
    .expect("run");
    Session::new(dirs.clone()).autosave(&app).expect("save");
    let hook_dirs = dirs.clone();
    std::panic::set_hook(Box::new(move |info| {
        if !session::panic_is_caught() {
            session::write_crash_note(&hook_dirs, info);
        }
    }));
    // A panic the game catches (playing a save back) leaves no note.
    assert!(session::catch_panic(|| panic!("caught")).is_err());
    assert!(!dirs.crash().exists());
    let result = std::panic::catch_unwind(|| panic!("the engine broke"));
    let _ = std::panic::take_hook();
    assert!(result.is_err());
    let note = std::fs::read_to_string(dirs.crash()).expect("a crash note");
    assert!(note.contains("the engine broke"), "{note}");
    assert!(note.contains("crash.rs"), "where: {note}");
    assert!(
        note.contains("the run is saved after 0 actions: Silent  A3"),
        "{note}"
    );
    // The next launch shows it once.
    let session = Session::new(dirs.clone());
    assert_eq!(session.take_crash_note(), Some(dirs.old_crash()));
    assert_eq!(session.take_crash_note(), None);
}
