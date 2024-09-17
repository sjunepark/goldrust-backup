set dotenv-required
set dotenv-filename := ".env.example"

# Watch

watch:
     cargo watch -q -c -x "c"

watch-test name="":
    cargo watch -q -c -s "just test {{name}}"

watch-example name="":
    cargo watch -q -c -x "run --example {{name}}"

watch-test-integration:
    cargo watch -q -c -x 'test --test "*" -- --nocapture'


# Individual commands

test name="":
    cargo test --all-targets {{name}} -- --nocapture

example name="":
    cargo run --example {{name}}

doc:
    cargo doc --no-deps --open