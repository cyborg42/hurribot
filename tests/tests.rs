use rand::random;

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
