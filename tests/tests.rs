use std::{
    any::Any, sync::{atomic::AtomicI32, Arc}, thread::sleep
};


use dashmap::DashMap;
use rayon::prelude::*;
#[test]
fn playground() {
    let atom = AtomicI32::new(0);
    // [0,1,2,3,4].into_par_iter().for_each(|i|{
    //     println!("{}: {}", i, atom.load(std::sync::atomic::Ordering::Relaxed));
    //     atom.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    //     sleep(std::time::Duration::from_secs(5));
    //     println!("{}: {}", i, atom.load(std::sync::atomic::Ordering::Relaxed));
    // });
    let x: DashMap<String, String> = DashMap::new();
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
