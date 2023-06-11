use std::time::{SystemTime, UNIX_EPOCH};
use std::marker::{Send, Sync};

pub trait Clock: Send + Sync {
    fn now(&self) -> u128;
    fn since(&self, ts: u128) -> u128 {
        let now = self.now();
        return now - ts;
    }
}

#[derive(Clone, Copy)]
pub struct SystemClock {}

impl Clock for SystemClock {
    fn now(&self) -> u128 {
        let start = SystemTime::now();
        let since_the_epoch = start
            .duration_since(UNIX_EPOCH)
            .expect("Time went backwards");
        return since_the_epoch.as_millis();    
    }
}