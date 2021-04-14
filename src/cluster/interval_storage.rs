#![allow(unused_imports, dead_code, unused_variables)]

use itertools::Itertools;
use statrs::distribution::{InverseCDF, Normal, Univariate};
use std::{f64::NEG_INFINITY, fmt::Debug};
use std::{
  collections::VecDeque,
  time::{Duration, Instant},
};
use std::{iter::repeat, ops::Add};
use rug::Float;
 
const PRECISION: u32 = 100;

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
    /*
    let diff = dur2u64(&dur) as f64;
    let mean = self.mean();
    let y = (diff - mean) / self.stdev();
    let h = (-y * (1.5976 + 0.070566 * y * y)).exp();
    if diff > mean {
      -(h / (1.0 + h)).log10()
    } else {
      -(1.0 - 1.0 / (1.0 + h)).log10()
    }
    */
    Normal::new(self.mean(), self.stdev()).unwrap().cdf(dur.as_secs_f64() * 1000.0)
  }

  fn duration_phi(&self, phi: f64) -> Duration {
    /*
    let lnh = (1.0/(1.0-10.0_f64.powf(-phi))-1.0).ln();
    println!("lnh: {}", lnh);
    let p = 0.1127362416;
    let q = 14.1711305728*lnh;
    println!("q: {}", q);
    // Cardano's formula
    let y = (-q/2.0 + (q*q/4.0 + p*p*p/27.0).sqrt()).cbrt()
              + (-q/2.0 - (q*q/4.0 + p*p*p/27.0).sqrt()).cbrt();
    let diff = y*self.stdev() + self.mean();
    println!("diff: {}", diff);
    // Divide by thousand because we've been using milliseconds
    if diff.is_finite() {
      Duration::from_secs_f64(diff / 1000.0)
    } else {
      Duration::new(0, 0)
    }
    */
    let millis = Normal::new(self.mean(), self.stdev()).unwrap().inverse_cdf(phi);
    if millis.is_finite() {
      Duration::from_secs_f64(millis / 1000.0)
    } else {
      Duration::new(0, 0)
    }
  }

  pub fn mean_f(&self) -> Float {
    Float::with_val(PRECISION, self.sum) /
    Float::with_val(PRECISION, self.intervals.len())
  }
  
  pub fn stdev_f(&self) -> Float {
    let mean = self.mean();
    (
      Float::with_val(PRECISION, self.sum_squares)
      / Float::with_val(PRECISION, self.intervals.len())
      - self.mean()
    )
    .sqrt()
  }
  
  //pub fn phi(&self) -> f64 {
  //  self.phi_duration(self.latest.elapsed())
  //}
  
  fn phi_duration_f(&self, dur: Duration) -> Float {
    let diff = Float::with_val(PRECISION, dur2u64(&dur));
    let mean = self.mean();
    let y = (diff - mean) / self.stdev();
    let h = (-y.clone() * (1.5976_f64 + 0.070566_f64 * y.clone() * y)).exp();
    -(1f64 - 1f64 / (1f64 + h)).log10()
  }
  
  fn duration_phi_f(&self, phi: Float) -> Duration {
    let lnh: Float = ((-(-phi).exp10() + 1f64).recip()-1f64).ln();
    println!("lnh-f: {}", lnh);
    let p = || Float::with_val(PRECISION, 0.1127362416);
    let q: Box<dyn Fn() -> Float> = Box::new(move || 14.1711305728*lnh.clone());
    println!("q-f: {}", q());
    // Cardano's formula
    let y: Float = (-q()/2f64 + (q().square()/4f64 + p().square()*p()/27f64).sqrt()).cbrt()
                 + (-q()/2f64 - (q().square()/4f64 + p().square()*p()/27f64).sqrt()).cbrt();
    let diff = y*self.stdev() + self.mean();
    println!("diff-f: {}", diff);
    // Divide by thousand because we've been using milliseconds
    if diff.is_finite() {
      Duration::from_secs_f64(diff.to_f64() / 1000f64)
    } else {
      Duration::new(0, 0)
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
    IntervalStorage::new(10, Duration::from_millis(10000), 2, Some(start.clone()));
  println!("intervals: {:?}", test.intervals);
  let insertions = (4500..=5500).step_by(100).map(Duration::from_millis).collect::<Vec<_>>();
  for dur in &insertions[..5] {
    start = start.add(*dur);
    test.push_instant(start.clone());
  }
  //println!("intervals: {:?}", test.intervals);
  //println!("test: {:?}", test);
  //println!("phi: {:?}", test.phi_duration(Duration::from_millis(50)));
  //println!("phi: {:?}", test.phi_duration(Duration::from_millis(100)));
  //println!("phi: {:?}", test.phi_duration(Duration::from_millis(300)));

  for dur in &insertions[5..] {
    start = start.add(*dur);
    test.push_instant(start.clone());
  }
  println!("intervals: {:?}", test.intervals);
  println!("test: {:?}", test);
  //println!("phi: {:?}", test.phi_duration(Duration::from_millis(50)));
  //println!("phi: {:?}", test.phi_duration(Duration::from_millis(100)));
  //println!("phi: {:?}", test.phi_duration(Duration::from_millis(95)));
  let millis = 5150;
  let phi = test.phi_duration(Duration::from_millis(millis));
  println!("phi: {:?}", phi);
  let dur = test.duration_phi(phi);
  println!("Expected: {}, Duration {}", millis, dur.as_millis());

  let millis = 5000;
  let phi = test.phi_duration_f(Duration::from_millis(millis));
  println!("phi-f: {:?}", phi);
  let dur = test.duration_phi_f(phi);
  println!("Expected: {}, Duration-f {}", millis, dur.as_millis());
}
