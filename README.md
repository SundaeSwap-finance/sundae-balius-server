# sundae-strategies

This crate provides several helpers and utilties to reduce boilerplate when writing [Sundae Strategies](https://github.com/SundaeSwap-finance/sundae-strategies) to run in the [Balius](https://github.com/txpipe/balius) runtime environment.

## Getting started

Make sure you have [Cargo](https://www.rust-lang.org/tools/install) and [Bun](https://bun.com/docs/installation).

You can create a new strategy of your own by using `cargo-generate`:

```sh
# If you don't have it installed already
# cargo install cargo-generate
cargo generate SundaeSwap-finance/sundae-strategy-template
```

This will set up a new directory with a pre-implemented trailing-stop loss strategy for you.

From there, you can compile the worker with

```sh
# Install these if you haven't already
# cargo install just
# cargo install --git https://github.com/SundaeSwap-finance/sundae-strategies balius-worker-builder
just build
```

and run it with

```sh
baliusd
```

When you're working on your strategies, you can use the Sundae SDK CLI to place a strategy order:

```sh
bunx @sundaeswap/cli
```

The best workflow is to run the worker once to initialize it's state, then stop baliusd. You can use

```sh
baliusd show-keys default
```

to show the public key, and then place the strategy order.

From there, you can run

```sh
baliusd --debug
```

While running in debug mode, it won't persist any state. Meaning if you stop baliusd and run it in debug mode again, it will replay all the same events, letting you iterate on your strategy as you get the logic right.

Please let us know if you have feedback on this development flow, we and the TxPipe team are always looking for opportunities to further streamline it!
