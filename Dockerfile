# Start with a Rust base image
FROM rust as builder

# Create a new empty shell project
RUN USER=root cargo new --bin cfbind
WORKDIR /cfbind

# Copy our manifests
COPY ./Cargo.lock ./Cargo.lock
COPY ./Cargo.toml ./Cargo.toml

# Build only the dependencies to cache them
RUN cargo build --release
RUN rm src/*.rs

# Copy the source code
COPY ./src ./src

# Build for release
RUN cargo build --release

# Final stage
FROM debian:buster-slim

# Install OpenSSL - required for HTTPS requests
RUN apt-get update && apt-get install -y openssl ca-certificates && rm -rf /var/lib/apt/lists/*

# Copy the build artifact from the builder stage
COPY --from=builder /cloudflare-dns-updater/target/release/cfbind .

# Set the startup command
ENTRYPOINT ["./cfbind"]