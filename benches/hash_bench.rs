use std::{
    hash::Hasher,
    net::{Ipv4Addr, Ipv6Addr, SocketAddr},
};

use ahash::AHasher;
use criterion::{criterion_group, criterion_main, Criterion};
use rand::{
    distr::{Distribution, Uniform},
    rngs::ThreadRng,
};

use bevy_quic_networking::common::IpAddrBytes;

const BENCH_SIZE: usize = 1000;

pub fn criterion_benchmark(c: &mut Criterion) {
    let mut group = c.benchmark_group("ahash");

    let mut rng = rand::rng();
    let addrs_ipv4: [SocketAddr; BENCH_SIZE] = std::array::from_fn(|_| random_ipv4(&mut rng));
    let addrs_ipv6: [SocketAddr; BENCH_SIZE] = std::array::from_fn(|_| random_ipv6(&mut rng));

    group.bench_function("ipv4-ahash", |b| b.iter(|| ipvx_ahash(&addrs_ipv4)));
    group.bench_function("ipv6-ahash", |b| b.iter(|| ipvx_ahash(&addrs_ipv6)));

    group.bench_function("ipv4-str-ahash", |b| b.iter(|| ipvx_str_ahash(&addrs_ipv4)));
    group.bench_function("ipv6-str-ahash", |b| b.iter(|| ipvx_str_ahash(&addrs_ipv6)));
}

fn ipvx_ahash(addrs: &[SocketAddr; BENCH_SIZE]) -> [u64; BENCH_SIZE] {
    std::array::from_fn(|i| {
        let mut hasher = AHasher::default();
        let bytes: IpAddrBytes = addrs[i].ip().into();
        hasher.write(bytes.to_vec().as_slice());
        hasher.finish()
    })
}

fn ipvx_str_ahash(addrs: &[SocketAddr; BENCH_SIZE]) -> [u64; BENCH_SIZE] {
    std::array::from_fn(|i| {
        let mut hasher = AHasher::default();
        hasher.write(addrs[i].to_string().as_bytes());
        hasher.finish()
    })
}

fn random_ipv4(rng: &mut ThreadRng) -> SocketAddr {
    let ip_step: Uniform<u8> =
        Uniform::new(0, u8::MAX).expect("Could not create uniform distribution");
    let port_step: Uniform<u16> =
        Uniform::new(0, u16::MAX).expect("Could not create uniform distribution");

    let ipv4 = Ipv4Addr::new(
        ip_step.sample(rng),
        ip_step.sample(rng),
        ip_step.sample(rng),
        ip_step.sample(rng),
    );

    SocketAddr::new(std::net::IpAddr::V4(ipv4), port_step.sample(rng))
}

fn random_ipv6(rng: &mut ThreadRng) -> SocketAddr {
    let ip_step: Uniform<u16> =
        Uniform::new(0, u16::MAX).expect("Could not create uniform distribution");
    let port_step: Uniform<u16> =
        Uniform::new(0, u16::MAX).expect("Could not create uniform distribution");

    let ipv6 = Ipv6Addr::new(
        ip_step.sample(rng),
        ip_step.sample(rng),
        ip_step.sample(rng),
        ip_step.sample(rng),
        ip_step.sample(rng),
        ip_step.sample(rng),
        ip_step.sample(rng),
        ip_step.sample(rng),
    );

    SocketAddr::new(std::net::IpAddr::V6(ipv6), port_step.sample(rng))
}

criterion_group!(benches, criterion_benchmark);
criterion_main!(benches);
