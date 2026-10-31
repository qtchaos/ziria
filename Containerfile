FROM rust AS builder
RUN apt-get update && apt-get install -y --no-install-recommends musl-tools && rm -rf /var/lib/apt/lists/*
RUN rustup target add x86_64-unknown-linux-musl
ENV CARGO_TARGET_X86_64_UNKNOWN_LINUX_MUSL_LINKER=musl-gcc
ENV RUSTFLAGS=""
WORKDIR /app
COPY . .
RUN cargo build --locked --release --target x86_64-unknown-linux-musl

FROM scratch
COPY --from=builder /app/target/x86_64-unknown-linux-musl/release/ziria /
CMD ["/ziria"]

EXPOSE 8000