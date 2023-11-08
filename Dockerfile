# Get started with a build env with Rust nightly
FROM rustlang/rust:nightly-bullseye as builder


RUN cargo install wasm-bindgen-cli --version 0.2.88

# Install cargo-leptos
RUN cargo install cargo-leptos --version 0.2.0



# Add the WASM target
RUN rustup target add wasm32-unknown-unknown


# Make an /app dir, which everything will eventually live in
RUN mkdir -p /app
WORKDIR /app
COPY . .

# Build the app
WORKDIR /app/web-leptos
RUN cargo leptos build --release -vv

WORKDIR /app
FROM rustlang/rust:nightly-bullseye as runner
# Copy the server binary to the /app directory
COPY --from=builder /app/target/release/web-leptos /app/
# /target/site contains our JS/WASM/CSS, etc.
COPY --from=builder /app/target/site /app/site
# Copy Cargo.toml if it’s needed at runtime
COPY --from=builder /app/Cargo.toml /app/

WORKDIR /app

# Set any required env variables and
ENV RUST_LOG="info"
ENV APP_ENVIRONMENT="production"
ENV LEPTOS_SITE_ADDR="0.0.0.0:8080"
ENV LEPTOS_SITE_ROOT="site"
EXPOSE 8080
# Run the server
CMD ["/app/web-leptos"]

