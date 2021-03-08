#![allow(unused_imports, dead_code, unused_variables)]

use std::iter::repeat;
use std::{
  collections::VecDeque,
  time::{Duration, Instant},
};

struct IntervalStorage {
  capacity: usize,
  intervals: VecDeque<u64>,
  sum: u64,
  sum_squares: u64,
  latest: Instant,
}
impl IntervalStorage {
  pub fn new(cap: usize, init: Duration, times: usize) -> IntervalStorage {
    let intervals = repeat(init.as_millis() as u64)
      .take(times)
      .chain(repeat(0).take(times))
      .collect::<VecDeque<_>>();
    let sum = intervals.iter().sum();
    let sum_squares = intervals.iter().map(|d| d * d).sum();
    IntervalStorage {
      capacity: cap,
      intervals: intervals,
      sum: sum,
      sum_squares: sum_squares,
      latest: Instant::now(),
    }
  }

  pub fn push(&mut self) {
    while self.intervals.len() >= self.capacity {
      let last = self.intervals.pop_back().unwrap();
      self.sum -= last;
      self.sum_squares -= last * last;
    }
    let new_latest = Instant::now();
    let first = new_latest.duration_since(self.latest).as_millis() as u64;
    self.intervals.push_front(first);
    self.sum += first;
    self.sum_squares += first * first;
    self.latest = new_latest;
  }

  pub fn mean(&self) -> f64 {
    self.sum as f64 / self.intervals.len() as f64
  }

  pub fn stdev(&self) -> f64 {
    let mean = self.mean();
    (self.sum_squares as f64 / self.intervals.len() as f64 - mean * mean).sqrt()
  }

  pub fn phi(&self) -> f64 {
    let diff = self.latest.elapsed().as_millis() as f64;
    let mean = self.mean();
    let y = (diff - mean) / self.stdev();
    let e = (-y * (1.5976 + 0.070566 * y * y)).exp();
    if diff > mean {
      (e / (1.0 + e)).log10()
    } else {
      (1.0 - 1.0 / (1.0 + e)).log10()
    }
  }
}
