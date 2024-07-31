use std::{
    any::Any,
    sync::{atomic::AtomicI32, Arc},
    thread::sleep,
};

use dashmap::DashMap;
use rayon::prelude::*;
#[test]
fn playground() {
    let x = [10; 10];
    let mut i = 10;
    i += fastrand::usize(0..10);
    println!("{:?}", x[i]);
}

#[test]
fn t() {
    let mut c = 100.;
    let step = 0.0001;
    let mut price = 1.;
    let leverage = 4.;
    while price < 1.5 {
        c *= step / price * leverage + 1.;
        price += step;
    }
    println!("{}%", c);
}
