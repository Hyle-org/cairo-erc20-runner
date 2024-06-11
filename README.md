```
make deps
cargo run ./cairo-erc20/src/lib.cairo ./target/trace ./target/memory ./erc20.args
platinum-prover prove ./target/trace ./target/memory proof
platinum-prover verify proof
```