use std::{time::SystemTime, vec};

use rand::seq::SliceRandom;
use rand::{random, thread_rng};
use time::OffsetDateTime;

#[test]
fn rands() {
    let mut x = 1.;
    for i in 0..20 {
        let r = random::<f64>();
        println!("{}: {}, {x}", i, r);
        // if random::<f64>() < 0.4 {
        //     x *= 48./50.;
        // } else {
        //     x *= 50./48.;
        // }
        x *= 5. / 4.;
    }
    println!("{}", x);
}

#[test]
fn t() {
    let mut x = vec![1.; 3];
    let mut rng = thread_rng();
    x.append(&mut vec![-1.; 2]);
    x.shuffle(&mut rng);
    dbg!(&x);
    for ratio in 1..100 {
        let mut cap_log = 1.;
        let ratio = ratio as f64 / 100.;
        for r in x.iter() {
            cap_log *= 1. + ratio * r;
        }
        dbg!(ratio, cap_log);
    }
}
#[test]
fn t2() {
    let mut x = 0.;
    for r in 0..100 {
        let r = r as f64 / 100.;
        x = (1. + r).powi(3) * (1. - r).powi(2);
        dbg!(x);
    }
}
