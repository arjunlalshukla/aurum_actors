#![allow(unused_imports, dead_code, unused_variables)]

use itertools::Itertools;
use std::fmt::Debug;
use std::{
  collections::VecDeque,
  time::{Duration, Instant},
};
use std::{iter::repeat, ops::Add};

pub struct IntervalStorage {
  capacity: usize,
  intervals: VecDeque<u64>,
  sum: u64,
  sum_squares: u64,
  latest: Instant,
}
impl IntervalStorage {
  pub fn new(
    cap: usize,
    init: Duration,
    times: usize,
    start: Option<Instant>,
  ) -> IntervalStorage {
    let intervals = repeat(dur2u64(&init))
      .take(times)
      .interleave(repeat(0).take(times))
      .take(cap)
      .collect::<VecDeque<_>>();
    let sum = intervals.iter().sum();
    let sum_squares = intervals.iter().map(|d| d * d).sum();
    IntervalStorage {
      capacity: cap,
      intervals: intervals,
      sum: sum,
      sum_squares: sum_squares,
      latest: start.unwrap_or(Instant::now()),
    }
  }

  pub fn push(&mut self) {
    self.push_instant(Instant::now())
  }

  fn push_instant(&mut self, i: Instant) {
    while self.intervals.len() >= self.capacity {
      let last = self.intervals.pop_back().unwrap();
      self.sum -= last;
      self.sum_squares -= last * last;
    }
    let first = dur2u64(&i.duration_since(self.latest));
    self.intervals.push_front(first);
    self.sum += first;
    self.sum_squares += first * first;
    self.latest = i;
  }

  pub fn mean(&self) -> f64 {
    self.sum as f64 / self.intervals.len() as f64
  }

  pub fn stdev(&self) -> f64 {
    let mean = self.mean();
    (self.sum_squares as f64 / self.intervals.len() as f64 - mean * mean).sqrt()
  }

  pub fn phi(&self) -> f64 {
    self.phi_duration(self.latest.elapsed())
  }

  fn phi_duration(&self, dur: Duration) -> f64 {
    let diff = dur2u64(&dur) as f64;
    let mean = self.mean();
    let y = (diff - mean) / self.stdev();
    let h = (-y * (1.5976 + 0.070566 * y * y)).exp();
    if diff > mean {
      -(h / (1.0 + h)).log10()
    } else {
      -(1.0 - 1.0 / (1.0 + h)).log10()
    }
  }
}
impl Debug for IntervalStorage {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    f.debug_struct("IntervalStorage")
      .field("mean", &self.mean())
      .field("stddev", &self.stdev())
      .finish()
  }
}

fn dur2u64(dur: &Duration) -> u64 {
  dur.as_millis() as u64
}

#[test]
fn test_interval_storage() {
  let mut start = Instant::now();
  let mut test =
    IntervalStorage::new(10, Duration::from_millis(50), 2, Some(start.clone()));
  println!("intervals: {:?}", test.intervals);
  let insertions = (45..=100).map(Duration::from_millis).collect::<Vec<_>>();
  for dur in &insertions[..5] {
    start = start.add(*dur);
    test.push_instant(start.clone());
  }
  println!("intervals: {:?}", test.intervals);
  println!("test: {:?}", test);
  println!("phi: {:?}", test.phi_duration(Duration::from_millis(50)));
  println!("phi: {:?}", test.phi_duration(Duration::from_millis(100)));
  println!("phi: {:?}", test.phi_duration(Duration::from_millis(300)));

  for dur in &insertions[5..] {
    start = start.add(*dur);
    test.push_instant(start.clone());
  }
  println!("intervals: {:?}", test.intervals);
  println!("test: {:?}", test);
  println!("phi: {:?}", test.phi_duration(Duration::from_millis(50)));
  println!("phi: {:?}", test.phi_duration(Duration::from_millis(100)));
  println!("phi: {:?}", test.phi_duration(Duration::from_millis(95)));
}
