set positional-arguments

default:
  @just --list

check:
  cargo check

test:
  cargo test

run *args='':
  cargo run -- {{args}}

demo:
  cargo run --features harness -- --demo

harness *args='':
  cargo run --features harness -- --harness-dump {{args}}
