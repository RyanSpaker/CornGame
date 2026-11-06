//! CLAUDE (claude-opus-5-5): 2026-11-02 where script time goes, with the `profile` feature: time spent
//! in host calls by kind (the rust side of a script's work), and in `drive_scripts`. Off, [`time`] does
//! nothing and [`take`] is empty.
//!
//! ```ignore
//! let before = corn_scripting::profile::take(); // and later, the same: what accumulated since
//! ```

use std::time::Duration;

/// time spent in one kind of host call, and how many
#[derive(Default, Clone, Copy, Debug)]
pub struct Spent {
    pub calls: u64,
    pub time: Duration,
}

#[cfg(feature = "profile")]
mod on {
    use std::{cell::RefCell, collections::BTreeMap, sync::Mutex, time::Instant};

    use super::Spent;

    /// all threads': bevy may run drive_scripts on one thread and the system reading this on another
    static SPENT: Mutex<BTreeMap<&'static str, Spent>> = Mutex::new(BTreeMap::new());

    thread_local! {
        /// host calls nest (a spawn inside a call inside drive_scripts): only the outermost of a kind counts
        static OPEN: RefCell<Vec<&'static str>> = RefCell::default();
    }

    pub struct Timer(&'static str, Instant, bool);

    pub fn time(kind: &'static str) -> Timer {
        let nested = OPEN.with_borrow_mut(|open| {
            let nested = open.contains(&kind);
            open.push(kind);
            nested
        });
        Timer(kind, Instant::now(), nested)
    }

    impl Drop for Timer {
        fn drop(&mut self) {
            let elapsed = self.1.elapsed();
            OPEN.with_borrow_mut(|open| open.pop());
            if !self.2 {
                let mut spent = SPENT.lock().unwrap();
                let s = spent.entry(self.0).or_default();
                s.calls += 1;
                s.time += elapsed;
            }
        }
    }

    pub fn take() -> Vec<(&'static str, Spent)> {
        std::mem::take(&mut *SPENT.lock().unwrap()).into_iter().collect()
    }
}

#[cfg(feature = "profile")]
pub use on::{take, time};

#[cfg(not(feature = "profile"))]
pub fn time(_: &'static str) {}

#[cfg(not(feature = "profile"))]
pub fn take() -> Vec<(&'static str, Spent)> {
    vec![]
}

/// what [`take`] gave, as one line: `kind calls time, ...`
pub fn describe(spent: &[(&'static str, Spent)]) -> String {
    spent
        .iter()
        .map(|(kind, s)| format!("{kind} {}x {:.2}ms", s.calls, s.time.as_secs_f64() * 1e3))
        .collect::<Vec<_>>()
        .join(", ")
}
