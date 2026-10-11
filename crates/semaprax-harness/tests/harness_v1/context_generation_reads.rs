use super::*;
use semaprax_harness::context::index_adoption::GenerationStore;
use std::sync::mpsc;
use std::time::Duration;

#[test]
fn hn10_selected_generation_remains_readable_until_read_finishes() {
    let root = fixture_dir("hp-hnf-held-read").canonicalize().unwrap();
    let store = GenerationStore::new(root.join("owned"));
    {
        let _lock = store.lock(Duration::from_secs(5)).unwrap();
        let (n, dir) = store.begin().unwrap();
        std::fs::write(dir.join("graph.json"), "generation-1").unwrap();
        store.publish(n).unwrap();
    }
    let (selected_tx, selected_rx) = mpsc::channel();
    let (release_tx, release_rx) = mpsc::channel();
    let reader_root = root.clone();
    let reader = std::thread::spawn(move || {
        GenerationStore::new(reader_root.join("owned"))
            .read_current(Duration::from_secs(5), |n, dir| {
                selected_tx.send(()).unwrap();
                release_rx.recv_timeout(Duration::from_secs(5)).unwrap();
                assert_eq!(n, 1);
                assert_eq!(
                    std::fs::read_to_string(dir.join("graph.json")).unwrap(),
                    "generation-1"
                );
                Ok(n)
            })
            .unwrap()
    });
    selected_rx.recv_timeout(Duration::from_secs(5)).unwrap();
    // Another store cannot enter publication while selected files remain in use.
    let err = store.lock(Duration::from_millis(100)).err().unwrap();
    assert_eq!(err.code, "SPX-HPF010");
    release_tx.send(()).unwrap();
    assert_eq!(reader.join().unwrap(), Some(1));
    for expected in 2..=3 {
        let _lock = store.lock(Duration::from_secs(5)).unwrap();
        let (n, dir) = store.begin().unwrap();
        assert_eq!(n, expected);
        std::fs::write(dir.join("graph.json"), format!("generation-{n}")).unwrap();
        store.publish(n).unwrap();
    }
    assert!(!root.join("owned/gen/g1").exists());
    assert_eq!(
        std::fs::read_dir(root.join("owned/gen")).unwrap().count(),
        2
    );
    // Callback failure releases the lock without changing CURRENT.
    let err = store
        .read_current::<()>(Duration::from_secs(5), |_, _| {
            Err(HarnessDiagnostic::new("SPX-HPF011", "read failed"))
        })
        .unwrap_err();
    assert_eq!(err.code, "SPX-HPF011");
    let _lock = store.lock(Duration::from_secs(5)).unwrap();
    assert_eq!(store.current().unwrap().0, 3);
}
