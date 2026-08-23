/******************************************************************************
   Author: Joaquín Béjar García
   Email: jb@taunais.com
   Date: 4/9/24
******************************************************************************/

//! Criterion benchmarks for the hot paths: rate-limiter pacing, request
//! construction and DTO deserialization.

use criterion::criterion_main;

mod bench;

criterion_main! {
    bench::benches,
}
