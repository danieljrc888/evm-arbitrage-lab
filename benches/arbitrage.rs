use criterion::{black_box, criterion_group, criterion_main, Criterion};

use alloy_primitives::{address, Address};
use helix::models::{Exchange, Pool, Swap, SwapPath};
use helix::pricing::{Dex, TernarySearchOptimizer, UniswapV2};
use helix::strategy::{ArbitrageDetector, MathSimulator};

fn weth() -> Address {
    address!("C02aaA39b223FE8D0A0e5C4F27eAD9083C756Cc2")
}

fn dai() -> Address {
    address!("6B175474E89094C44Da98b954EedeAC495271d0F")
}

fn usdc() -> Address {
    address!("A0b86991c6218b36c1d19D4a2e9Eb0cE3606eB48")
}

fn sample_pools() -> Vec<Pool> {
    vec![
        Pool::new_v2(
            address!("A478c2975Ab1Ea89e8196811F51A7B7Ade33eB11"),
            weth(),
            dai(),
            Exchange::Uniswap,
            5_000 * 10u128.pow(18),
            10_000_000 * 10u128.pow(18),
        ),
        Pool::new_v2(
            address!("C3D03e4F041Fd4cD388c549Ee2A29a9E5075882f"),
            weth(),
            dai(),
            Exchange::Sushiswap,
            4_800 * 10u128.pow(18),
            9_800_000 * 10u128.pow(18),
        ),
        Pool::new_v2(
            address!("AE461cA67B15dc8dc81CE7615e0320dA1A9aB8D5"),
            dai(),
            usdc(),
            Exchange::Uniswap,
            50_000_000 * 10u128.pow(18),
            50_000_000 * 10u128.pow(6),
        ),
        Pool::new_v2(
            address!("B4e16d0168e52d35CaCD2c6185b44281Ec28C9Dc"),
            usdc(),
            weth(),
            Exchange::Uniswap,
            9_500_000 * 10u128.pow(6),
            5_000 * 10u128.pow(18),
        ),
    ]
}

fn sample_paths(pools: &[Pool]) -> Vec<SwapPath> {
    vec![
        SwapPath::new(
            "tri-weth-dai-usdc-weth",
            vec![
                Swap {
                    pool_address: pools[0].address,
                    token_in: weth(),
                    token_out: dai(),
                },
                Swap {
                    pool_address: pools[2].address,
                    token_in: dai(),
                    token_out: usdc(),
                },
                Swap {
                    pool_address: pools[3].address,
                    token_in: usdc(),
                    token_out: weth(),
                },
            ],
        )
        .unwrap(),
        SwapPath::new(
            "tri-weth-dai-sushi-usdc-weth",
            vec![
                Swap {
                    pool_address: pools[1].address,
                    token_in: weth(),
                    token_out: dai(),
                },
                Swap {
                    pool_address: pools[2].address,
                    token_in: dai(),
                    token_out: usdc(),
                },
                Swap {
                    pool_address: pools[3].address,
                    token_in: usdc(),
                    token_out: weth(),
                },
            ],
        )
        .unwrap(),
    ]
}

fn bench_ternary_search(c: &mut Criterion) {
    let optimizer = TernarySearchOptimizer::default();
    let eval = |x: u128| -> Option<u128> {
        let diff = if x > 500 { x - 500 } else { 500 - x };
        let profit = 250_000u128.saturating_sub(diff * diff);
        Some(x + profit)
    };

    c.bench_function("ternary_search", |b| {
        b.iter(|| optimizer.find_optimal(black_box(0), black_box(1000), &eval))
    });
}

fn bench_uniswap_v2_swap(c: &mut Criterion) {
    let pool = Pool::new_v2(
        address!("B4e16d0168e52d35CaCD2c6185b44281Ec28C9Dc"),
        weth(),
        usdc(),
        Exchange::Uniswap,
        50_000 * 10u128.pow(18),
        100_000_000 * 10u128.pow(6),
    );
    let amount_in = 10u128.pow(18); // 1 WETH

    c.bench_function("uniswap_v2_get_amount_out", |b| {
        b.iter(|| UniswapV2.get_amount_out(black_box(amount_in), black_box(weth()), &pool))
    });
}

fn bench_full_scan(c: &mut Criterion) {
    let pools = sample_pools();
    let paths = sample_paths(&pools);

    c.bench_function("full_scan_4_pools_2_paths", |b| {
        b.iter(|| {
            let simulator = MathSimulator::new(pools.clone());
            let detector = ArbitrageDetector::new(simulator, 30);
            detector.scan(black_box(&paths), 0)
        })
    });
}

criterion_group!(
    benches,
    bench_ternary_search,
    bench_uniswap_v2_swap,
    bench_full_scan
);
criterion_main!(benches);
