#![no_std]
#![no_main]

use user::{println, test_main};

const LEN: usize = 100;

#[no_mangle]
fn main() -> i32 {
    test_main("test_power", || {
        let p = 7u64;
        let m = 998244353u64;
        let iter: usize = 160000;
        let mut s = [0u64; LEN];
        let mut cur = 0usize;
        s[cur] = 1;
        for i in 1..=iter {
            let next = if cur + 1 == LEN { 0 } else { cur + 1 };
            s[next] = s[cur] * p % m;
            cur = next;
            if i % 10000 == 0 {
                println!("power [{}/{}]", i, iter);
            }
        }
        assert_eq!(s[cur], 667897727);
        println!("{}^{} % {} = {}", p, iter, m, s[cur]);
    });
    0
}
