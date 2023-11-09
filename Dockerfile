FROM anvlkv42/rust-nightly-wasm-leptos:latest as builder

# Make an /app dir, which everything will eventually live in
RUN mkdir -p /app
WORKDIR /app
COPY . .

# Build the app
WORKDIR /app/web-leptos
RUN cargo leptos build --release -vv


FROM rustlang/rust:nightly-bullseye as runner
# Copy the server binary to the /app directory
COPY --from=builder /app/target/release/web-leptos /app/
# /target/site contains our JS/WASM/CSS, etc.
COPY --from=builder /app/target/site /app/site
# Copy Cargo.toml if it’s needed at runtime
COPY --from=builder /app/Cargo.toml /app/

WORKDIR /app

ENV PORT=3000

# Set any required env variables and
ENV RUST_LOG="info"
ENV APP_ENVIRONMENT="production"
ENV LEPTOS_SITE_ROOT="site"

# default port
EXPOSE $PORT

# Run the server
CMD LEPTOS_SITE_ADDR=0.0.0.0:$PORT /app/web-leptos

