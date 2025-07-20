build:
    RUSTUP_TOOLCHAIN=nightly-2025-04-01 anchor idl build -o target/idl/bifido_swap.json -t target/types/bifido_swap.ts && cp target/idl/bifido_swap.json ../swap-sdk/src/raydium/clmm/idl.json && cp target/types/bifido_swap.ts ../swap-sdk/src/raydium/clmm/bifido_swap.ts && anchor build --no-idl

set-version:
    avm use 0.29.0
    agave-install init 1.16.27
